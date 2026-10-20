/*~~~~~~~~~~~~~~~~~~~~
 ~ gaucho - conquer mobile private server
 ~ with love from bolzov ٩(◕‿◕)۶
 ~ https://github.com/ibolzov
 ~~~~~~~~~~~~~~~~~~~~*/

use super::*;

const REQUEST: u8 = 10;
const NEW_FRIEND: u8 = 11;
const ONLINE: u8 = 12;
const OFFLINE: u8 = 13;
const REMOVE: u8 = 14;
const ADD: u8 = 15;
const LIST: u8 = 22;

fn friend_msg(action: u8, h: &Hero, online: bool) -> Vec<u8> {
    let mut b = vec![0u8; 72];
    b[..4].copy_from_slice(&h.id.to_le_bytes());
    b[4] = action;
    b[5] = online as u8;
    b[12..16].copy_from_slice(&h.class.to_le_bytes());
    b[16..20].copy_from_slice(&h.look.to_le_bytes());
    b[20..24].copy_from_slice(&h.level.to_le_bytes());
    let n = h.name.len().min(31);
    b[36..36 + n].copy_from_slice(&h.name.as_bytes()[..n]);
    b
}

fn removal(id: u32) -> Vec<u8> {
    let mut b = vec![0u8; 72];
    b[..4].copy_from_slice(&id.to_le_bytes());
    b[4] = REMOVE;
    b
}

impl World {
    pub(super) fn friend(&mut self, cid: u64, body: &[u8]) {
        let target = body.get(..4).map_or(0, |b| u32::from_le_bytes(b.try_into().unwrap()));
        match body.get(4).copied().unwrap_or(0) {
            REQUEST | NEW_FRIEND => self.friend_request(cid, target),
            REMOVE => self.friend_remove(cid, target),
            LIST => self.friend_list(cid),
            a => println!("[FRIEND] {} action {a} target {target}", self.players[&cid].h.name),
        }
    }

    fn friend_request(&mut self, cid: u64, target: u32) {
        let me = &self.players[&cid].h;
        if target == me.id || me.friends.contains(&target) {
            return;
        }
        let Some(&tc) = self.by_id.get(&target) else {
            return self.send(cid, 0x27c3, &talk(2005, "That player is offline"));
        };
        let my_id = me.id;
        if self.players.get_mut(&cid).unwrap().friend_req.remove(&target) {
            self.players.get_mut(&cid).unwrap().h.friends.push(target);
            self.players.get_mut(&tc).unwrap().h.friends.push(my_id);
            let (a, b) = (&self.players[&cid].h, &self.players[&tc].h);
            println!("[FRIEND] {} <-> {}", a.name, b.name);
            for (to, h) in [(cid, b), (tc, a)] {
                self.send(to, 0x279b, &friend_msg(NEW_FRIEND, h, true));
                self.send(to, 0x279b, &friend_msg(ONLINE, h, true));
            }
        } else {
            self.players.get_mut(&tc).unwrap().friend_req.insert(my_id);
            self.send(tc, 0x279b, &friend_msg(REQUEST, &self.players[&cid].h, true));
        }
    }

    fn friend_remove(&mut self, cid: u64, target: u32) {
        let me = self.players.get_mut(&cid).unwrap();
        me.h.friends.retain(|&f| f != target);
        let my_id = me.h.id;
        self.send(cid, 0x279b, &removal(target));
        match self.by_id.get(&target).copied() {
            Some(tc) => {
                self.players.get_mut(&tc).unwrap().h.friends.retain(|&f| f != my_id);
                self.send(tc, 0x279b, &removal(my_id));
            }
            None => {
                let Some((key, mut h)) = self.store.heroes.iter().find(|(_, h)| h.id == target).map(|(k, h)| (k.clone(), h.clone()))
                else { return };
                h.friends.retain(|&f| f != my_id);
                self.store.put(&key, &h);
            }
        }
    }

    fn friend_list(&self, cid: u64) {
        for &id in &self.players[&cid].h.friends {
            match self.by_id.get(&id) {
                Some(c) => {
                    self.send(cid, 0x279b, &friend_msg(ADD, &self.players[c].h, true));
                    self.send(cid, 0x279b, &friend_msg(ONLINE, &self.players[c].h, true));
                }
                None => {
                    if let Some(h) = self.store.heroes.values().find(|h| h.id == id) {
                        self.send(cid, 0x279b, &friend_msg(ADD, h, false));
                    }
                }
            }
        }
    }

    pub(super) fn friends_online(&self, cid: u64, online: bool) {
        let me = &self.players[&cid].h;
        let msg = friend_msg(if online { ONLINE } else { OFFLINE }, me, online);
        for id in &me.friends {
            if let Some(&c) = self.by_id.get(id) {
                self.send(c, 0x279b, &msg);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn friend_msg_layout() {
        let h = Hero { id: 1300002, class: 16000, look: 5412007, level: 7, name: "Bot1".into(), ..Default::default() };
        let b = friend_msg(NEW_FRIEND, &h, true);
        assert_eq!((b.len(), &b[..6], &b[36..41]), (72, &[0x22, 0xd6, 0x13, 0, 11, 1][..], &b"Bot1\0"[..]));
        assert_eq!(u32::from_le_bytes(b[20..24].try_into().unwrap()), 7);
    }
}
