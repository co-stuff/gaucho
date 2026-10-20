/*#######################
 · https://github.com/ibolzov
 · gaucho - conquer mobile private server
 · with love from bolzov (ᵔᴥᵔ)
 #######################*/

use super::*;

const RED: u32 = 30;
const BLACK: u32 = 100;
const FLASH: Duration = Duration::from_secs(60);
pub(super) const PK_DECAY: Duration = Duration::from_secs(360);

fn criminal(p: &Player, now: Instant) -> bool {
    p.h.pk >= RED || p.flash.is_some_and(|t| now < t)
}

fn may_pk(a: &Player, v: &Player, now: Instant) -> bool {
    match a.pk_mode {
        0 | 2 => true,
        3 | 4 => criminal(v, now),
        5 => v.h.guild == 0 || v.h.guild != a.h.guild,
        6 => v.pk_mode == 6,
        _ => false,
    }
}

pub(super) fn status_word(p: &Player) -> u64 {
    let dead = if p.alive { 0 } else { 0x20 };
    let flash = if p.flash.is_some_and(|t| Instant::now() < t) { 0x1 } else { 0 };
    let name = match p.h.pk {
        n if n >= BLACK => 0x8000,
        n if n >= RED => 0x4000,
        _ => 0,
    };
    dead | flash | name
}

impl World {
    pub(super) fn send_status(&self, cid: u64) {
        let p = &self.players[&cid];
        let ride = if p.h.riding { RIDE_BIT } else { 0 };
        self.send_view(p.h.map, p.h.id, 0x27c0, &status(p.h.id, status_word(p), ride));
    }

    pub(super) fn can_pk(&self, cid: u64, tc: u64) -> bool {
        cid != tc && may_pk(&self.players[&cid], &self.players[&tc], Instant::now())
    }

    pub(super) fn pk_hit(&mut self, cid: u64, tc: u64) {
        let now = Instant::now();
        if criminal(&self.players[&tc], now) {
            return;
        }
        let a = self.players.get_mut(&cid).unwrap();
        let was = a.flash.is_some_and(|t| now < t);
        a.flash = Some(now + FLASH);
        if !was {
            self.send_status(cid);
        }
    }

    fn pk_kill(&mut self, ac: u64) {
        let a = self.players.get_mut(&ac).unwrap();
        a.h.pk += 10;
        let (id, pk) = (a.h.id, a.h.pk);
        println!("[WORLD] {} pk points {pk}", a.h.name);
        self.send(ac, 0x27c0, &attrib(id, &[(6, pk as u64)]));
        self.send_status(ac);
    }

    pub(super) fn pk_tick(&mut self, now: Instant) {
        let mut changed = Vec::new();
        for (&cid, p) in &mut self.players {
            if p.flash.is_some_and(|t| now >= t) {
                p.flash = None;
                changed.push(cid);
            }
            if now >= p.pk_decay {
                p.pk_decay = now + PK_DECAY;
                if p.h.pk > 0 {
                    p.h.pk -= 1;
                    let _ = p.tx.send(frame(0x27c0, &attrib(p.h.id, &[(6, p.h.pk as u64)])));
                    changed.push(cid);
                }
            }
        }
        for cid in changed {
            if self.players[&cid].ready {
                self.send_status(cid);
            }
        }
    }

    pub(super) fn interact(&mut self, cid: u64, body: &[u8]) {
        let m = Msg::parse_lossy(body);
        let (kind, target) = (m.get(13), m.get(5) as u32);
        if kind == 24 && self.players[&cid].ready {
            return self.magic_attack(cid, &m);
        }
        let now = Instant::now();
        let p = self.players.get_mut(&cid).unwrap();
        if !matches!(kind, 2 | 24) || target == 0 || !p.alive || !p.ready
            || now.duration_since(p.last_attack) < Duration::from_millis(300) {
            return;
        }
        p.last_attack = now;
        if p.h.riding {
            self.ride(cid, false);
        }
        if let Some(&i) = self.mob_at.get(&target) {
            self.hit_mob(cid, i);
        } else if let Some(&tc) = self.by_id.get(&target) {
            self.hit_player(cid, tc);
        }
    }

