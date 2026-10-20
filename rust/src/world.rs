/*++++++++++
 : gaucho - conquer mobile private server
 : with love from bolzov (人◕‿◕)
 : https://github.com/ibolzov
 ++++++++++*/

use crate::data::{self, Data, MonsterType};
use crate::hero::{Classes, Hero, Store};
use crate::net::{frame, Tx};
use crate::proto::{Msg, Pb};
use rand::Rng;
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::UnboundedReceiver;

mod account;
mod combat;
mod friend;
mod gm;
mod ground;
mod item;
mod mail;
mod mobs;
mod movement;
mod quest;
mod session;
mod skill;
mod social;
mod talk;
mod trade;

pub use account::{Accounts, SdkLogin};
pub use skill::Skills;
pub use social::Social;
use combat::{status_word, PK_DECAY};

const VIEW: i32 = 18;
const MOB_ID0: u32 = 400_001;
pub const HERO_ID0: u32 = 1_300_001;
const MELEE: i32 = 3;
const RIDE_BIT: u64 = 0x40000;
const REVIVE: Duration = Duration::from_secs(10);
const CORPSE: Duration = Duration::from_secs(4);

const ECHO_EMPTY: [u16; 6] = [0x288f, 0x28b1, 0x286d, 0x286b, 0x2847, 0x2803];
const ECHO_BODY: [u16; 6] = [0x2799, 0x27b3, 0x044c, 0x2798, 0x28a4, 0x27cf];
const DIRS: [(i32, i32); 8] = [(0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1), (1, 0), (1, 1)];

pub enum Ev {
    Join { cid: u64, tx: Tx, addr: SocketAddr },
    Packet { cid: u64, ty: u16, body: Vec<u8> },
    Leave { cid: u64 },
    Sdk { ip: String, account: String, pass: String, register: bool, reply: tokio::sync::oneshot::Sender<Result<SdkLogin, &'static str>> },
}

struct Player {
    tx: Tx,
    addr: SocketAddr,
    h: Hero,
    key: String,
    pending: Option<String>,
    dir: u8,
    alive: bool,
    revive_at: Option<Instant>,
    entered: bool,
    ready: bool,
    seen: HashSet<u32>,
    last_attack: Instant,
    last_mbtick: Instant,
    quest_timer: Option<Instant>,
    cine: Option<(u64, u32, Instant)>,
    res: u32,
    cooldown: HashMap<u32, Instant>,
    stamina: i32,
    pk_mode: u8,
    flash: Option<Instant>,
    pk_decay: Instant,
    friend_req: HashSet<u32>,
    trade_req: HashSet<u32>,
    trade: Option<trade::Trade>,
}

#[derive(Clone, Copy, PartialEq)]
enum Ai {
    Idle,
    Chase(u32),
    Dead { fade: Instant, respawn: Instant, faded: bool },
}

struct Mob {
    id: u32,
    map: u32,
    typ: u32,
    area: (i32, i32, i32, i32),
    rest: Duration,
    home: (i32, i32),
    x: i32,
    y: i32,
    dir: u8,
    hp: i32,
    ai: Ai,
    next_move: Instant,
    next_attack: Instant,
    guard: bool,
}

pub struct World {
    d: Data,
    players: HashMap<u64, Player>,
    by_id: HashMap<u32, u64>,
    mobs: Vec<Mob>,
    mob_at: HashMap<u32, usize>,
    mobs_by_map: HashMap<u32, Vec<usize>>,
    player_base: Msg,
    classes: Classes,
    store: Store,
    social: Social,
    skills: Skills,
    ground: ground::Ground,
    last_save: Instant,
    trace: bool,
    fresh: HashSet<String>,
    pick: HashMap<String, String>,
    accounts: Accounts,
    bound: HashMap<String, String>,
}

fn dist(ax: i32, ay: i32, bx: i32, by: i32) -> i32 {
    (ax - bx).abs().max((ay - by).abs())
}
fn unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}
fn ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64) & 0xffff_ffff
}
fn clock() -> Vec<u8> {
    let tz: i64 = std::env::var("CQ2_TZ").ok().and_then(|v| v.parse().ok()).unwrap_or(-3);
    clock_at(unix() as i64, tz)
}

