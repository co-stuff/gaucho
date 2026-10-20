/*···········
 · https://github.com/ibolzov
 · gaucho - conquer mobile private server
 · with love from bolzov (◡‿◡✿)
 ···········*/

use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

pub type Frame = (u16, Vec<u8>);

const PORTAL_REACH: i32 = 8;

pub struct Template {
    pub hero: u32,
    pub userinfo: Vec<u8>,
    pub name: Vec<u8>,
    pub map: u32,
    pub pos: (u16, u16),
    pub look: u32,
    pub class: u32,
    pub setup: Vec<Frame>,
    pub enter: Vec<Frame>,
    pub after: Vec<Frame>,
}

pub struct MonsterType {
    pub id: u32,
    pub name: String,
    pub look: u32,
    pub life: i32,
    pub atk: (i32, i32),
    pub def: i32,
    pub level: u32,
    pub range: i32,
    pub view: i32,
    pub atk_ms: u64,
    pub move_ms: u64,
}

pub struct Spawn {
    pub typ: u32,
    pub x: u16,
    pub y: u16,
    pub cx: u16,
    pub cy: u16,
    pub count: u32,
    pub rest: u64,
}

pub struct Npc {
    pub id: u32,
    pub x: u16,
    pub y: u16,
    pub look: u32,
    pub typ: u32,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Quest {
    pub id: u32,
    pub name: String,
    pub npc: u32,
    pub map: u32,
    pub x: u16,
    pub y: u16,
    pub mode: String,
    pub plots: Vec<u32>,
    pub reading: bool,
    pub exp: u64,
    pub kill: String,
    pub count: u32,
    pub fly: Option<[u32; 3]>,
    pub path: Option<[u32; 4]>,
    pub items: Vec<[u32; 3]>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct Items {
    pub shops: HashMap<u32, Vec<u32>>,
    pub items: HashMap<u32, [u32; 4]>,
}

impl Items {
    pub fn price(&self, typ: u32) -> u32 {
        self.items.get(&typ).map_or(0, |t| t[0])
    }
    pub fn dur(&self, typ: u32) -> u16 {
        self.items.get(&typ).map_or(1, |t| t[1].clamp(1, 65535) as u16)
    }
    pub fn heal(&self, typ: u32) -> (i32, i32) {
        self.items.get(&typ).map_or((0, 0), |t| (t[2] as i32, t[3] as i32))
    }
}

pub struct Data {
    pub template: Template,
    pub monsters: HashMap<u32, MonsterType>,
    pub spawns: HashMap<u32, Vec<Spawn>>,
    pub npcs: HashMap<u32, Vec<Npc>>,
    pub player_frame: Vec<u8>,
    pub replies: HashMap<u16, Vec<(Vec<u8>, Vec<Vec<u8>>)>>,
    pub quests: Vec<Quest>,
    pub map_flags: HashMap<u32, String>,
    pub items: Items,
    pub portals: Vec<[u32; 6]>,
}

fn n(v: &Value) -> u64 {
    v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok())).unwrap_or(0)
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap_or(0)).collect()
}

fn read(path: &Path) -> Value {
    let txt = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&txt).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

fn action_of(body: &[u8]) -> u64 {
    crate::proto::Msg::parse_lossy(body).get(12)
}

impl Data {
    pub fn portal_exit(&self, map: u32, x: i32, y: i32) -> Option<(u32, i32, i32)> {
        let away = |p: &&[u32; 6]| (p[2] as i32 - x).abs().max((p[3] as i32 - y).abs());
        let from = self.portals.iter().filter(|p| p[0] == map && away(p) <= PORTAL_REACH).min_by_key(away)?;
        let to = self.portals.iter().find(|p| p[0] == from[4] && p[1] == from[5])
            .or_else(|| self.portals.iter().find(|p| p[0] == from[4] && p[4] == map))?;
        Some((to[0], to[2] as i32, to[3] as i32))
    }

