/*ooooooooooooooo
 > https://github.com/ibolzov
 > gaucho - conquer mobile private server
 > with love from bolzov (◡‿◡✿)
 ooooooooooooooo*/

use super::*;
use crate::hero::Item;

const GIFT: u8 = 48;
const SLOTS: u8 = 15;
const VIEW_EQUIP: u8 = 4;
const GATES: [(u32, u32, i32, i32); 5] = [
    (1060020, 1002, 430, 378),
    (1060021, 1000, 496, 650),
    (1060022, 1020, 566, 565),
    (1060023, 1011, 193, 266),
    (1060024, 1015, 717, 577),
];
const EXP_BALLS: [u32; 4] = [723700, 723834, 723911, 722136];

pub(super) fn item_info(it: &Item, action: u8, reason: u8) -> Vec<u8> {
    let mut b = vec![0u8; 211];
    b[..4].copy_from_slice(&it.id.to_le_bytes());
    b[4..8].copy_from_slice(&it.typ.to_le_bytes());
    b[8..10].copy_from_slice(&it.dur.to_le_bytes());
    b[10..12].copy_from_slice(&it.dur.to_le_bytes());
    b[12] = action;
    b[14] = it.pos;
    b[28] = if it.bound { 3 } else { 0 };
    b[50..54].copy_from_slice(&it.count.to_le_bytes());
    b[160] = reason;
    b
}

fn item_view(it: &Item, owner: u32, name: &str) -> Vec<u8> {
    let mut b = vec![0u8; 229];
    b[..4].copy_from_slice(&it.id.to_le_bytes());
    b[4..8].copy_from_slice(&owner.to_le_bytes());
    b[16..20].copy_from_slice(&it.typ.to_le_bytes());
    b[20..22].copy_from_slice(&it.dur.to_le_bytes());
    b[22..24].copy_from_slice(&it.dur.to_le_bytes());
    b[24] = VIEW_EQUIP;
    b[26] = it.pos;
    b[62] = 1;
    b[80..84].copy_from_slice(&owner.to_le_bytes());
    let n = name.len().min(31);
    b[84..84 + n].copy_from_slice(&name.as_bytes()[..n]);
    b
}

pub(super) fn item_msg(id: u32, data: u32, action: u8) -> Vec<u8> {
    let mut b = vec![0u8; 87];
    b[..4].copy_from_slice(&id.to_le_bytes());
    b[4..8].copy_from_slice(&data.to_le_bytes());
    b[12] = action;
    b
}

pub(super) fn equipment(items: &[Item]) -> Vec<u8> {
    let mut b = item_msg(0, 0, 46);
    for it in items.iter().filter(|i| (1..=SLOTS).contains(&i.pos)) {
        let at = 23 + 4 * it.pos as usize;
        b[at..at + 4].copy_from_slice(&it.id.to_le_bytes());
    }
    b
}

fn reward_line(typ: u32, count: u32) -> String {
    let gift = format!("<STR_ITEM_TYPE_{typ}@@>@@<STR_ID_tRewardTemplate[Gift]@@>@@");
    if count == 1 {
        format!("STR_ID_tRewardTemplate[Currency]@@<STR_ID_tLuaRes[10025]@@{gift}>@@")
    } else {
        format!("STR_ID_tRewardTemplate[Currency]@@<STR_ID_tLuaRes[10026]@@<STR_ID_tLuaRes[10025]@@{gift}>@@<STR_ID_tRewardTemplate[Number]@@>@@{count}@@>@@")
    }
}

impl World {
    pub(super) fn view_equipment(&self, cid: u64, target: u32) {
        let Some(t) = self.by_id.get(&target).and_then(|c| self.players.get(c)) else { return };
        for it in t.h.items.iter().filter(|i| i.pos != 0) {
            self.send(cid, 0x2823, &item_view(it, t.h.id, &t.h.name));
        }
    }

    pub(super) fn send_items(&self, cid: u64) {
        let items = &self.players[&cid].h.items;
        for it in items {
            self.send(cid, 0x2784, &item_info(it, 1, 0));
        }
    }

