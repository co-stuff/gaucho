/*~~~~~~~~~~~
 : gaucho - conquer mobile private server
 : with love from bolzov (人◕‿◕)
 : https://github.com/ibolzov
 ~~~~~~~~~~~*/

use super::*;

impl World {
    pub(super) fn npc_talk(&mut self, cid: u64, body: &[u8]) {
        let m = Msg::parse_lossy(body);
        let npc = m.get(2);
        println!("[WORLD] {} talks to npc {npc}", self.players[&cid].h.name);
        if self.quest_npc(cid, npc as u32, m.get(3)) {
            return;
        }
        for (opt, kind, text) in [
            (0xff, 1, "Greetings, traveler."),
            (1, 2, "Heal me"),
            (2, 2, "Goodbye"),
            (0xff, 100, ""),
        ] {
            self.send(cid, 0x280f, &dialog(opt, kind, text));
        }
    }

    pub(super) fn npc_answer(&mut self, cid: u64, body: &[u8]) {
        let (opt, kind) = (body.get(6).copied().unwrap_or(0), body.get(7).copied().unwrap_or(0));
        if kind == 101 && opt == 1 {
            let max = { let h = &self.players[&cid].h; h.max_hp(self.classes.get(h.class)) };
            let p = self.players.get_mut(&cid).unwrap();
            if p.alive {
                p.h.hp = max;
                let id = p.h.id;
                self.send(cid, 0x27c0, &attrib(id, &[(0, max as u64)]));
            }
        }
    }

    pub(super) fn chat(&mut self, cid: u64, body: &[u8]) {
        let m = Msg::parse_lossy(body);
        let text = String::from_utf8_lossy(m.bytes(13, 3).unwrap_or_default()).into_owned();
        if text.is_empty() || self.command(cid, &text) {
            return;
        }
        let p = &self.players[&cid];
        let (channel, to) = (m.get(2), m.bytes(13, 1).unwrap_or_default());
        println!("[CHAT] {} ch{channel}: {text}", p.h.name);
        if channel == 2001 && !self.players.values().any(|q| q.h.id != 0 && q.h.name.as_bytes() == to) {
            return self.send(cid, 0x27c3, &talk(2005, &format!("{} is offline", String::from_utf8_lossy(to))));
        }
        let f = frame(0x27c3, body);
        for q in self.players.values().filter(|q| q.h.id != 0 && q.h.id != p.h.id) {
            let hear = match channel {
                2000 => q.h.map == p.h.map && q.seen.contains(&p.h.id),
                2001 => q.h.name.as_bytes() == to,
                2003 => false,
                2004 => p.h.guild != 0 && q.h.guild == p.h.guild,
                _ => true,
            };
            if hear {
                let _ = q.tx.send(f.clone());
            }
        }
    }
}
