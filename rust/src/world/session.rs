/****************
 # with love from bolzov (◕‿◕)
 # https://github.com/ibolzov
 # gaucho - conquer mobile private server
 ****************/

use super::*;

fn owns(account: Option<&str>, ip: &str, h: &Hero) -> bool {
    match account {
        Some(a) => h.account == a,
        None => h.account.is_empty() && h.ip == ip,
    }
}

impl World {
    pub(super) fn hero_info(&self, cid: u64) -> Vec<u8> {
        let p = &self.players[&cid];
        let h = &p.h;
        let mut m = self.player_base.clone();
        for (f, v) in m.0.iter_mut() {
            if !matches!(f, 7 | 31) && matches!(v, crate::proto::Val::V(_)) {
                *v = crate::proto::Val::V(0);
            }
        }
        m.set(1, h.look as u64).set(2, h.id as u64).set(3, h.guild as u64).set(4, h.guild_rank as u64)
            .set(27, h.x as u64).set(28, h.y as u64).set(29, h.hair as u64).set(30, p.dir as u64)
            .set(34, h.reborn as u64).set(35, h.level as u64).set(42, 32550102).set(51, 8300)
            .set(72, h.class as u64).set(75, h.battle_power() as u64).set(112, 1)
            .set_nth(6, 0, status_word(p)).set_nth_bytes(107, 0, h.name.as_bytes());
        self.social_fields(h, &mut m);
        m.encode()
    }

    pub(super) fn join(&mut self, cid: u64, tx: Tx, addr: SocketAddr) {
        let t = &self.d.template;
        let h = Hero { map: t.map, x: t.pos.0 as i32, y: t.pos.1 as i32, ..Default::default() };
        let now = Instant::now();
        self.players.insert(cid, Player {
            tx, addr, h, key: String::new(), pending: None, dir: 0, alive: true, revive_at: None, entered: false,
            ready: false, seen: HashSet::new(), last_attack: now, last_mbtick: now, quest_timer: None, cine: None, res: 0,
            cooldown: HashMap::new(), stamina: 0, pk_mode: 1, flash: None, pk_decay: now + PK_DECAY,
            friend_req: HashSet::new(), trade_req: HashSet::new(), trade: None,
        });
    }

    pub(super) fn leave(&mut self, cid: u64) {
        if self.players.get(&cid).is_some_and(|p| p.ready) {
            self.trade_close(cid);
            self.friends_online(cid, false);
        }
        let Some(p) = self.players.remove(&cid) else { return };
        if p.h.id != 0 {
            self.store.put(&p.key, &p.h);
            self.store.flush();
        }
        println!("[WORLD] {} {} left ({} online)", p.h.name, p.addr, self.players.len());
        self.by_id.remove(&p.h.id);
        if p.h.id != 0 {
            self.forget(p.h.map, p.h.id);
            for m in &mut self.mobs {
                if m.ai == Ai::Chase(p.h.id) {
                    m.ai = Ai::Idle;
                }
            }
        }
    }

    pub(super) fn connect(&mut self, cid: u64, body: &[u8]) {
        let p = self.players.get_mut(&cid).unwrap();
        p.res = body.get(28..32).map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        if p.h.id != 0 {
            return;
        }
        let ip = p.addr.ip().to_string();
        if p.pending.is_none() && self.fresh.remove(&ip) {
            self.send(cid, 0x27c3, &talk(0x835, "NEW_ROLE"));
            return;
        }
        let online = |w: &World, k: &str| w.players.values().any(|q| q.h.id != 0 && q.key == k);
        let (picked, acc) = (self.pick.remove(&ip), self.account_of(cid));
        let key = self.players[&cid].pending.clone().or(picked)
            .or_else(|| self.store.latest(|h| owns(acc.as_deref(), &ip, h)).filter(|k| !online(self, k)).cloned())
            .filter(|k| !online(self, k));
        match key.and_then(|k| self.store.heroes.get(&k).cloned().map(|h| (k, h))) {
            Some((k, h)) => self.login(cid, k, h),
            None => self.send(cid, 0x27c3, &talk(0x835, "NEW_ROLE")),
        }
    }