    pub fn load(dir: &Path) -> Data {
        let t = read(&dir.join("login_template.json"));
        let frames: Vec<Frame> = t["frames"].as_array().expect("frames").iter()
            .map(|f| (n(&f["ty"]) as u16, unhex(f["hex"].as_str().unwrap_or(""))))
            .collect();
        let split = frames.iter()
            .position(|(ty, b)| *ty == 0x273e && matches!(action_of(b), 471 | 74 | 104))
            .unwrap_or(frames.len());
        let spawn_start = (split..frames.len())
            .find(|&j| matches!(frames[j].0, 0x27e2 | 0x27f3))
            .unwrap_or(frames.len());
        let ui = frames.iter().find(|(ty, _)| *ty == 0x27ed).map(|f| f.1.clone()).unwrap_or_default();
        let u32_at = |o: usize| ui.get(o..o + 4).map_or(0, |b| u32::from_le_bytes(b.try_into().unwrap()));
        let template = Template {
            hero: n(&t["hero_id"]) as u32,
            userinfo: ui.clone(),
            name: b"sixseven".to_vec(),
            map: n(&t["map_id"]) as u32,
            pos: (n(&t["pos"][0]) as u16, n(&t["pos"][1]) as u16),
            look: u32_at(0x06),
            class: u32_at(0x4c),
            setup: frames[..split].to_vec(),
            enter: frames[split..spawn_start].to_vec(),
            after: frames[spawn_start..].iter().filter(|(ty, _)| !matches!(ty, 0x27e2 | 0x27f3)).cloned().collect(),
        };

        let w = read(&dir.join("world.json"));
        let monsters = w["monsters"].as_object().expect("monsters").values().map(|m| {
            let t = MonsterType {
                id: n(&m["id"]) as u32,
                name: m["name"].as_str().unwrap_or("").to_lowercase(),
                look: n(&m["look"]) as u32,
                life: n(&m["life"]).max(1) as i32,
                atk: (n(&m["atk_min"]) as i32, n(&m["atk_max"]).max(n(&m["atk_min"])) as i32),
                def: n(&m["def"]) as i32,
                level: n(&m["level"]) as u32,
                range: n(&m["range"]).max(1) as i32,
                view: n(&m["view"]) as i32,
                atk_ms: n(&m["atk_speed"]).max(500),
                move_ms: n(&m["move_speed"]).max(300),
            };
            (t.id, t)
        }).collect();
        let per_map = |key: &str| -> Vec<(u32, &Vec<Value>)> {
            w[key].as_object().expect(key).iter()
                .filter_map(|(k, v)| Some((k.parse().ok()?, v.as_array()?)))
                .collect()
        };
        let spawns = per_map("spawns").into_iter().map(|(map, rows)| {
            (map, rows.iter().map(|s| Spawn {
                typ: n(&s["type"]) as u32,
                x: n(&s["x"]) as u16,
                y: n(&s["y"]) as u16,
                cx: n(&s["cx"]).max(1) as u16,
                cy: n(&s["cy"]).max(1) as u16,
                count: n(&s["count"]) as u32,
                rest: n(&s["rest"]),
            }).collect())
        }).collect();
        let npc = |r: &Value| Npc {
            id: n(&r["id"]) as u32,
            x: n(&r["x"]) as u16,
            y: n(&r["y"]) as u16,
            look: n(&r["look"]) as u32,
            typ: n(&r["type"]) as u32,
        };
        let mut npcs: HashMap<u32, Vec<Npc>> = per_map("npcs").into_iter()
            .map(|(map, rows)| (map, rows.iter().map(npc).collect())).collect();
        let q = dir.join("quests.json");
        let q = if q.exists() { read(&q) } else { Value::Null };
        for (map, rows) in q["npcs"].as_object().into_iter().flatten() {
            let list = npcs.entry(map.parse().unwrap_or(0)).or_default();
            for r in rows.as_array().into_iter().flatten() {
                if !list.iter().any(|x| x.id == n(&r["id"]) as u32) {
                    list.push(npc(r));
                }
            }
        }
        let quests = serde_json::from_value(q["quests"].clone()).unwrap_or_default();
        let map_flags = q["maps"].as_object().into_iter().flatten()
            .filter_map(|(k, v)| Some((k.parse().ok()?, v.as_str()?.to_string()))).collect();
        let replies = w["replies"].as_object().into_iter().flatten().filter_map(|(op, q)| {
            let opts = q.as_object()?.iter().map(|(query, frames)| {
                let frames = frames.as_array().into_iter().flatten().filter_map(|f| f.as_str()).map(unhex).collect();
                (unhex(query), frames)
            }).collect();
            Some((op.parse().ok()?, opts))
        }).collect();
        let player_frame = unhex(w["player_frame"].as_str().unwrap_or(""));
        let items: Items = std::fs::read_to_string(dir.join("items.json")).ok()
            .and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        for n in npcs.values_mut().flatten().filter(|n| items.shops.contains_key(&n.id)) {
            n.typ = 1;
        }
        let portals = std::fs::read_to_string(dir.join("portals.json")).ok()
            .and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        Data { template, monsters, spawns, npcs, player_frame, replies, quests, map_flags, items, portals }
    }
}

