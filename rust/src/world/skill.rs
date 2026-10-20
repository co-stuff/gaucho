/*···········
 # with love from bolzov (●'◡'●)
 # https://github.com/ibolzov
 # gaucho - conquer mobile private server
 ···········*/

use super::*;
use super::social::RIDE_MAGIC;
use crate::hero::Skill;
use serde::Deserialize;
use std::path::Path;

const MAX_TARGETS: usize = 12;
const HEAL_RADIUS: i32 = 5;
const SLACK: i32 = 3;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Magic {
    pub level: u16,
    pub sort: u32,
    pub target: u32,
    pub ground: u32,
    pub multi: u32,
    pub mp: i32,
    pub power: i32,
    pub range: i32,
    pub distance: i32,
    pub need_exp: u32,
    pub need_level: u32,
    pub xp: u32,
    pub ep: i32,
    pub delay: u64,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct Stamina {
    max: i32,
    regen: i32,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct Skills {
    stamina: Stamina,
    xp_cooldown_ms: u64,
    classes: HashMap<u32, Vec<u32>>,
    magics: HashMap<u32, Vec<Magic>>,
    #[serde(skip)]
    next_regen: Option<Instant>,
}

impl Skills {
    pub fn load(dir: &Path) -> Skills {
        let path = dir.join("skills.json");
        let s: Skills = std::fs::read_to_string(&path).ok()
            .and_then(|t| serde_json::from_str(&t).map_err(|e| println!("[SKILL] {}: {e}", path.display())).ok())
            .unwrap_or_default();
        println!("[SKILL] {} classes, {} magics", s.classes.len(), s.magics.len());
        s
    }

    fn row(&self, typ: u32, level: u16) -> Option<&Magic> {
        self.magics.get(&typ)?.iter().find(|m| m.level == level)
    }

    fn learnable(&self, class: u32, level: u32) -> Vec<u32> {
        self.classes.get(&class).into_iter().flatten()
            .filter(|t| self.row(**t, 0).is_some_and(|m| m.need_level <= level)).copied().collect()
    }
}

#[derive(Debug, PartialEq)]
enum Kind {
    Damage,
    Heal,
    Effect,
}

fn kind(m: &Magic) -> Kind {
    match m.sort {
        2 => Kind::Heal,
        6 | 7 | 32 | 70 => Kind::Effect,
        _ if m.target & 8 != 0 => Kind::Effect,
        _ if m.power >= 30000 || (m.sort == 1 && m.power > 0) => Kind::Damage,
        _ => Kind::Effect,
    }
}

fn area(m: &Magic) -> bool {
    m.multi != 0 || m.ground != 0 || m.range > 0
}

fn amount(m: &Magic, base: i32) -> i32 {
    if m.power >= 30000 { base * (m.power - 30000) / 100 } else { m.power + base }
}

fn heal_amount(m: &Magic, max_hp: i32) -> i32 {
    if m.power >= 30000 { max_hp * (m.power - 30000) / 100 } else { m.power }
}

#[derive(Debug, PartialEq)]
struct Cast {
    magic: u16,
    target: u32,
    x: i32,
    y: i32,
}

fn decode(m: &Msg, uid: u32) -> Cast {
    let u = uid as u16;
    let x = ((m.get(7) as u16) ^ u ^ 0x2ed6).rotate_left(1).wrapping_add(0xdd12);
    let y = ((m.get(8) as u16) ^ u ^ 0xb99b).rotate_left(5).wrapping_add(0x76de);
    Cast {
        magic: ((m.get(19) as u16) ^ u ^ 0x915d).rotate_left(3).wrapping_add(0x14be),
        target: ((m.get(5) as u32).rotate_left(19) ^ uid ^ 0x5f2d_2463).wrapping_add(0x8b90_b51a),
        x: x as i32,
        y: y as i32,
    }
}

fn level_up(sk: &mut Skill, rows: &[Magic], hero_level: u32, gain: u32) -> bool {
    let Some(cur) = rows.iter().find(|m| m.level == sk.level) else { return false };
    if cur.need_exp == 0 {
        return false;
    }
    sk.exp = sk.exp.saturating_add(gain);
    let ready = rows.iter().find(|m| m.level == sk.level + 1).is_some_and(|n| hero_level >= n.need_level);
    if sk.exp >= cur.need_exp && ready {
        sk.level += 1;
        sk.exp = 0;
        return true;
    }
    false
}

pub(super) fn magic_info(typ: u32, level: u16, exp: u32, action: u16) -> Vec<u8> {
    let mut b = vec![0u8; 28];
    b[..4].copy_from_slice(&exp.to_le_bytes());
    b[4..8].copy_from_slice(&typ.to_le_bytes());
    b[8..10].copy_from_slice(&level.to_le_bytes());
    b[10..12].copy_from_slice(&action.to_le_bytes());
    b[16] = 1;
    b
}

fn flush_exp(typ: u32, exp: u32) -> Vec<u8> {
    let mut b = exp.to_le_bytes().to_vec();
    b.extend_from_slice(&[0, 0, 0, 0]);
    b.extend_from_slice(&(typ as u16).to_le_bytes());
    b.extend_from_slice(&[1, 0]);
    b
}

fn cast_echo(id: u32, target: u32, x: i32, y: i32, magic: u32, level: u16) -> Vec<u8> {
    Pb::new().v(4, id as u64).v(5, target as u64).v(7, x as u64).v(8, y as u64).v(13, 24).v(17, magic as u64)
        .v(18, level as u64).v(21, 0).v(25, 0).v(26, 0).v(29, 0).v(30, 0).out()
}

fn effect(id: u32, x: i32, y: i32, magic: u32, level: u16, roles: &[(u32, i32)]) -> Vec<u8> {
    let mut pb = Pb::new().v(1, id as u64).v(3, x as u64).v(4, y as u64).v(5, magic as u64).v(6, level as u64).v(8, 0).v(9, 0);
    for &(t, n) in roles {
        let mut r = Pb::new().v(1, t as u64).v(2, n.max(0) as u64);
        for f in 3..=9 {
            r = r.v(f, 0);
        }
        pb = pb.b(10, &r.out());
    }
    pb.out()
}

enum Hit {
    Mob(usize),
    Hero(u64),
}

impl World {
    pub(super) fn send_skills(&mut self, cid: u64) {
        self.learn_skills(cid);
        for s in &self.players[&cid].h.skills {
            self.send(cid, 0x2775, &magic_info(s.typ, s.level, s.exp, 0));
        }
    }

    pub(super) fn skills_ready(&mut self, cid: u64) {
        let max = self.skills.stamina.max;
        let p = self.players.get_mut(&cid).unwrap();
        p.stamina = max;
        let id = p.h.id;
        self.send(cid, 0x27c0, &attrib(id, &[(8, max as u64)]));
    }

    pub(super) fn learn_skills(&mut self, cid: u64) {
        let h = &mut self.players.get_mut(&cid).unwrap().h;
        let new: Vec<u32> = self.skills.learnable(h.class, h.level).into_iter()
            .filter(|t| !h.skills.iter().any(|s| s.typ == *t)).collect();
        for &typ in &new {
            h.skills.push(Skill { typ, level: 0, exp: 0 });
        }
        if !new.is_empty() {
            println!("[SKILL] {} learns {new:?}", h.name);
        }
        if self.players[&cid].ready {
            for typ in new {
                self.send(cid, 0x2775, &magic_info(typ, 0, 0, 0));
            }
        }
    }

    pub(super) fn stamina_tick(&mut self, now: Instant) {
        if self.skills.next_regen.is_some_and(|t| now < t) {
            return;
        }
        self.skills.next_regen = Some(now + Duration::from_secs(1));
        let (max, regen) = (self.skills.stamina.max, self.skills.stamina.regen);
        for p in self.players.values_mut().filter(|p| p.ready && p.stamina < max) {
            p.stamina = (p.stamina + regen).min(max);
            let _ = p.tx.send(frame(0x27c0, &attrib(p.h.id, &[(8, p.stamina as u64)])));
        }
    }

    fn entity_pos(&self, map: u32, id: u32) -> Option<(i32, i32, Hit)> {
        if let Some(&i) = self.mob_at.get(&id) {
            let m = &self.mobs[i];
            return (m.map == map && !matches!(m.ai, Ai::Dead { .. }) && !m.guard).then_some((m.x, m.y, Hit::Mob(i)));
        }
        let &c = self.by_id.get(&id)?;
        let p = &self.players[&c];
        (p.h.map == map && p.alive && p.ready).then_some((p.h.x, p.h.y, Hit::Hero(c)))
    }

    pub(super) fn magic_attack(&mut self, cid: u64, m: &Msg) {
        let p = &self.players[&cid];
        let c = decode(m, p.h.id);
        if c.magic == RIDE_MAGIC {
            return self.ride_cast(cid);
        }
        let typ = c.magic as u32;
        let Some(sk) = p.h.skills.iter().find(|s| s.typ == typ).cloned() else {
            return println!("[SKILL] {} cast unknown magic {typ}", p.h.name);
        };
        let Some(row) = self.skills.row(typ, sk.level).cloned() else { return };
        let now = Instant::now();
        let (uid, map, px, py) = (p.h.id, p.h.map, p.h.x, p.h.y);
        if !p.alive || p.cooldown.get(&typ).is_some_and(|t| now < *t) || p.h.mp < row.mp || p.stamina < row.ep {
            return;
        }
        let target = self.entity_pos(map, c.target);
        let echo_target = if target.is_some() { c.target } else { 0 };
        let center = match (&target, row.ground) {
            (_, 1) => (c.x, c.y),
            (Some((x, y, _)), _) => (*x, *y),
            (None, _) => (px, py),
        };
        if row.distance > 0 && dist(px, py, center.0, center.1) > row.distance + SLACK {
            return;
        }
        let delay = if row.xp == 1 { row.delay.max(self.skills.xp_cooldown_ms) } else { row.delay };
        let cls = self.classes.get(p.h.class);
        let (atk, max_hp) = (p.h.attack(cls), p.h.max_hp(cls));
        if p.h.riding && matches!(kind(&row), Kind::Damage) {
            self.ride(cid, false);
        }
        let p = self.players.get_mut(&cid).unwrap();
        p.h.mp -= row.mp;
        p.stamina -= row.ep;
        p.cooldown.insert(typ, now + Duration::from_millis(delay));
        if row.mp > 0 {
            self.send(cid, 0x27c0, &attrib(uid, &[(2, self.players[&cid].h.mp.max(0) as u64)]));
        }
        if row.ep > 0 {
            self.send(cid, 0x27c0, &attrib(uid, &[(8, self.players[&cid].stamina.max(0) as u64)]));
        }
        let hits: Vec<(Hit, u32)> = match kind(&row) {
            Kind::Effect => vec![],
            Kind::Heal => self.heal_targets(cid, map, target, row.multi != 0),
            Kind::Damage if area(&row) => self.area_targets(map, center, row.range.max(1)),
            Kind::Damage => target.map(|(_, _, h)| h).into_iter().filter(|h| !matches!(h, Hit::Hero(t) if !self.can_pk(cid, *t)))
                .map(|h| (h, c.target)).collect(),
        };
        let mut rng = rand::thread_rng();
        let roles: Vec<(Hit, u32, i32)> = hits.into_iter().map(|(h, id)| {
            let n = match (&h, kind(&row)) {
                (_, Kind::Heal) => heal_amount(&row, max_hp),
                (Hit::Mob(i), _) => (amount(&row, rng.gen_range(atk.0..=atk.1.max(atk.0))) - self.d.monsters[&self.mobs[*i].typ].def).max(1),
                (Hit::Hero(t), _) => {
                    let v = &self.players[t].h;
                    (amount(&row, rng.gen_range(atk.0..=atk.1.max(atk.0))) - v.defense(self.classes.get(v.class))).max(1)
                }
            };
            (h, id, n)
        }).collect();
        self.send_view(map, uid, 0x2797, &cast_echo(uid, echo_target, center.0, center.1, typ, sk.level));
        let list: Vec<(u32, i32)> = roles.iter().map(|(_, id, n)| (*id, *n)).collect();
        self.send_view(map, uid, 0x27ff, &effect(uid, center.0, center.1, typ, sk.level, &list));
        let mut gained = 0u32;
        for (h, id, n) in roles {
            gained = gained.saturating_add(n as u32);
            match (h, kind(&row)) {
                (Hit::Hero(t), Kind::Heal) => self.heal(t, id, n),
                (Hit::Mob(i), _) => self.mob_hurt(cid, i, n),
                (Hit::Hero(t), _) => {
                    self.pk_hit(cid, t);
                    self.player_hurt(t, uid, n);
                }
            }
        }
        self.skill_exp(cid, typ, gained.max(1));
    }

    fn ride_cast(&mut self, cid: u64) {
        let cost = self.skills.row(RIDE_MAGIC as u32, 0).map_or(0, |m| m.ep);
        let p = &self.players[&cid];
        if !p.h.riding && p.stamina < cost {
            return;
        }
        let was = p.h.riding;
        self.toggle_ride(cid);
        let p = self.players.get_mut(&cid).unwrap();
        if !was && p.h.riding {
            p.stamina -= cost;
            let (id, st) = (p.h.id, p.stamina);
            self.send(cid, 0x27c0, &attrib(id, &[(8, st as u64)]));
        }
    }

    fn heal_targets(&self, cid: u64, map: u32, target: Option<(i32, i32, Hit)>, multi: bool) -> Vec<(Hit, u32)> {
        let me = &self.players[&cid];
        if multi {
            return self.players.iter()
                .filter(|(_, q)| q.ready && q.alive && q.h.map == map && dist(q.h.x, q.h.y, me.h.x, me.h.y) <= HEAL_RADIUS)
                .map(|(c, q)| (Hit::Hero(*c), q.h.id)).take(MAX_TARGETS).collect();
        }
        match target {
            Some((_, _, Hit::Hero(c))) => vec![(Hit::Hero(c), self.players[&c].h.id)],
            _ => vec![(Hit::Hero(cid), me.h.id)],
        }
    }

    fn area_targets(&self, map: u32, (cx, cy): (i32, i32), radius: i32) -> Vec<(Hit, u32)> {
        let mut found: Vec<(i32, usize)> = self.mobs_by_map.get(&map).into_iter().flatten()
            .map(|&i| (&self.mobs[i], i))
            .filter(|(m, _)| !m.guard && !matches!(m.ai, Ai::Dead { .. }) && dist(m.x, m.y, cx, cy) <= radius)
            .map(|(m, i)| (dist(m.x, m.y, cx, cy), i)).collect();
        found.sort();
        found.into_iter().take(MAX_TARGETS).map(|(_, i)| (Hit::Mob(i), self.mobs[i].id)).collect()
    }

    fn heal(&mut self, tc: u64, id: u32, n: i32) {
        let max = { let h = &self.players[&tc].h; h.max_hp(self.classes.get(h.class)) };
        let v = self.players.get_mut(&tc).unwrap();
        v.h.hp = (v.h.hp + n).min(max);
        let hp = v.h.hp as u64;
        let map = v.h.map;
        self.send_view(map, id, 0x27c0, &attrib(id, &[(0, hp)]));
    }

    fn skill_exp(&mut self, cid: u64, typ: u32, gain: u32) {
        let Some(rows) = self.skills.magics.get(&typ) else { return };
        let h = &mut self.players.get_mut(&cid).unwrap().h;
        let level = h.level;
        let Some(sk) = h.skills.iter_mut().find(|s| s.typ == typ) else { return };
        let before = sk.clone();
        let up = level_up(sk, rows, level, gain);
        let sk = sk.clone();
        if up {
            println!("[SKILL] {} {typ} reached level {}", h.name, sk.level);
            self.send(cid, 0x2775, &magic_info(typ, sk.level, 0, 1));
        } else if sk != before {
            self.send(cid, 0x27d7, &flush_exp(typ, sk.exp));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
    }

    fn row(level: u16, need_exp: u32, need_level: u32) -> Magic {
        Magic { level, need_exp, need_level, ..Magic::default() }
    }

    #[test]
    fn decode_level_and_packets() {
        let mut m = Msg::default();
        m.set(5, 2372871543).set(7, 11869).set(8, 63282).set(19, 25461);
        assert_eq!(decode(&m, 1282811), Cast { magic: 7001, target: 1282811, x: 499, y: 313 });
        let mut m = Msg::default();
        m.set(7, 11949).set(8, 42803).set(19, 35218);
        let c = decode(&m, 1282811);
        assert_eq!((c.magic, c.x, c.y), (26210, 531, 335));

        let thunder = Magic { sort: 1, power: 7, ..Magic::default() };
        let blade = Magic { sort: 66, power: 30150, ground: 1, range: 3, ..Magic::default() };
        let cure = Magic { sort: 2, power: 5000, ..Magic::default() };
        let superman = Magic { sort: 6, power: 31000, ..Magic::default() };
        let passive = Magic { sort: 70, power: 5, target: 8, ..Magic::default() };
        assert_eq!([kind(&thunder), kind(&blade), kind(&cure), kind(&superman), kind(&passive)],
            [Kind::Damage, Kind::Damage, Kind::Heal, Kind::Effect, Kind::Effect]);
        assert_eq!((amount(&thunder, 50), amount(&blade, 100), area(&blade), area(&thunder)), (57, 150, true, false));
        assert_eq!(heal_amount(&cure, 2000), 5000);

        let rows = [row(0, 100, 0), row(1, 300, 10), row(2, 0, 20)];
        let mut sk = Skill { typ: 1000, level: 0, exp: 0 };
        assert!(!level_up(&mut sk, &rows, 5, 150));
        assert_eq!(sk.exp, 150);
        assert!(level_up(&mut sk, &rows, 10, 1));
        assert_eq!((sk.level, sk.exp), (1, 0));
        assert!(level_up(&mut sk, &rows, 30, 400));
        assert!(!level_up(&mut sk, &rows, 99, 9999));
        assert_eq!(sk.level, 2);

        assert_eq!(magic_info(26210, 0, 0, 0), unhex("00000000626600000000000000000000010000000000000000000000"));
        assert_eq!(flush_exp(26210, 504), unhex("f80100000000000062660100"));
        let e = Msg::parse(&effect(1, 2, 3, 26210, 0, &[(9, 735)])).unwrap();
        assert_eq!((e.get(5), Msg::parse(e.bytes(10, 0).unwrap()).unwrap().get(2)), (26210, 735));
    }
}
