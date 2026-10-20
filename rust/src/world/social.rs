/*####################
 | with love from bolzov ʕ•ᴥ•ʔ
 | https://github.com/ibolzov
 | gaucho - conquer mobile private server
 ####################*/

use super::*;
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::path::{Path, PathBuf};

pub(super) const RIDE_MAGIC: u16 = 7001;
const MOUNT_POS: u8 = 12;
const WING: u32 = 6000;
const LEADER: u32 = 1;
const MEMBER: u32 = 6;
const GUILD_ID0: u32 = 1001;
const TITLE_SCORE: u64 = 2000;

#[derive(Default, Deserialize)]
#[serde(default)]
struct MountCfg {
    starter: u32,
    color: u32,
    move_point: u32,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct TitleCfg {
    starter: Vec<u32>,
    names: HashMap<String, String>,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct Cfg {
    mount: MountCfg,
    titles: TitleCfg,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Guild {
    pub id: u32,
    pub name: String,
    pub leader: String,
    pub announce: String,
    pub level: u32,
    pub created: u64,
}

pub struct Social {
    cfg: Cfg,
    path: PathBuf,
    guilds: Vec<Guild>,
}

impl Social {
    pub fn load(dir: &Path) -> Social {
        let read = |f: &str| std::fs::read_to_string(dir.join(f)).ok()
            .and_then(|t| serde_json::from_str(&t).map_err(|e| println!("[SOCIAL] {f}: {e}")).ok());
        let cfg = read("social.json").and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
        let guilds: Vec<Guild> = read("guilds.json").and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
        println!("[SOCIAL] {} guilds", guilds.len());
        Social { cfg, path: dir.join("guilds.json"), guilds }
    }

    fn save(&self) {
        let tmp = self.path.with_extension("tmp");
        if let Ok(txt) = serde_json::to_string_pretty(&self.guilds) {
            if std::fs::write(&tmp, txt).is_ok() {
                let _ = std::fs::rename(&tmp, &self.path);
            }
        }
    }

    fn guild(&self, id: u32) -> Option<&Guild> {
        self.guilds.iter().find(|g| id != 0 && g.id == id)
    }

    fn named(&self, name: &str) -> Option<&Guild> {
        self.guilds.iter().find(|g| g.name.eq_ignore_ascii_case(name))
    }
}

const QUERY_PLAYER: u8 = 29;
const QUERY_MATE: u8 = 16;

fn mount_of(h: &Hero) -> u32 {
    h.items.iter().find(|i| i.pos == MOUNT_POS).map_or(0, |i| i.typ)
}

fn ride_cast(id: u32, x: i32, y: i32) -> (Vec<u8>, Vec<u8>) {
    let (id, x, y, m) = (id as u64, x as u64, y as u64, RIDE_MAGIC as u64);
    let hit = Pb::new().v(4, id).v(5, id).v(7, x).v(8, y).v(13, 24).v(17, m).v(18, 0)
        .v(21, 0).v(25, 0).v(26, 0).v(29, 0).v(30, 0).out();
    (hit, Pb::new().v(1, id).v(3, x).v(4, y).v(5, m).v(6, 0).v(8, 0).v(9, 0).out())
}

fn title_msg(action: u64, code: u32, list: &[(u32, bool)]) -> Vec<u8> {
    let mut pb = Pb::new().v(1, action).v(2, 0).v(3, (code / 10000) as u64).v(4, (code % 10000) as u64);
    for &(c, on) in list {
        pb = pb.b(5, &Pb::new().v(1, (c / 10000) as u64).v(2, (c % 10000) as u64).v(3, on as u64).v(4, 0).out());
    }
    pb.out()
}

fn pack(b: &mut Vec<u8>, strs: &[&str]) {
    b.push(strs.len() as u8);
    for s in strs {
        b.extend_from_slice(&(s.len() as u16).to_le_bytes());
        b.extend_from_slice(s.as_bytes());
    }
}

fn unpack(b: &[u8]) -> Vec<String> {
    let n = b.first().copied().unwrap_or(0) as usize;
    let mut p = 1;
    let mut out = Vec::new();
    for _ in 0..n {
        let Some(l) = b.get(p..p + 2).map(|l| u16::from_le_bytes([l[0], l[1]]) as usize) else { break };
        let Some(s) = b.get(p + 2..p + 2 + l) else { break };
        out.push(String::from_utf8_lossy(s).trim_end_matches('\0').to_string());
        p += 2 + l;
    }
    out
}

fn names(id: u32, action: u8, strs: &[&str]) -> Vec<u8> {
    let mut b = id.to_le_bytes().to_vec();
    b.push(action);
    pack(&mut b, strs);
    b.extend_from_slice(&[0, 0]);
    b
}

fn player_card(h: &Hero, max_hp: i32, guild: &str) -> String {
    format!("{} {} {} 1 {} {} 0 {} {} 0 {guild} {} 1 {} {} 0 0", h.id, h.level, h.look, h.reborn, h.class, h.hp.max(0), max_hp,
        h.name, h.vip, h.battle_power())
}

fn syn_msg(action: u16, data: u32, strs: &[&str]) -> Vec<u8> {
    let mut b = vec![0u8; 36];
    b[..2].copy_from_slice(&action.to_le_bytes());
    b[4..8].copy_from_slice(&data.to_le_bytes());
    pack(&mut b, strs);
    b.extend_from_slice(&[0, 0, 0]);
    b
}

fn syn_attr(g: Option<&Guild>, rank: u32, members: u32) -> Vec<u8> {
    let mut b = vec![0u8; 99];
    b[0x3c] = 1;
    b[0x48] = 1;
    if let Some(g) = g {
        b[..4].copy_from_slice(&g.id.to_le_bytes());
        b[0x14..0x18].copy_from_slice(&members.to_le_bytes());
        b[0x18..0x1c].copy_from_slice(&rank.to_le_bytes());
        let leader = &g.leader.as_bytes()[..g.leader.len().min(31)];
        b[0x1c..0x1c + leader.len()].copy_from_slice(leader);
        b[0x4c] = g.level.min(255) as u8;
        b[0x4d..0x4f].copy_from_slice(&(if rank == LEADER { 1000u16 } else { 200 }).to_le_bytes());
    }
    b
}

fn tilde(s: &str) -> String {
    s.replace(' ', "~")
}

fn title_score(h: &Hero) -> u64 {
    if h.title != 0 || h.wing != 0 { TITLE_SCORE } else { 0 }
}

fn valid_guild_name(name: &str) -> bool {
    (2..=15).contains(&name.chars().count()) && name.chars().all(|c| c.is_alphanumeric() || "_-.".contains(c))
}

impl World {
    pub(super) fn social_fields(&self, h: &Hero, m: &mut Msg) {
        let mount = mount_of(h) as u64;
        m.set(15, mount).set(47, if mount != 0 { self.social.cfg.mount.color as u64 } else { 0 })
            .set(89, h.title as u64).set(90, title_score(h)).set(91, h.wing as u64)
            .set_nth(6, 1, if h.riding { RIDE_BIT } else { 0 })
            .set_nth_bytes(107, 5, self.social.guild(h.guild).map_or(&b""[..], |g| g.name.as_bytes()));
    }

    pub(super) fn social_ready(&mut self, cid: u64) {
        let World { players, social, .. } = self;
        let starter = social.cfg.mount.starter;
        let h = &mut players.get_mut(&cid).unwrap().h;
        h.riding = false;
        if social.guild(h.guild).is_none() {
            (h.guild, h.guild_rank) = (0, 0);
        }
        let mut steed = None;
        if starter != 0 && !h.items.iter().any(|i| i.typ == starter || i.pos == MOUNT_POS) {
            if let Some((it, _)) = h.add_item(starter, 1, 100).pop() {
                h.equip(it.id, MOUNT_POS);
                steed = h.items.iter().find(|i| i.id == it.id).cloned();
            }
        }
        for t in &social.cfg.titles.starter {
            if !h.titles.contains(t) {
                h.titles.push(*t);
            }
        }
        h.mount = mount_of(h);
        let titles: Vec<(u32, bool)> = h.titles.iter().map(|&c| (c, c == h.title || c == h.wing)).collect();
        let (guild, id, score, title, wing) = (h.guild, h.id, title_score(h), h.title, h.wing);
        if let Some(it) = steed {
            self.send(cid, 0x2784, &item::item_info(&it, 1, 0));
            self.send(cid, 0x2799, &item::equipment(&self.players[&cid].h.items));
        }
        self.send(cid, 0x2775, &super::skill::magic_info(RIDE_MAGIC as u32, 0, 0, 0));
        self.send(cid, 0x2849, &title_msg(0, 0, &titles));
        if score != 0 {
            self.send(cid, 0x27c0, &attrib(id, &[(89, score), (87, title as u64), (88, wing as u64)]));
        }
        if guild != 0 {
            self.send_guild(cid);
        }
    }

    pub(super) fn mb_name(&mut self, cid: u64, body: &[u8]) {
        if body.get(4) == Some(&QUERY_MATE) {
            let id = body.get(..4).map_or(0, |b| u32::from_le_bytes(b.try_into().unwrap()));
            return self.send(cid, 0x2734, &names(id, QUERY_MATE, &["NOMATE_NAME@@"]));
        }
        if body.get(4) != Some(&QUERY_PLAYER) {
            if !self.canned(cid, 0x2734, body) {
                println!("[GAME {cid}] unhandled 0x2734 action {:?}", body.get(4));
            }
            return;
        }
        let name = body.get(5..).map(unpack).unwrap_or_default().into_iter().next().unwrap_or_default();
        let reply = match self.players.values().find(|p| p.ready && p.h.name == name) {
            Some(p) => {
                let guild = self.social.guild(p.h.guild).map_or("", |g| g.name.as_str());
                names(0, QUERY_PLAYER, &[&name, &player_card(&p.h, p.h.max_hp(self.classes.get(p.h.class)), guild)])
            }
            None => body.to_vec(),
        };
        self.send(cid, 0x2734, &reply);
    }

    pub(super) fn toggle_ride(&mut self, cid: u64) {
        let p = &self.players[&cid];
        let on = !p.h.riding && p.alive && mount_of(&p.h) != 0;
        self.ride(cid, on);
    }

    pub(super) fn mount_check(&mut self, cid: u64) {
        let p = self.players.get_mut(&cid).unwrap();
        let mount = mount_of(&p.h);
        if p.h.mount == mount {
            return;
        }
        p.h.mount = mount;
        let (id, map, riding) = (p.h.id, p.h.map, p.h.riding);
        if riding && mount == 0 {
            self.ride(cid, false);
        }
        let info = frame(0x27f3, &self.hero_info(cid));
        for q in self.players.values().filter(|q| q.ready && q.h.map == map && q.h.id != id && q.seen.contains(&id)) {
            let _ = q.tx.send(info.clone());
        }
    }

    pub(super) fn ride(&mut self, cid: u64, on: bool) {
        let p = self.players.get_mut(&cid).unwrap();
        p.h.riding = on;
        let (id, map, x, y) = (p.h.id, p.h.map, p.h.x, p.h.y);
        println!("[WORLD] {} {}", p.h.name, if on { "mounts" } else { "dismounts" });
        let (hit, fx) = ride_cast(id, x, y);
        self.send_view(map, id, 0x2797, &hit);
        self.send_view(map, id, 0x27ff, &fx);
        self.send_status(cid);
        if on {
            self.send(cid, 0x27f0, &Pb::new().v(1, 2).v(2, self.social.cfg.mount.move_point as u64).v(3, 0).v(4, 0).v(5, 0)
                .v(6, 0).v(7, 0).v(8, 0).v(9, 0).out());
        }
    }

    pub(super) fn title_storage(&mut self, cid: u64, body: &[u8]) {
        let m = Msg::parse_lossy(body);
        let code = (m.get(3) * 10000 + m.get(4)) as u32;
        match m.get(1) {
            4 if self.players[&cid].h.titles.contains(&code) => self.equip_title(cid, code, true),
            5 => self.equip_title(cid, code, false),
            a => println!("[SOCIAL] title action {a} code {code}"),
        }
    }

    fn equip_title(&mut self, cid: u64, code: u32, on: bool) {
        let p = self.players.get_mut(&cid).unwrap();
        let wing = code / 10000 >= WING;
        let slot = if wing { &mut p.h.wing } else { &mut p.h.title };
        *slot = if on { code } else if *slot == code { 0 } else { *slot };
        let (id, map, score) = (p.h.id, p.h.map, title_score(&p.h));
        self.send(cid, 0x2849, &title_msg(if on { 4 } else { 5 }, code, &[]));
        self.send_view(map, id, 0x27c0, &attrib(id, &[(89, score), (if wing { 88 } else { 87 }, if on { code as u64 } else { 0 })]));
    }

    pub(super) fn gm_title(&mut self, cid: u64, n: u32) -> String {
        let code = if n < 10000 { n * 10000 + n } else { n };
        let h = &mut self.players.get_mut(&cid).unwrap().h;
        if code == 0 {
            let cur = h.title;
            if cur != 0 {
                self.equip_title(cid, cur, false);
            }
            return "title 0".into();
        }
        if !h.titles.contains(&code) {
            h.titles.push(code);
            self.send(cid, 0x2849, &title_msg(0, 0, &[(code, false)]));
        }
        self.equip_title(cid, code, true);
        let name = self.social.cfg.titles.names.get(&code.to_string()).cloned().unwrap_or_default();
        format!("title {code} {name}")
    }

    fn members(&self, gid: u32) -> u32 {
        self.store.heroes.values().filter(|h| h.guild == gid).count() as u32
    }

    fn guild_name(&self, g: &Guild) -> Vec<u8> {
        names(g.id, 3, &[&format!("{} {} {} {} 0", tilde(&g.name), tilde(&g.leader), g.level, self.members(g.id))])
    }

    fn send_guild(&self, cid: u64) {
        let h = &self.players[&cid].h;
        let g = self.social.guild(h.guild);
        self.send(cid, 0x27bc, &syn_attr(g, h.guild_rank, g.map_or(0, |g| self.members(g.id))));
        if let Some(g) = g {
            self.send(cid, 0x2734, &self.guild_name(g));
        }
    }

    fn set_guild(&mut self, cid: u64, gid: u32, rank: u32) {
        let p = self.players.get_mut(&cid).unwrap();
        (p.h.guild, p.h.guild_rank) = (gid, if gid == 0 { 0 } else { rank });
        let (id, map) = (p.h.id, p.h.map);
        println!("[SOCIAL] {} guild={gid} rank={}", p.h.name, p.h.guild_rank);
        self.store.put(&p.key, &p.h);
        self.store.flush();
        self.send_guild(cid);
        let info = frame(0x27f3, &self.hero_info(cid));
        let name = self.social.guild(gid).map(|g| frame(0x2734, &self.guild_name(g)));
        for q in self.players.values().filter(|q| q.ready && q.h.map == map && q.h.id != id && q.seen.contains(&id)) {
            if let Some(n) = &name {
                let _ = q.tx.send(n.clone());
            }
            let _ = q.tx.send(info.clone());
        }
    }

    fn create_guild(&mut self, cid: u64, name: &str, announce: &str) -> Result<u32, String> {
        let h = &self.players[&cid].h;
        let name = name.trim();
        if !valid_guild_name(name) {
            return Err(format!("invalid guild name: {name}"));
        }
        if self.social.named(name).is_some() {
            return Err(format!("guild {name} already exists"));
        }
        if h.guild != 0 {
            return Err("you are already in a guild".into());
        }
        let id = self.social.guilds.iter().map(|g| g.id + 1).max().unwrap_or(GUILD_ID0).max(GUILD_ID0);
        let g = Guild { id, name: name.into(), leader: h.name.clone(), announce: announce.into(), level: 1, created: unix() };
        self.social.guilds.push(g);
        self.social.save();
        self.set_guild(cid, id, LEADER);
        Ok(id)
    }

    fn join_guild(&mut self, cid: u64, gid: u32) -> Result<u32, String> {
        let g = self.social.guild(gid).ok_or_else(|| format!("guild {gid} does not exist"))?;
        if self.players[&cid].h.guild != 0 {
            return Err("you are already in a guild".into());
        }
        let id = g.id;
        self.set_guild(cid, id, MEMBER);
        Ok(id)
    }

    fn leave_guild(&mut self, cid: u64) -> Result<u32, String> {
        let gid = self.players[&cid].h.guild;
        if gid == 0 {
            return Err("you are not in a guild".into());
        }
        self.set_guild(cid, 0, 0);
        if self.members(gid) == 0 {
            self.social.guilds.retain(|g| g.id != gid);
            self.social.save();
        }
        Ok(gid)
    }

    pub(super) fn syndicate(&mut self, cid: u64, body: &[u8]) {
        let act = body.get(..2).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]));
        let data = body.get(4..8).map_or(0, |b| u32::from_le_bytes(b.try_into().unwrap()));
        let strs = unpack(body.get(36..).unwrap_or_default());
        let str_at = |i: usize| strs.get(i).map_or("", String::as_str);
        let reply = match act {
            6 => {
                if let Some(g) = self.social.guild(data) {
                    self.send(cid, 0x2734, &self.guild_name(g));
                }
                return;
            }
            12 => return self.send_guild(cid),
            57 => {
                let leader = self.social.guild(data).map_or(String::new(), |g| tilde(&g.leader));
                return self.send(cid, 0x27b7, &syn_msg(57, data, &[&format!("0 1 1 0 0 {leader}")]));
            }
            51 => self.join_guild(cid, data),
            56 => {
                let pick = self.social.guilds.iter().min_by_key(|g| (self.members(g.id), g.id)).map_or(0, |g| g.id);
                self.join_guild(cid, pick)
            }
            68 => self.create_guild(cid, str_at(0), str_at(1)),
            3 => self.leave_guild(cid),
            _ => {
                println!("[SOCIAL] syndicate action {act} data {data} {strs:?}");
                return;
            }
        };
        if let Err(e) = &reply {
            self.send(cid, 0x27c3, &talk(2005, e));
        }
        self.send(cid, 0x27b7, &syn_msg(act, reply.unwrap_or(0), &[]));
    }