    pub(super) fn account_command(&mut self, cid: u64, cmd: &str) -> bool {
        let mut words = cmd.split_whitespace();
        let verb = words.next().unwrap_or("").to_lowercase();
        if !matches!(verb.as_str(), "characters" | "switch") {
            return false;
        }
        let acc = self.account_of(cid);
        let p = &self.players[&cid];
        let ip = p.addr.ip().to_string();
        let mut mine: Vec<(&String, &Hero)> = self.store.heroes.iter().filter(|(_, h)| owns(acc.as_deref(), &ip, h)).collect();
        mine.sort_by_key(|(k, _)| k.as_str());
        let reply = if verb == "characters" {
            mine.iter().map(|(k, h)| format!("{}{} lv{} {}", if **k == p.key { "*" } else { "" }, h.name, h.level,
                self.classes.get(h.class).name)).collect::<Vec<_>>().join(" | ")
        } else {
            let want = words.next().map(str::to_lowercase);
            let at = mine.iter().position(|(k, _)| **k == p.key).unwrap_or(0);
            let next = match &want {
                Some(n) => mine.iter().find(|(k, _)| *k == n),
                None => mine.get((at + 1) % mine.len().max(1)),
            }.map(|(k, h)| ((*k).clone(), h.name.clone()));
            match next.filter(|(k, _)| *k != p.key) {
                Some((key, name)) => {
                    self.pick.insert(ip, key);
                    self.send(cid, 0x27c3, &talk(2005, &format!("Reconnect to play as {name}")));
                    self.leave(cid);
                    return true;
                }
                None => "You have no such character. See /characters".into(),
            }
        };
        self.send(cid, 0x27c3, &talk(2005, &reply));
        true
    }

    pub(super) fn register(&mut self, cid: u64, body: &[u8]) {
        if body.len() < 0x88 || body[..4] != [0, 0, 0, 0] {
            return;
        }
        let raw = &body[0x24..0x44];
        let name = String::from_utf8_lossy(&raw[..raw.iter().position(|&b| b == 0).unwrap_or(raw.len())]).trim().to_string();
        let index = u16::from_le_bytes([body[0x86], body[0x87]]);
        let ok = (2..=15).contains(&name.chars().count()) && name.chars().all(|c| c.is_alphanumeric() || c == '_');
        if !ok {
            self.send(cid, 0x27c3, &talk(0x834, "STR_ERROR_ILLEGAL_NAME@@"));
            return;
        }
        let key = name.to_lowercase();
        let ip = self.players[&cid].addr.ip().to_string();
        let acc = self.account_of(cid);
        let taken = self.store.heroes.get(&key).is_some_and(|h| !h.ip.is_empty() && !owns(acc.as_deref(), &ip, h));
        if taken || self.players.values().any(|q| q.h.id != 0 && q.key == key) {
            self.send(cid, 0x27c3, &talk(0x834, "STR_ERROR_DUPLICATE_NAME@@"));
            return;
        }
        if !self.store.heroes.contains_key(&key) {
            let t = &self.d.template;
            let (class, c) = self.classes.by_index(index).unwrap_or((t.class, self.classes.get(t.class)));
            let id = self.store.next_id(HERO_ID0);
            let mut h = Hero::new(id, &name, class, c, t.map, (t.pos.0 as i32, t.pos.1 as i32));
            h.look = c.looks.get(index as usize % 2).copied().unwrap_or(t.look);
            h.ip = ip;
            h.account = acc.unwrap_or_default();
            println!("[WORLD] created {} ({}) id={id}", name, c.name);
            self.store.put(&key, &h);
            self.store.flush();
        }
        self.players.get_mut(&cid).unwrap().pending = Some(key);
        self.send(cid, 0x27c3, &talk(0x834, "ANSWER_OK"));
    }