pub fn retarget(frames: &[Frame], from: (u32, &[u8]), to: (u32, &[u8])) -> Vec<Frame> {
    let (vf, vt) = (crate::proto::varint_bytes(from.0 as u64), crate::proto::varint_bytes(to.0 as u64));
    assert_eq!(vf.len(), vt.len(), "hero ids must share varint width");
    let (lf, lt) = (from.0.to_le_bytes(), to.0.to_le_bytes());
    frames.iter().map(|(ty, b)| {
        let mut b = b.clone();
        replace(&mut b, &lf, &lt, |_| true);
        replace(&mut b, &vf, &vt, |prev| prev.is_some_and(|p| p < 0x80));
        if from.1.len() == to.1.len() {
            replace(&mut b, from.1, to.1, |_| true);
        }
        (*ty, b)
    }).collect()
}

fn replace(b: &mut [u8], from: &[u8], to: &[u8], ok: impl Fn(Option<u8>) -> bool) {
    let mut i = 0;
    while i + from.len() <= b.len() {
        if &b[i..i + from.len()] == from && ok(i.checked_sub(1).map(|p| b[p])) {
            b[i..i + from.len()].copy_from_slice(to);
            i += from.len();
        } else {
            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retarget_swaps_id_and_name() {
        let pb = crate::proto::Pb::new().v(1, 1282811).b(13, b"sixseven").out();
        let mut bin = 1282811u32.to_le_bytes().to_vec();
        bin.extend_from_slice(b"..sixseven");
        let out = retarget(&[(1, pb), (2, bin)], (1282811, b"sixseven"), (1300001, b"Hero0001"));
        let m = crate::proto::Msg::parse(&out[0].1).unwrap();
        assert_eq!((m.get(1), m.bytes(13, 0)), (1300001, Some(&b"Hero0001"[..])));
        assert_eq!(&out[1].1[..4], &1300001u32.to_le_bytes());
        assert!(out[1].1.ends_with(b"Hero0001"));
    }

    #[test]
    fn portals_lead_to_the_linked_cell() {
        let d = Data::load(Path::new("../data"));
        assert_eq!(d.portal_exit(1002, 382, 387), Some((1006, 37, 31)));
        assert_eq!(d.portal_exit(1006, 37, 31), Some((1002, 386, 389)));
        assert_eq!(d.portal_exit(1015, 1010, 710), Some((1002, 466, 330)));
        assert_eq!(d.portal_exit(1002, 430, 388), None);
    }
}
