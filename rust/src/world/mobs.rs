/*~~~~~~~~~~~~~~~~~~~~~~
 · gaucho - conquer mobile private server
 · with love from bolzov ٩(◕‿◕)۶
 · https://github.com/ibolzov
 ~~~~~~~~~~~~~~~~~~~~~~*/

use super::*;

const GUARD_IDLE: Duration = Duration::from_millis(500);
const WANTED: u64 = 0x1 | 0x8000;

impl World {
    pub(super) fn tick(&mut self) {
        let now = Instant::now();
        let mut revive = Vec::new();
        for (&cid, p) in &mut self.players {
            if p.revive_at.is_some_and(|t| now >= t) {
                revive.push(cid);
            }
            if p.h.id != 0 && now.duration_since(p.last_mbtick) >= Duration::from_secs(30) {
                p.last_mbtick = now;
                let mut b = vec![0u8; 28];
                b[..4].copy_from_slice(&p.h.id.to_le_bytes());
                let _ = p.tx.send(frame(0x278a, &b));
            }
        }
        self.stamina_tick(now);
        self.pk_tick(now);
        for cid in revive {
            self.revive(cid);
        }
        self.quest_tick(now);
        self.ground_tick(now);
        if now.duration_since(self.last_save) >= Duration::from_secs(30) {
            self.last_save = now;
            for p in self.players.values().filter(|p| p.h.id != 0) {
                self.store.put(&p.key, &p.h);
            }
            self.store.flush();
        }

        let mut heroes: HashMap<u32, Vec<(u32, i32, i32, bool)>> = HashMap::new();
        for p in self.players.values().filter(|p| p.ready) {
            heroes.entry(p.h.map).or_default().push((p.h.id, p.h.x, p.h.y, p.alive));
        }
        for (map, hs) in heroes {
            let idx = self.mobs_by_map.get(&map).cloned().unwrap_or_default();
            for i in idx {
                self.mob_ai(i, &hs, now);
            }
        }
    }

    pub(super) fn mob_ai(&mut self, i: usize, hs: &[(u32, i32, i32, bool)], now: Instant) {
        let m = &self.mobs[i];
        let (map, id, mx, my) = (m.map, m.id, m.x, m.y);
        if m.guard {
            if hs.iter().any(|h| dist(h.1, h.2, mx, my) <= VIEW + 6) {
                self.guard_ai(i, hs, now);
            }
            return;
        }
        match m.ai {
            Ai::Dead { fade, respawn, faded } => {
                if !faded && now >= fade {
                    self.send_view(map, id, 0x27c0, &status(id, 0x820, 0));
                    self.forget(map, id);
                    self.mobs[i].ai = Ai::Dead { fade, respawn, faded: true };
                }
                if now >= respawn {
                    self.respawn(i, now);
                }
            }
            _ if !hs.iter().any(|h| dist(h.1, h.2, mx, my) <= VIEW + 6) => {}
            Ai::Idle => {
                if now < m.next_move {
                    return;
                }
                let t = &self.d.monsters[&m.typ];
                let aggro = t.view.clamp(0, 8);
                let near = hs.iter().filter(|h| h.3 && dist(h.1, h.2, mx, my) <= aggro)
                    .min_by_key(|h| dist(h.1, h.2, mx, my));
                let mut rng = rand::thread_rng();
                if let Some(h) = near {
                    self.mobs[i].ai = Ai::Chase(h.0);
                    let info = frame(0x27f3, &mob_info(&self.mobs[i], t));
                    if let Some(p) = self.by_id.get(&h.0).and_then(|c| self.players.get_mut(c)) {
                        p.seen.insert(id);
                        let _ = p.tx.send(info);
                    }
                    return;
                }
                let (home, wait) = (m.home, rng.gen_range(2000..5000));
                self.mobs[i].next_move = now + Duration::from_millis(wait);
                if rng.gen_bool(0.4) {
                    let d = rng.gen_range(0..8u8);
                    let (dx, dy) = DIRS[d as usize];
                    if dist(mx + dx, my + dy, home.0, home.1) <= 5 {
                        self.step_mob(i, d);
                    }
                }
            }
            Ai::Chase(pid) => {
                let t = &self.d.monsters[&m.typ];
                let (range, atk, atk_ms, move_ms) = (t.range, t.atk, t.atk_ms, t.move_ms);
                let target = hs.iter().find(|h| h.0 == pid && h.3);
                let Some(&(_, tx, ty, _)) = target.filter(|h| dist(h.1, h.2, mx, my) <= 15) else {
                    self.mobs[i].ai = Ai::Idle;
                    self.mobs[i].next_move = now + Duration::from_secs(1);
                    return;
                };
                if dist(tx, ty, mx, my) <= range {
                    if now >= m.next_attack {
                        self.mobs[i].next_attack = now + Duration::from_millis(atk_ms);
                        if let Some(&tc) = self.by_id.get(&pid) {
                            let v = &self.players[&tc].h;
                            let def = v.defense(self.classes.get(v.class));
                            let dmg = if rand::thread_rng().gen_bool(0.85) { (roll(atk) - def).max(1) } else { 0 };
                            self.damage_player(tc, id, mx, my, dmg);
                        }
                    }
                } else if now >= m.next_move {
                    self.mobs[i].next_move = now + Duration::from_millis(move_ms);
                    let (sx, sy) = ((tx - mx).signum(), (ty - my).signum());
                    let d = DIRS.iter().position(|&v| v == (sx, sy)).unwrap_or(0) as u8;
                    self.step_mob(i, d);
                }
            }
        }
    }