    pub(super) fn give_item(&mut self, cid: u64, typ: u32, count: u32, dur: u16) {
        if count == 0 {
            return;
        }
        let h = &mut self.players.get_mut(&cid).unwrap().h;
        let id = h.id;
        for (it, new) in h.add_gift(typ, count, dur) {
            if new {
                self.send(cid, 0x2784, &item_info(&it, 1, GIFT));
                self.send(cid, 0x2784, &item_info(&it, 3, 0));
            } else {
                self.send(cid, 0x2784, &item_info(&it, 3, GIFT));
            }
        }
        let list = vec![typ.to_string(); count as usize].join(",");
        let fly = format!("ClientFlyItemToHero_Open|{{['Data']={{['List']={{{list}}}}},['Code']=200}}");
        self.send(cid, 0x273e, &quest::lua(id, &fly));
        self.send(cid, 0x27c3, &talk(2005, &reward_line(typ, count)));
    }

    pub(super) fn download_reward(&mut self, cid: u64) {
        let h = &mut self.players.get_mut(&cid).unwrap().h;
        let (id, first) = (h.id, !std::mem::replace(&mut h.download_reward, true));
        if first {
            for (typ, count) in [(1088000, 1), (1088001, 1), (3342879, 3)] {
                self.give_item(cid, typ, count, 1);
            }
        }
        self.send(cid, 0x273e, &quest::lua(id, "SubPackUserDialgShow|1"));
    }

    pub(super) fn use_item(&mut self, cid: u64, body: &[u8]) {
        let at = |o: usize| body.get(o..o + 4).map_or(0, |b| u32::from_le_bytes(b.try_into().unwrap()));
        let act = body.get(12..14).map_or(0, |b| u16::from_le_bytes([b[0], b[1]]));
        let (id, data, count) = (at(0), at(4), at(18));
        match act {
            1 => self.buy(cid, id, data, count),
            2 => self.sell(cid, id, data),
            4 if data == 0 => self.consume(cid, id),
            4 => self.equip_item(cid, id, data),
            6 => self.unequip_item(cid, id),
            37 => self.discard(cid, id),
            _ => println!("[ITEM] {} act {act} id {id} data {data} count {count}", self.players[&cid].h.name),
        }
    }

    fn equip_item(&mut self, cid: u64, id: u32, pos: u32) {
        let h = &mut self.players.get_mut(&cid).unwrap().h;
        let Some(moved) = h.equip(id, pos.min(255) as u8) else { return };
        let eq = equipment(&h.items);
        for it in &moved {
            self.send(cid, 0x2784, &item_info(it, 3, 0));
        }
        self.send(cid, 0x2799, &item_msg(id, pos, 5));
        self.send(cid, 0x2799, &eq);
        self.send(cid, 0x273e, &Pb::new().v(3, 0).v(4, 0).v(9, ms()).v(12, 472).out());
    }

    fn unequip_item(&mut self, cid: u64, id: u32) {
        let h = &mut self.players.get_mut(&cid).unwrap().h;
        let Some(pos) = h.items.iter().find(|i| i.id == id && i.pos != 0).map(|i| i.pos) else { return };
        if h.bag_full() {
            return;
        }
        let Some(it) = h.unequip(id) else { return };
        let eq = equipment(&h.items);
        self.send(cid, 0x2799, &item_msg(id, pos as u32, 6));
        self.send(cid, 0x2784, &item_info(&it, 3, 0));
        self.send(cid, 0x2799, &eq);
    }

    fn send_taken(&self, cid: u64, it: &Item) {
        if it.count == 0 {
            self.send(cid, 0x2799, &item_msg(it.id, 255, 3));
        } else {
            self.send(cid, 0x2784, &item_info(it, 3, 0));
        }
    }

    fn consume(&mut self, cid: u64, id: u32) {
        let p = &self.players[&cid];
        let Some(typ) = p.h.items.iter().find(|i| i.id == id && i.pos == 0).map(|i| i.typ) else { return };
        let c = self.classes.get(p.h.class);
        let (max_hp, max_mp) = (p.h.max_hp(c), p.h.max_mp(c));
        let (life, mana) = self.d.items.heal(typ);
        let gate = GATES.iter().find(|g| g.0 == typ).copied();
        let ball = EXP_BALLS.contains(&typ);
        let useful = (life > 0 && p.h.hp < max_hp) || (mana > 0 && p.h.mp < max_mp) || gate.is_some() || ball;
        if !useful || !p.alive {
            return;
        }
        let h = &mut self.players.get_mut(&cid).unwrap().h;
        let Some(it) = h.take(id, 1) else { return };
        h.hp = (h.hp + life).min(max_hp);
        h.mp = (h.mp + mana).min(max_mp);
        let (hid, hp, mp) = (h.id, h.hp.max(0) as u64, h.mp.max(0) as u64);
        self.send_taken(cid, &it);
        if life > 0 || mana > 0 {
            self.send(cid, 0x27c0, &attrib(hid, &[(0, hp), (2, mp)]));
        }
        if let Some((_, map, x, y)) = gate {
            self.change_map(cid, map, x, y);
        }
        if ball {
            let exp = self.players[&cid].h.exp_to_next();
            self.give_exp(cid, exp);
        }
    }

