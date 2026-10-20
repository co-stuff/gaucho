/*oooooooooooooooooooo
 ~ with love from bolzov (づ◕‿◕)づ
 ~ https://github.com/ibolzov
 ~ gaucho - conquer mobile private server
 oooooooooooooooooooo*/

use super::*;
use crate::hero::BAG;

const REQUEST: u16 = 1;
const CLOSE: u16 = 2;
const OPEN: u16 = 3;
const HIDE: u16 = 5;
const ADD_ITEM: u16 = 6;
const SET_MONEY: u16 = 7;
const PARTNER_MONEY: u16 = 8;
const MY_MONEY: u16 = 9;
const ACCEPT: u16 = 10;
const ITEM_FAIL: u16 = 11;
const PARTNER_EMONEY: u16 = 12;
const SET_EMONEY: u16 = 13;
const MY_EMONEY: u16 = 14;
const REFUSE: u16 = 17;
const AUTO_REFUSE: u16 = 18;
const TRADE_ITEM: u8 = 2;

#[derive(Default)]
pub(super) struct Trade {
    with: u64,
    items: Vec<u32>,
    money: u64,
    emoney: u32,
    agreed: bool,
}

fn trade_msg(data: u64, action: u16) -> Vec<u8> {
    let mut b = data.to_le_bytes().to_vec();
    b.extend_from_slice(&action.to_le_bytes());
    b
}

impl World {
    pub(super) fn trade(&mut self, cid: u64, body: &[u8]) {
        if body.len() < 10 {
            return;
        }
        let data = u64::from_le_bytes(body[..8].try_into().unwrap());
        match u16::from_le_bytes([body[8], body[9]]) {
            REQUEST => self.trade_request(cid, data as u32),
            CLOSE => self.trade_close(cid),
            ADD_ITEM => self.trade_item(cid, data as u32),
            SET_MONEY => self.trade_money(cid, data, false),
            SET_EMONEY => self.trade_money(cid, data, true),
            ACCEPT => self.trade_accept(cid),
            REFUSE | AUTO_REFUSE => self.trade_refuse(cid),
            a => println!("[TRADE] {} action {a} data {data}", self.players[&cid].h.name),
        }
    }

    fn partner(&self, cid: u64) -> Option<u64> {
        self.players[&cid].trade.as_ref().map(|t| t.with)
    }

    fn trade_request(&mut self, cid: u64, target: u32) {
        let Some(&tc) = self.by_id.get(&target) else { return };
        let (me, them) = (&self.players[&cid], &self.players[&tc]);
        if tc == cid || me.trade.is_some() || them.trade.is_some() || me.h.map != them.h.map
            || dist(me.h.x, me.h.y, them.h.x, them.h.y) > VIEW {
            return;
        }
        let my_id = me.h.id;
        if self.players.get_mut(&cid).unwrap().trade_req.remove(&target) {
            self.players.get_mut(&cid).unwrap().trade = Some(Trade { with: tc, ..Default::default() });
            self.players.get_mut(&tc).unwrap().trade = Some(Trade { with: cid, ..Default::default() });
            self.send(cid, 0x2787, &trade_msg(target as u64, OPEN));
            self.send(tc, 0x2787, &trade_msg(my_id as u64, OPEN));
        } else {
            self.players.get_mut(&tc).unwrap().trade_req.insert(my_id);
            self.send(tc, 0x2787, &trade_msg(my_id as u64, REQUEST));
        }
    }

    fn trade_refuse(&mut self, cid: u64) {
        let p = self.players.get_mut(&cid).unwrap();
        let who: Vec<u32> = p.trade_req.drain().collect();
        let text = format!("{} declined the trade", p.h.name);
        for id in who {
            if let Some(&c) = self.by_id.get(&id) {
                self.send(c, 0x27c3, &talk(2005, &text));
            }
        }
    }

    pub(super) fn trade_close(&mut self, cid: u64) {
        let Some(tc) = self.partner(cid) else { return };
        let id = self.players[&cid].h.id as u64;
        for c in [cid, tc] {
            if let Some(p) = self.players.get_mut(&c) {
                p.trade = None;
            }
            self.send(c, 0x2787, &trade_msg(id, HIDE));
        }
    }

    fn unagree(&mut self, cid: u64, tc: u64) {
        for c in [cid, tc] {
            if let Some(t) = self.players.get_mut(&c).unwrap().trade.as_mut() {
                t.agreed = false;
            }
        }
    }