    pub(super) fn hit_mob(&mut self, cid: u64, i: usize) {
        let p = &self.players[&cid];
        let m = &self.mobs[i];
        let t = &self.d.monsters[&m.typ];
        if m.guard || matches!(m.ai, Ai::Dead { .. }) || m.map != p.h.map || dist(p.h.x, p.h.y, m.x, m.y) > MELEE.max(t.range) + 1 {
            return;
        }
        let atk = p.h.attack(self.classes.get(p.h.class));
        let dmg = if rand::thread_rng().gen_bool(0.9) { (roll(atk) - t.def).max(1) } else { 0 };
        let (pid, px, py, map, mid) = (p.h.id, p.h.x, p.h.y, m.map, m.id);
        self.send_view(map, mid, 0x2797, &hit(pid, mid, px, py, dmg));
        self.mob_hurt(cid, i, dmg);
    }

    pub(super) fn mob_hurt(&mut self, cid: u64, i: usize, dmg: i32) {
        if matches!(self.mobs[i].ai, Ai::Dead { .. }) {
            return;
        }
        let pid = self.players[&cid].h.id;
        let t = &self.d.monsters[&self.mobs[i].typ];
        let reward = (t.level as u64 * 8 + t.life as u64 / 3).max(1);
        if !self.mobs[i].guard {
            self.mobs[i].ai = Ai::Chase(pid);
        }
        if self.strike_mob(i, pid, dmg) {
            self.give_exp(cid, reward);
            self.quest_kill(cid, self.mobs[i].typ);
            self.drop_loot(cid, i);
        }
    }

    pub(super) fn strike_mob(&mut self, i: usize, att: u32, dmg: i32) -> bool {
        let m = &mut self.mobs[i];
        let (map, mid) = (m.map, m.id);
        m.hp -= dmg;
        let hp = m.hp.max(0) as u64;
        self.send_view(map, mid, 0x27c0, &attrib(mid, &[(0, hp)]));
        if self.mobs[i].hp > 0 {
            return false;
        }
        let (mx, my, rest) = (self.mobs[i].x, self.mobs[i].y, self.mobs[i].rest);
        self.send_view(map, mid, 0x2797, &kill(att, mid, mx, my));
        self.send_view(map, mid, 0x27c0, &status(mid, 0x20, 0));
        let now = Instant::now();
        self.mobs[i].ai = Ai::Dead { fade: now + CORPSE, respawn: now + CORPSE + rest, faded: false };
        true
    }

    pub(super) fn give_exp(&mut self, cid: u64, amount: u64) {
        let p = self.players.get_mut(&cid).unwrap();
        let c = self.classes.get(p.h.class);
        let up = p.h.gain_exp(amount, c);
        let (id, exp, name, level) = (p.h.id, p.h.exp, p.h.name.clone(), p.h.level);
        if up {
            println!("[WORLD] {name} reached level {level}");
            self.sync_attribs(cid);
            self.learn_skills(cid);
            let map = self.players[&cid].h.map;
            self.send_view(map, id, 0x27c0, &attrib(id, &[(12, level as u64)]));
            self.send_view(map, id, 0x273e, &Pb::new().v(1, id as u64).v(9, ms()).v(12, 92).v(14, level as u64).out());
        } else {
            self.send(cid, 0x27c0, &attrib(id, &[(5, exp)]));
        }
    }

    pub(super) fn hit_player(&mut self, cid: u64, tc: u64) {
        let (a, v) = (&self.players[&cid], &self.players[&tc]);
        if !v.alive || !v.ready || a.h.map != v.h.map || dist(a.h.x, a.h.y, v.h.x, v.h.y) > MELEE || !self.can_pk(cid, tc) {
            return;
        }
        let atk = a.h.attack(self.classes.get(a.h.class));
        let def = v.h.defense(self.classes.get(v.h.class));
        let dmg = if rand::thread_rng().gen_bool(0.9) { (roll(atk) - def).max(1) } else { 0 };
        let (aid, ax, ay) = (a.h.id, a.h.x, a.h.y);
        self.pk_hit(cid, tc);
        self.damage_player(tc, aid, ax, ay, dmg);
    }