    fn guard_ai(&mut self, i: usize, hs: &[(u32, i32, i32, bool)], now: Instant) {
        let g = &self.mobs[i];
        if now < g.next_attack {
            return;
        }
        let t = &self.d.monsters[&g.typ];
        let (gid, map, gx, gy, reach, atk, atk_ms) = (g.id, g.map, g.x, g.y, t.range.clamp(1, t.view.max(1)), t.atk, t.atk_ms);
        let mobs = &self.mobs;
        let prey = self.mobs_by_map.get(&map).into_iter().flatten().copied()
            .filter(|&j| !mobs[j].guard && !matches!(mobs[j].ai, Ai::Dead { .. }) && dist(mobs[j].x, mobs[j].y, gx, gy) <= reach)
            .min_by_key(|&j| dist(mobs[j].x, mobs[j].y, gx, gy));
        let criminal = || hs.iter().filter(|h| h.3 && dist(h.1, h.2, gx, gy) <= reach)
            .filter_map(|h| self.by_id.get(&h.0).copied())
            .find(|c| status_word(&self.players[c]) & WANTED != 0);
        if let Some(j) = prey {
            let (mid, def) = (self.mobs[j].id, self.d.monsters[&self.mobs[j].typ].def);
            let dmg = (roll(atk) - def).max(1);
            self.send_view(map, mid, 0x2797, &hit(gid, mid, gx, gy, dmg));
            self.strike_mob(j, gid, dmg);
        } else if let Some(c) = criminal() {
            let v = &self.players[&c].h;
            let dmg = (roll(atk) - v.defense(self.classes.get(v.class))).max(1);
            self.damage_player(c, gid, gx, gy, dmg);
        } else {
            self.mobs[i].next_attack = now + GUARD_IDLE;
            return;
        }
        self.mobs[i].next_attack = now + Duration::from_millis(atk_ms);
    }

    pub(super) fn step_mob(&mut self, i: usize, dir: u8) {
        let World { players, mobs, d, .. } = self;
        let m = &mut mobs[i];
        let (dx, dy) = DIRS[dir as usize];
        m.x += dx;
        m.y += dy;
        m.dir = dir;
        let w = frame(0x27dd, &walk(m.id, dir));
        for p in players.values_mut().filter(|p| p.ready && p.h.map == m.map) {
            let dd = dist(p.h.x, p.h.y, m.x, m.y);
            if p.seen.contains(&m.id) {
                if dd > VIEW {
                    p.seen.remove(&m.id);
                    let _ = p.tx.send(frame(0x273e, &remove_entity(m.id)));
                } else {
                    let _ = p.tx.send(w.clone());
                }
            } else if dd <= VIEW {
                p.seen.insert(m.id);
                let _ = p.tx.send(frame(0x27f3, &mob_info(m, &d.monsters[&m.typ])));
            }
        }
    }

    pub(super) fn respawn(&mut self, i: usize, now: Instant) {
        let World { players, mobs, d, .. } = self;
        let m = &mut mobs[i];
        let t = &d.monsters[&m.typ];
        let mut rng = rand::thread_rng();
        let a = m.area;
        (m.x, m.y) = (a.0 + rng.gen_range(0..a.2), a.1 + rng.gen_range(0..a.3));
        m.home = (m.x, m.y);
        m.hp = t.life;
        m.ai = Ai::Idle;
        m.next_move = now + Duration::from_secs(2);
        let (info, fx) = (frame(0x27f3, &mob_info(m, t)), frame(0x273e, &spawn_fx(m.id, m.dir, m.x, m.y)));
        for p in players.values_mut().filter(|p| p.ready && p.h.map == m.map) {
            if dist(p.h.x, p.h.y, m.x, m.y) <= VIEW && p.seen.insert(m.id) {
                let _ = p.tx.send(info.clone());
                let _ = p.tx.send(fx.clone());
            }
        }
    }
}