    pub(super) fn syndicate_list(&mut self, cid: u64, body: &[u8]) {
        let m = Msg::parse_lossy(body);
        let per = match m.get(4) { 0 => 10, n => n.min(20) } as usize;
        let page = m.get(3).max(1) as usize;
        let mut gs: Vec<(&Guild, u32)> = self.social.guilds.iter().map(|g| (g, self.members(g.id))).collect();
        gs.sort_by_key(|&(g, n)| (Reverse(n), g.id));
        let mut pb = Pb::new().v(3, page as u64).v(5, gs.len() as u64);
        for (i, (g, n)) in gs.iter().enumerate().skip((page - 1) * per).take(per) {
            pb = pb.b(6, &Pb::new().v(1, g.id as u64).v(2, i as u64).v(3, 0).b(4, g.name.as_bytes()).b(5, g.leader.as_bytes())
                .v(6, g.level as u64).v(7, *n as u64).b(8, g.announce.as_bytes()).v(9, 0).v(10, 0).out());
        }
        self.send(cid, 0x28a2, &pb.out());
    }

    pub(super) fn gm_guild(&mut self, cid: u64, words: &[&str]) -> String {
        let rest = words.get(1..).unwrap_or_default().join(" ");
        let r = match words.first().map(|w| w.to_lowercase()).as_deref() {
            Some("create") => self.create_guild(cid, &rest, ""),
            Some("join") => match self.social.named(&rest).map(|g| g.id) {
                Some(id) => self.join_guild(cid, id),
                None => rest.parse().map_err(|_| format!("guild {rest} does not exist")).and_then(|id| self.join_guild(cid, id)),
            },
            Some("leave") => self.leave_guild(cid),
            _ => {
                let list: Vec<String> = self.social.guilds.iter().map(|g| format!("{}:{}({})", g.id, g.name, self.members(g.id))).collect();
                return format!("/guild create N | join N | leave -- {}", list.join(" "));
            }
        };
        match r {
            Ok(id) => format!("guild {} ok", self.social.guild(id).map_or("-".into(), |g| g.name.clone())),
            Err(e) => e,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packets_match_capture() {
        let s = Msg::parse(&status(7, 0, RIDE_BIT)).unwrap();
        let words: Vec<u64> = Msg::parse(s.bytes(2, 0).unwrap()).unwrap().0.iter().skip(1).map(|(_, v)| match v {
            crate::proto::Val::V(x) => *x,
            _ => 99,
        }).collect();
        assert_eq!((words.len(), words[1]), (14, 262144));
        assert_eq!(super::super::skill::magic_info(7001, 0, 0, 0), unhex_bytes("00000000591b00000000000000000000010000000000000000000000"));
        assert_eq!(names(0x1392fb, 10, &["xp"]), unhex_bytes("fb9213000a01020078700000"));
        let og = unhex_bytes("390000000100000000000000000000000000000000000000000000000000000000000000010f00302031203120302030202a42616279000000");
        assert_eq!(syn_msg(57, 1, &["0 1 1 0 0 *Baby"]), og);
        assert_eq!(unpack(&og[36..]), vec!["0 1 1 0 0 *Baby".to_string()]);
        assert_eq!(syn_msg(56, 0, &[]).len(), 40);
        assert_eq!(syn_attr(None, 0, 0).len(), 99);
        assert_eq!(title_msg(0, 0, &[]), unhex_bytes("0800100018002000"));
        let t = Msg::parse(&title_msg(0, 0, &[(60030022, true)])).unwrap();
        assert_eq!(t.bytes(5, 0), Some(&unhex_bytes("08f32e101618012000")[..]));
        assert!(valid_guild_name("The_Gauchos") && !valid_guild_name("a b") && !valid_guild_name("x"));
        let h = Hero { id: 1300001, level: 33, look: 5342007, class: 5000, hp: 900, name: "GauchoNinja".into(), ..Default::default() };
        let card = player_card(&h, 1000, "");
        assert_eq!(card, "1300001 33 5342007 1 0 5000 0 900 1000 0  GauchoNinja 1 0 33 0 0");
        assert_eq!(card.split(' ').filter(|t| !t.is_empty()).count(), 16);
        assert_eq!(player_card(&h, 1000, "Gauchos").split(' ').count(), 17);
    }

    fn unhex_bytes(s: &str) -> Vec<u8> {
        (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
    }
}