    fn near_shop(&self, cid: u64, npc: u32) -> bool {
        let h = &self.players[&cid].h;
        self.d.items.shops.contains_key(&npc) && self.d.npcs.get(&h.map).into_iter().flatten()
            .any(|n| n.id == npc && dist(h.x, h.y, n.x as i32, n.y as i32) <= VIEW)
    }

    fn buy(&mut self, cid: u64, npc: u32, typ: u32, count: u32) {
        let count = count.clamp(1, 99);
        let cost = self.d.items.price(typ) as u64 * count as u64;
        let sells = self.d.items.shops.get(&npc).is_some_and(|s| s.contains(&typ));
        if !sells || !self.near_shop(cid, npc) {
            return;
        }
        let h = &self.players[&cid].h;
        let refusal = if h.money < cost { "Not enough money." } else if h.bag_full() { "Your bag is full." } else { "" };
        if !refusal.is_empty() {
            self.send(cid, 0x27c3, &talk(2005, refusal));
            return;
        }
        let dur = self.d.items.dur(typ);
        let h = &mut self.players.get_mut(&cid).unwrap().h;
        h.money -= cost;
        let (hid, money) = (h.id, h.money);
        for (it, new) in h.add_item(typ, count, dur) {
            self.send(cid, 0x2784, &item_info(&it, if new { 1 } else { 3 }, 0));
        }
        self.send(cid, 0x27c0, &attrib(hid, &[(4, money)]));
    }

    fn sell(&mut self, cid: u64, npc: u32, id: u32) {
        if !self.near_shop(cid, npc) {
            return;
        }
        let h = &mut self.players.get_mut(&cid).unwrap().h;
        let Some(count) = h.items.iter().find(|i| i.id == id && i.pos == 0 && !i.bound).map(|i| i.count) else { return };
        let Some(it) = h.take(id, count) else { return };
        h.money += self.d.items.price(it.typ) as u64 / 3 * count as u64;
        let (hid, money) = (h.id, h.money);
        self.send_taken(cid, &it);
        self.send(cid, 0x27c0, &attrib(hid, &[(4, money)]));
    }

    fn discard(&mut self, cid: u64, id: u32) {
        let p = self.players.get_mut(&cid).unwrap();
        let Some(count) = p.h.items.iter().find(|i| i.id == id && i.pos == 0).map(|i| i.count) else { return };
        let Some(mut it) = p.h.take(id, count) else { return };
        let (map, x, y) = (p.h.map, p.h.x, p.h.y);
        it.count = count;
        self.send(cid, 0x2799, &item_msg(id, 255, 3));
        if !it.bound {
            self.spawn_floor(map, (x, y), (x, y), it, 0, 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
    }

    #[test]
    fn item_packets_match_capture() {
        let mut sword = Item { id: 14663376, typ: 608007, dur: 1698, count: 1, pos: 0, bound: true };
        let og = unhex("d0bedf0007470900a206a2060100000000000000000000000000000003000000");
        let b = item_info(&sword, 1, GIFT);
        assert_eq!((b.len(), &b[..32], b[50], b[160]), (211, &og[..], 1, 0x30));
        assert_eq!(item_msg(14663376, 4, 5)[..14], unhex("d0bedf000400000000000000050000")[..14]);
        sword.pos = 4;
        let eq = equipment(&[sword]);
        assert_eq!((eq.len(), eq[12], &eq[39..43]), (87, 46, &unhex("d0bedf00")[..]));
        assert_eq!(reward_line(1088001, 2), "STR_ID_tRewardTemplate[Currency]@@<STR_ID_tLuaRes[10026]@@<STR_ID_tLuaRes[10025]@@<STR_ITEM_TYPE_1088001@@>@@<STR_ID_tRewardTemplate[Gift]@@>@@>@@<STR_ID_tRewardTemplate[Number]@@>@@2@@>@@");
        assert_eq!(reward_line(1088000, 1), "STR_ID_tRewardTemplate[Currency]@@<STR_ID_tLuaRes[10025]@@<STR_ITEM_TYPE_1088000@@>@@<STR_ID_tRewardTemplate[Gift]@@>@@>@@");
    }
}
