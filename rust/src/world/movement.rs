/*ooooooooooo
 * https://github.com/ibolzov
 * gaucho - conquer mobile private server
 * with love from bolzov (◡‿◡✿)
 ooooooooooo*/

use super::*;

impl World {
    pub(super) fn mb_action(&mut self, cid: u64, body: &[u8]) {
        let m = Msg::parse_lossy(body);
        let action = m.get(12);
        let p = &self.players[&cid];
        let id = if m.get(1) != 0 { m.get(1) } else { p.h.id as u64 };
        let (x, y, map) = (p.h.x as u64, p.h.y as u64, p.h.map as u64);
        let (dw, dir, now) = (m.get(3), m.get(13), ms());
        let base = |a: u64, d3: u64| Pb::new().v(1, id).v(3, d3).v(5, id).v(7, x).v(8, y).v(9, now).v(12, a);
        match action {
            74 => {
                if !p.entered {
                    self.enter(cid);
                }
            }
            75..=78 | 97 => {
                self.send(cid, 0x273e, &base(action, map).v(14, x).v(15, y).v(17, map).out());
                if action == 97 && !p.ready {
                    self.ready(cid);
                }
            }
            408 => {
                if !p.ready {
                    self.ready(cid);
                }
            }
            96 => {
                self.players.get_mut(&cid).unwrap().pk_mode = dw as u8;
                self.send(cid, 0x273e, &Pb::new().v(1, id).v(3, dw).v(5, dw).v(7, x).v(8, y).v(9, now).v(12, 96)
                    .v(14, x).v(15, y).v(17, map).out())
            }
            102 => {
                if dw != 0 && self.players[&cid].h.riding {
                    self.ride(cid, false);
                }
                self.send(cid, 0x273e, &base(102, dw).v(13, dir).v(14, x).v(15, y).v(17, map).out())
            }
            117 => self.view_equipment(cid, dw as u32),
            130 | 132 | 466 | 485 | 489 => self.send(cid, 0x273e, &base(action, map).out()),
            258 => {
                let text = String::from_utf8_lossy(m.bytes(24, 0).unwrap_or_default()).into_owned();
                if text.contains("UpFeedBack_DownloadReward") {
                    self.download_reward(cid);
                }
                self.quest_lua(cid, &text);
            }
            85 => {
                let (px, py) = if m.get(7) != 0 { (m.get(7) as i32, m.get(8) as i32) } else { (x as i32, y as i32) };
                match self.d.portal_exit(map as u32, px, py) {
                    Some((to, tx, ty)) => {
                        println!("[PORTAL] {} {map} ({px},{py}) -> {to} ({tx},{ty})", p.h.name);
                        self.change_map(cid, to, tx, ty);
                    }
                    None => println!("[PORTAL] {} no portal near {map} ({px},{py})", p.h.name),
                }
            }
            456 => self.quest_cine_ack(cid, m.get(4)),
            162 => {}
            81 | 133 | 137 | 161 | 230 => self.on_jump(cid, &m),
            _ => {
                let mut r = base(action, dw).v(13, dir).v(14, x).v(15, y);
                if matches!(action, 79 | 92 | 114) {
                    r = r.v(17, map);
                }
                self.send(cid, 0x273e, &r.out());
            }
        }
    }

    pub(super) fn on_jump(&mut self, cid: u64, m: &Msg) {
        let p = &self.players[&cid];
        if !p.alive {
            return;
        }
        let (tx, ty) = (m.get(7) as i32, m.get(8) as i32);
        let (tx, ty) = if tx == 0 || ty == 0 { (p.h.x, p.h.y) } else { (tx, ty) };
        let body = Pb::new().v(1, p.h.id as u64).v(7, tx as u64).v(8, ty as u64).v(9, ms()).v(12, m.get(12))
            .v(13, m.get(13)).v(14, p.h.x as u64).v(15, p.h.y as u64).v(17, p.h.map as u64).v(23, 0).out();
        let (id, map) = (p.h.id, p.h.map);
        self.send_view(map, id, 0x273e, &body);
        let p = self.players.get_mut(&cid).unwrap();
        p.h.x = tx;
        p.h.y = ty;
        p.dir = m.get(13) as u8 & 7;
        self.refresh_view(cid);
    }

    pub(super) fn on_walk(&mut self, cid: u64, body: &[u8]) {
        let m = Msg::parse_lossy(body);
        let dir = (m.get(1) & 7) as u8;
        let p = self.players.get_mut(&cid).unwrap();
        if !p.alive {
            return;
        }
        let (dx, dy) = DIRS[dir as usize];
        p.h.x += dx;
        p.h.y += dy;
        p.dir = dir;
        let (id, map) = (p.h.id, p.h.map);
        self.send(cid, 0x27dd, body);
        let f = frame(0x27dd, &Pb::new().v(1, dir as u64).v(2, id as u64).v(3, m.get(3)).v(4, m.get(4)).out());
        for p in self.players.values() {
            if p.h.id != id && p.h.map == map && p.seen.contains(&id) {
                let _ = p.tx.send(f.clone());
            }
        }
        self.refresh_view(cid);
    }

    pub(super) fn refresh_view(&mut self, cid: u64) {
        let Some(p) = self.players.get(&cid) else { return };
        if !p.ready {
            return;
        }
        let (map, x, y, id) = (p.h.map, p.h.x, p.h.y, p.h.id);
        let mine = self.hero_info(cid);
        let others: Vec<u64> = self.players.iter()
            .filter(|(c, q)| **c != cid && q.ready && q.h.map == map)
            .map(|(c, _)| *c).collect();
        for oc in others {
            let (qid, d) = { let q = &self.players[&oc]; (q.h.id, dist(x, y, q.h.x, q.h.y)) };
            if d <= VIEW {
                if self.players.get_mut(&cid).unwrap().seen.insert(qid) {
                    self.send(cid, 0x27f3, &self.hero_info(oc));
                }
                let q = self.players.get_mut(&oc).unwrap();
                if q.seen.insert(id) {
                    let _ = q.tx.send(frame(0x27f3, &mine));
                }
            } else if d > VIEW {
                if self.players.get_mut(&cid).unwrap().seen.remove(&qid) {
                    self.send(cid, 0x273e, &remove_entity(qid));
                }
                let q = self.players.get_mut(&oc).unwrap();
                if q.seen.remove(&id) {
                    let _ = q.tx.send(frame(0x273e, &remove_entity(id)));
                }
            }
        }
        let World { players, d, mobs, mobs_by_map, .. } = self;
        let p = players.get_mut(&cid).unwrap();
        for n in d.npcs.get(&map).into_iter().flatten() {
            let dd = dist(x, y, n.x as i32, n.y as i32);
            if dd <= VIEW && p.seen.insert(n.id) {
                let _ = p.tx.send(frame(0x27e2, &npc_info(n)));
            } else if dd > VIEW {
                p.seen.remove(&n.id);
            }
        }
        for &i in mobs_by_map.get(&map).into_iter().flatten() {
            let m = &mobs[i];
            let dd = dist(x, y, m.x, m.y);
            if dd <= VIEW && !matches!(m.ai, Ai::Dead { .. }) && p.seen.insert(m.id) {
                let _ = p.tx.send(frame(0x27f3, &mob_info(m, &d.monsters[&m.typ])));
            } else if dd > VIEW && p.seen.remove(&m.id) {
                let _ = p.tx.send(frame(0x273e, &remove_entity(m.id)));
            }
        }
        self.ground_view(cid);
    }
}