    pub(super) fn login(&mut self, cid: u64, key: String, mut h: Hero) {
        let t = &self.d.template;
        let c = self.classes.get(h.class);
        if h.map != t.map || h.x == 0 {
            (h.map, h.x, h.y) = (t.map, t.pos.0 as i32, t.pos.1 as i32);
        }
        if h.hp <= 0 {
            h.hp = h.max_hp(c);
        }
        let ui = h.userinfo(&t.userinfo);
        let setup: Vec<_> = data::retarget(&t.setup, (t.hero, t.name.as_slice()), (h.id, h.name.as_bytes()))
            .into_iter().map(|(ty, b)| match ty {
                0x27ed => (ty, ui.clone()),
                0x27f0 if Msg::parse_lossy(&b).get(1) == 0 => (ty, clock()),
                _ => (ty, b),
            }).filter(|(ty, b)| *ty != 0x27c3 || !matches!(Msg::parse_lossy(b).get(2), 0x834 | 0x835)).collect();
        let online = self.players.len();
        let p = self.players.get_mut(&cid).unwrap();
        h.ip = p.addr.ip().to_string();
        h.last_login = unix();
        println!("[WORLD] {} id={} logged in from {} ({} online)", h.name, h.id, p.addr, online);
        self.by_id.insert(h.id, cid);
        p.key = key;
        p.pending = None;
        p.h = h;
        for (ty, b) in &setup {
            self.send(cid, *ty, b);
        }
        self.send_skills(cid);
        self.send(cid, 0x27c3, &talk(0x835, "ANSWER_OK"));
    }

    pub(super) fn enter(&mut self, cid: u64) {
        let p = &self.players[&cid];
        let (id, map, x, y) = (p.h.id as u64, p.h.map as u64, p.h.x as u64, p.h.y as u64);
        self.send(cid, 0x273e, &Pb::new().v(1, id).v(3, map).v(5, id).v(9, ms()).v(12, 86).v(13, 0)
            .v(14, x).v(15, y).v(17, map).out());
        let t = &self.d.template;
        let p = &self.players[&cid];
        for (ty, b) in data::retarget(&t.enter, (t.hero, t.name.as_slice()), (p.h.id, p.h.name.as_bytes())) {
            self.send(cid, ty, &b);
        }
        self.players.get_mut(&cid).unwrap().entered = true;
    }

    pub(super) fn ready(&mut self, cid: u64) {
        let t = &self.d.template;
        let p = &self.players[&cid];
        for (ty, b) in data::retarget(&t.after, (t.hero, t.name.as_slice()), (p.h.id, p.h.name.as_bytes())) {
            match ty {
                0x2784 => self.send_items(cid),
                0x2799 if b.get(12) == Some(&46) => self.send(cid, ty, &item::equipment(&p.h.items)),
                _ => self.send(cid, ty, &b),
            }
        }
        self.sync_attribs(cid);
        self.send_status(cid);
        self.social_ready(cid);
        self.friends_online(cid, true);
        self.skills_ready(cid);
        self.players.get_mut(&cid).unwrap().ready = true;
        self.refresh_view(cid);
        self.quest_login(cid);
    }

    pub(super) fn sync_attribs(&self, cid: u64) {
        let h = &self.players[&cid].h;
        let c = self.classes.get(h.class);
        let st = |i: usize| h.stats[i] as u64;
        self.send(cid, 0x27c0, &attrib(h.id, &[
            (1, h.max_hp(c) as u64), (0, h.hp.max(0) as u64), (3, h.max_mp(c) as u64), (2, h.mp.max(0) as u64),
            (12, h.level as u64), (15, st(0)), (16, st(1)), (14, st(2)), (13, st(3)), (10, h.add_points as u64),
            (11, h.look as u64), (26, h.hair as u64), (4, h.money), (29, h.emoney as u64), (5, h.exp), (6, h.pk as u64),
        ]));
        self.send(cid, 0x283d, &h.attri_info(c));
    }

    pub(super) fn attri_query(&self, cid: u64, body: &[u8]) {
        let want = body.get(..4).map_or(0, |b| u32::from_le_bytes(b.try_into().unwrap()));
        let me = self.players[&cid].h.id;
        let target = self.by_id.get(&if want == 0 { me } else { want }).and_then(|c| self.players.get(c));
        if let Some(t) = target {
            self.send(cid, 0x283d, &t.h.attri_info(self.classes.get(t.h.class)));
        }
    }
}