fn clock_at(now: i64, tz: i64) -> Vec<u8> {
    let local = now + tz * 3600;
    let (days, secs) = (local.div_euclid(86400), local.rem_euclid(86400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let before = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334][(month - 1) as usize];
    let yday = before + day - 1 + if leap && month > 2 { 1 } else { 0 };
    Pb::new().v(1, 0).v(2, (year - 1900) as u64).v(3, (month - 1) as u64).v(4, yday as u64).v(5, day as u64)
        .v(6, (secs / 3600) as u64).v(7, (secs / 60 % 60) as u64).v(8, (secs % 60) as u64).v(9, now as u64).out()
}

fn roll(r: (i32, i32)) -> i32 {
    rand::thread_rng().gen_range(r.0..=r.1.max(r.0))
}

fn remove_entity(id: u32) -> Vec<u8> {
    Pb::new().v(1, id as u64).v(3, 0).v(9, ms()).v(10, 0).v(12, 135).v(13, 0).v(14, 0).v(15, 0).out()
}
fn spawn_fx(id: u32, dir: u8, x: i32, y: i32) -> Vec<u8> {
    Pb::new().v(1, id as u64).v(3, 0).v(9, ms()).v(10, 0).v(12, 134).v(13, dir as u64).v(14, x as u64).v(15, y as u64).out()
}
fn attrib(id: u32, pairs: &[(u64, u64)]) -> Vec<u8> {
    let mut pb = Pb::new().v(1, id as u64).b(2, &Pb::new().v(1, 0xffff_ffff).v(2, 0).out());
    for &(k, v) in pairs {
        pb = pb.b(2, &Pb::new().v(1, k).v(2, v).out());
    }
    pb.out()
}
fn status(id: u32, w0: u64, w1: u64) -> Vec<u8> {
    let mut s = Pb::new().v(1, 25).v(3, w0).v(3, w1);
    for _ in 0..12 {
        s = s.v(3, 0);
    }
    Pb::new().v(1, id as u64).b(2, &s.out()).out()
}
fn hit(att: u32, tgt: u32, x: i32, y: i32, dmg: i32) -> Vec<u8> {
    let miss = if dmg == 0 { 2 } else { 0 };
    Pb::new().v(3, miss).v(4, att as u64).v(5, tgt as u64).v(7, x as u64).v(8, y as u64).v(13, 2)
        .v(15, dmg as u64).v(21, 0).v(25, 0).v(26, 0).v(27, 0).out()
}
fn kill(att: u32, tgt: u32, x: i32, y: i32) -> Vec<u8> {
    Pb::new().v(4, att as u64).v(5, tgt as u64).v(7, x as u64).v(8, y as u64).v(13, 14).v(17, 1)
        .v(18, 0).v(21, 0).v(25, 0).v(26, 1).v(29, 0).v(30, 0).out()
}
fn npc_info(n: &data::Npc) -> Vec<u8> {
    Pb::new().v(1, n.id as u64).v(2, n.id as u64).v(3, 0).v(4, n.x as u64).v(5, n.y as u64)
        .v(6, n.look as u64).v(7, n.typ as u64).v(8, 0).out()
}
fn mob_info(m: &Mob, t: &MonsterType) -> Vec<u8> {
    let mut pb = Pb::new().v(1, t.look as u64).v(2, m.id as u64).v(3, 0).v(4, 0).v(5, 0);
    for _ in 0..14 {
        pb = pb.v(6, 0);
    }
    for f in [8, 9, 10, 11, 12, 15] {
        pb = pb.v(f, 0);
    }
    pb = pb.v(23, m.hp.max(0) as u64).v(26, t.level as u64).v(27, m.x as u64).v(28, m.y as u64)
        .v(29, 0).v(30, m.dir as u64).v(31, 100).v(32, 0);
    for f in [36, 37, 38, 42, 43, 60, 61, 63, 64, 65, 66, 67, 68, 69, 78, 79] {
        pb = pb.v(f, 0);
    }
    pb = pb.v(81, 255).v(82, if m.guard { 16 } else { 1 });
    for _ in 0..4 {
        pb = pb.b(107, b"");
    }
    pb.v(111, t.id as u64).v(115, 0).out()
}
fn talk(channel: u64, text: &str) -> Vec<u8> {
    Pb::new().v(1, 0xffffff).v(2, channel).v(3, 0).v(4, 839).v(6, 0).v(7, 0).v(8, 1).v(9, 0)
        .b(13, b"SYSTEM").b(13, b"ALLUSERS").b(13, b"").b(13, text.as_bytes()).out()
}

fn walk(id: u32, dir: u8) -> Vec<u8> {
    Pb::new().v(1, dir as u64).v(2, id as u64).v(3, 1).v(4, ms()).out()
}
fn dialog(opt: u8, kind: u8, text: &str) -> Vec<u8> {
    let mut b = vec![0, 0, 0, 0, 0, 0, opt, kind];
    if text.is_empty() {
        b.push(0);
    } else {
        b.push(1);
        b.extend_from_slice(&(text.len() as u16).to_le_bytes());
        b.extend_from_slice(text.as_bytes());
    }
    b.extend_from_slice(&[0, 0, 0]);
    b
}

impl World {
    pub fn new(d: Data, classes: Classes, store: Store, social: Social, skills: Skills, accounts: Accounts) -> World {
        let player_base = Msg::parse_lossy(&d.player_frame);
        let mut w = World {
            d,
            classes,
            store,
            social,
            skills,
            ground: ground::Ground::default(),
            last_save: Instant::now(),
            trace: std::env::var("CQ2_TRACE").is_ok(),
            fresh: HashSet::new(),
            pick: HashMap::new(),
            accounts,
            bound: HashMap::new(),
            players: HashMap::new(),
            by_id: HashMap::new(),
            mobs: Vec::new(),
            mob_at: HashMap::new(),
            mobs_by_map: HashMap::new(),
            player_base,
        };
        let now = Instant::now();
        let mut rng = rand::thread_rng();
        for (&map, rows) in &w.d.spawns {
            for s in rows {
                let Some(t) = w.d.monsters.get(&s.typ) else { continue };
                for _ in 0..s.count {
                    let area = (s.x as i32, s.y as i32, s.cx as i32, s.cy as i32);
                    let (x, y) = (area.0 + rng.gen_range(0..area.2), area.1 + rng.gen_range(0..area.3));
                    let id = MOB_ID0 + w.mobs.len() as u32;
                    w.mob_at.insert(id, w.mobs.len());
                    w.mobs_by_map.entry(map).or_default().push(w.mobs.len());
                    w.mobs.push(Mob {
                        id, map, typ: s.typ, area,
                        rest: Duration::from_secs(s.rest.max(5)),
                        home: (x, y), x, y, dir: rng.gen_range(0..8), hp: t.life, ai: Ai::Idle,
                        next_move: now, next_attack: now,
                        guard: matches!(t.look, 900 | 910),
                    });
                }
            }
        }
        println!("[WORLD] {} mobs on {} maps, {} npc maps", w.mobs.len(), w.mobs_by_map.len(), w.d.npcs.len());
        w
    }

    pub async fn run(mut self, mut rx: UnboundedReceiver<Ev>) {
        let mut tick = tokio::time::interval(Duration::from_millis(100));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                ev = rx.recv() => match ev {
                    Some(Ev::Join { cid, tx, addr }) => self.join(cid, tx, addr),
                    Some(Ev::Packet { cid, ty, body }) => self.packet(cid, ty, body),
                    Some(Ev::Leave { cid }) => self.leave(cid),
                    Some(Ev::Sdk { ip, account, pass, register, reply }) => {
                        let _ = reply.send(self.sdk(ip, &account, &pass, register));
                    }
                    None => break,
                },
                _ = tick.tick() => self.tick(),
            }
        }
    }

    fn send(&self, cid: u64, ty: u16, body: &[u8]) {
        if let Some(p) = self.players.get(&cid) {
            let _ = p.tx.send(frame(ty, body));
        }
    }

    fn send_view(&self, map: u32, id: u32, ty: u16, body: &[u8]) {
        let f = frame(ty, body);
        for p in self.players.values() {
            if p.ready && p.h.map == map && (p.h.id == id || p.seen.contains(&id)) {
                let _ = p.tx.send(f.clone());
            }
        }
    }

    fn forget(&mut self, map: u32, id: u32) {
        let f = frame(0x273e, &remove_entity(id));
        for p in self.players.values_mut() {
            if p.h.map == map && p.seen.remove(&id) {
                let _ = p.tx.send(f.clone());
            }
        }
    }

    fn packet(&mut self, cid: u64, ty: u16, body: Vec<u8>) {
        if !self.players.contains_key(&cid) {
            return;
        }
        if self.trace {
            let hex: String = body.iter().take(48).map(|b| format!("{b:02x}")).collect();
            println!("[C2S {cid}] 0x{ty:04x} {hex}");
        }
        match ty {
            0x278c => self.connect(cid, &body),
            0x27a6 => self.register(cid, &body),
            0x273e => self.mb_action(cid, &body),
            0x27dd => self.on_walk(cid, &body),
            0x2797 => self.interact(cid, &body),
            0x2764 => self.npc_talk(cid, &body),
            0x280f => self.npc_answer(cid, &body),
            0x27c3 => self.chat(cid, &body),
            0x283d => self.attri_query(cid, &body),
            0x27f0 => self.send(cid, 0x27f0, &clock()),
            0x2799 if body.get(12) != Some(&27) => {
                self.use_item(cid, &body);
                self.mount_check(cid);
            }
            0x27b4 => self.pick_up(cid, &body),
            0x32ca => self.mail_list_req(cid),
            0x32cc => self.mail_action(cid, &body),
            0x2849 => self.title_storage(cid, &body),
            0x27b7 => self.syndicate(cid, &body),
            0x2734 => self.mb_name(cid, &body),
            0x279b => self.friend(cid, &body),
            0x2787 => self.trade(cid, &body),
            0x28a2 => self.syndicate_list(cid, &body),
            t if ECHO_BODY.contains(&t) => self.send(cid, t, &body),
            0x2801 => self.send(cid, 0x041f, &Pb::new().v(1, 0).out()),
            0x041c => {
                let mut r = vec![0u8; 48];
                r[0] = 8;
                r[4] = 10;
                self.send(cid, 0x041c, &r)
            }
            0x05ee => self.send(cid, 0x05ee, &[8, 0, 0, 0, 0, 0, 0, 0]),
            0x07e4 | 0x278a | 0x27d9 | 0x0833 | 0x0939 => {}
            t if ECHO_EMPTY.contains(&t) => self.send(cid, t, &[]),
            t if self.canned(cid, t, &body) => {}
            _ => println!("[GAME {cid}] unhandled 0x{ty:04x} {}", body.iter().take(32).map(|b| format!("{b:02x}")).collect::<String>()),
        }
    }

    fn canned(&self, cid: u64, ty: u16, body: &[u8]) -> bool {
        let Some(opts) = self.d.replies.get(&ty) else { return false };
        let lcp = |q: &[u8]| q.iter().zip(body).take_while(|(a, b)| a == b).count();
        let Some((_, frames)) = opts.iter().max_by_key(|(q, _)| (lcp(q), usize::MAX - q.len().abs_diff(body.len()))) else {
            return false;
        };
        let t = &self.d.template;
        let h = &self.players[&cid].h;
        let frames: Vec<_> = frames.iter().map(|f| (ty, f.clone())).collect();
        for (ty, b) in data::retarget(&frames, (t.hero, t.name.as_slice()), (h.id, h.name.as_bytes())) {
            self.send(cid, ty, &b);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_matches_capture_date() {
        let m = Msg::parse(&clock_at(1_784_483_939, 0)).unwrap();
        let got: Vec<u64> = (2..=8).map(|f| m.get(f)).collect();
        assert_eq!(got, vec![126, 6, 199, 19, 17, 58, 59]);
    }
}