    fn trade_item(&mut self, cid: u64, id: u32) {
        let Some(tc) = self.partner(cid) else { return };
        let (me, them) = (&self.players[&cid], &self.players[&tc]);
        let t = me.trade.as_ref().unwrap();
        let it = me.h.items.iter().find(|i| i.id == id && i.pos == 0 && !i.bound).cloned();
        let room = them.h.items.iter().filter(|i| i.pos == 0).count() + t.items.len() < BAG;
        let Some(it) = it.filter(|_| room && !t.items.contains(&id)) else {
            return self.send(cid, 0x2787, &trade_msg(id as u64, ITEM_FAIL));
        };
        self.players.get_mut(&cid).unwrap().trade.as_mut().unwrap().items.push(id);
        self.unagree(cid, tc);
        self.send(tc, 0x2784, &item::item_info(&it, TRADE_ITEM, 0));
    }

    fn trade_money(&mut self, cid: u64, amount: u64, emoney: bool) {
        let Some(tc) = self.partner(cid) else { return };
        let p = self.players.get_mut(&cid).unwrap();
        let have = if emoney { p.h.emoney as u64 } else { p.h.money };
        let amount = amount.min(have);
        let t = p.trade.as_mut().unwrap();
        let (mine, theirs) = if emoney {
            t.emoney = amount as u32;
            (MY_EMONEY, PARTNER_EMONEY)
        } else {
            t.money = amount;
            (MY_MONEY, PARTNER_MONEY)
        };
        self.unagree(cid, tc);
        self.send(cid, 0x2787, &trade_msg(amount, mine));
        self.send(tc, 0x2787, &trade_msg(amount, theirs));
    }

    fn trade_accept(&mut self, cid: u64) {
        let Some(tc) = self.partner(cid) else { return };
        self.players.get_mut(&cid).unwrap().trade.as_mut().unwrap().agreed = true;
        self.send(tc, 0x2787, &trade_msg(self.players[&cid].h.id as u64, ACCEPT));
        if self.players[&tc].trade.as_ref().is_some_and(|t| t.agreed) {
            self.trade_finish(cid, tc);
        }
    }

    fn offer_ok(&self, giver: u64, taker: u64) -> bool {
        let (g, t) = (&self.players[&giver], &self.players[&taker]);
        let o = g.trade.as_ref().unwrap();
        let back = t.trade.as_ref().unwrap().items.len();
        o.items.iter().all(|id| g.h.items.iter().any(|i| i.id == *id && i.pos == 0 && !i.bound))
            && g.h.money >= o.money && g.h.emoney >= o.emoney
            && t.h.items.iter().filter(|i| i.pos == 0).count() + o.items.len() <= BAG + back
    }

    fn trade_finish(&mut self, a: u64, b: u64) {
        if !self.offer_ok(a, b) || !self.offer_ok(b, a) {
            return self.trade_close(a);
        }
        let offers: Vec<(u64, u64, Vec<u32>, u64, u32)> = [(a, b), (b, a)].iter().map(|&(g, t)| {
            let o = self.players[&g].trade.as_ref().unwrap();
            (g, t, o.items.clone(), o.money, o.emoney)
        }).collect();
        for (giver, taker, items, money, emoney) in offers {
            for id in items {
                let g = self.players.get_mut(&giver).unwrap();
                let count = g.h.items.iter().find(|i| i.id == id).map_or(0, |i| i.count);
                let Some(mut it) = g.h.take(id, count) else { continue };
                it.count = count;
                self.send(giver, 0x2799, &item::item_msg(id, 255, 3));
                let got = self.players.get_mut(&taker).unwrap().h.receive(it);
                self.send(taker, 0x2784, &item::item_info(&got, 1, 0));
            }
            let g = &mut self.players.get_mut(&giver).unwrap().h;
            g.money -= money;
            g.emoney -= emoney;
            let t = &mut self.players.get_mut(&taker).unwrap().h;
            t.money += money;
            t.emoney += emoney;
        }
        for c in [a, b] {
            let h = &self.players[&c].h;
            self.send(c, 0x27c0, &attrib(h.id, &[(4, h.money), (29, h.emoney as u64)]));
        }
        println!("[TRADE] {} <-> {} done", self.players[&a].h.name, self.players[&b].h.name);
        self.trade_close(a);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trade_msg_matches_capture() {
        assert_eq!(trade_msg(1279436, OPEN), [0xcc, 0x85, 0x13, 0, 0, 0, 0, 0, 3, 0]);
    }
}