    pub(super) fn damage_player(&mut self, tc: u64, att: u32, ax: i32, ay: i32, dmg: i32) {
        let v = &self.players[&tc];
        let (vid, map) = (v.h.id, v.h.map);
        self.send_view(map, vid, 0x2797, &hit(att, vid, ax, ay, dmg));
        self.player_hurt(tc, att, dmg);
    }

    pub(super) fn player_hurt(&mut self, tc: u64, att: u32, dmg: i32) {
        let v = self.players.get_mut(&tc).unwrap();
        let (vid, map) = (v.h.id, v.h.map);
        v.h.hp = (v.h.hp - dmg).max(0);
        let (hp, vx, vy) = (v.h.hp, v.h.x, v.h.y);
        self.send(tc, 0x27c0, &attrib(vid, &[(0, hp as u64)]));
        if hp == 0 {
            let innocent = !criminal(&self.players[&tc], Instant::now());
            self.send_view(map, vid, 0x2797, &kill(att, vid, vx, vy));
            let v = self.players.get_mut(&tc).unwrap();
            v.alive = false;
            v.h.riding = false;
            v.revive_at = Some(Instant::now() + REVIVE);
            println!("[WORLD] {} died", v.h.name);
            self.send_status(tc);
            if let Some(&ac) = self.by_id.get(&att).filter(|_| innocent) {
                self.pk_kill(ac);
            }
            for m in &mut self.mobs {
                if m.ai == Ai::Chase(vid) {
                    m.ai = Ai::Idle;
                }
            }
        }
    }

    pub(super) fn revive(&mut self, cid: u64) {
        let max = { let h = &self.players[&cid].h; h.max_hp(self.classes.get(h.class)) };
        let p = self.players.get_mut(&cid).unwrap();
        p.alive = true;
        p.revive_at = None;
        p.h.hp = max;
        let id = p.h.id;
        self.send_status(cid);
        self.send(cid, 0x27c0, &attrib(id, &[(1, max as u64), (0, max as u64)]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(mode: u8, guild: u32, pk: u32) -> Player {
        let now = Instant::now();
        Player {
            tx: tokio::sync::mpsc::unbounded_channel().0, addr: "127.0.0.1:1".parse().unwrap(),
            h: Hero { guild, pk, ..Default::default() }, key: String::new(), pending: None, dir: 0, alive: true,
            revive_at: None, entered: true, ready: true, seen: HashSet::new(), last_attack: now, last_mbtick: now,
            quest_timer: None, cine: None, res: 0, cooldown: HashMap::new(), stamina: 0, pk_mode: mode, flash: None,
            pk_decay: now, friend_req: HashSet::new(), trade_req: HashSet::new(), trade: None,
        }
    }

    #[test]
    fn pk_modes_and_names() {
        let now = Instant::now();
        let innocent = player(1, 7, 0);
        assert!(!may_pk(&player(1, 0, 0), &innocent, now));
        assert!(may_pk(&player(0, 0, 0), &innocent, now));
        assert!(!may_pk(&player(3, 0, 0), &innocent, now));
        assert!(may_pk(&player(3, 0, 0), &player(1, 0, 30), now));
        assert!(!may_pk(&player(5, 7, 0), &innocent, now) && may_pk(&player(5, 8, 0), &innocent, now));
        let mut blue = player(0, 0, 0);
        blue.flash = Some(now + FLASH);
        assert!(may_pk(&player(3, 0, 0), &blue, now));
        assert_eq!((status_word(&blue), status_word(&player(0, 0, 30)), status_word(&player(0, 0, 100))), (0x1, 0x4000, 0x8000));
    }
}
