/*•••••••••••••••••
 : https://github.com/ibolzov
 : gaucho - conquer mobile private server
 : with love from bolzov (づ◕‿◕)づ
 •••••••••••••••••*/

use super::*;
use crate::hero::Item;

const OWNED: Duration = Duration::from_secs(30);
const LAST: Duration = Duration::from_secs(60);
const PICK: i32 = 5;
const GROUND0: u32 = 7_700_000;
const SPOTS: [(i32, i32); 9] = [(0, 0), (1, 0), (0, 1), (-1, 0), (0, -1), (1, 1), (-1, -1), (1, -1), (-1, 1)];

pub struct Floor {
    id: u32,
    item: Item,
    money: u32,
    map: u32,
    x: i32,
    y: i32,
    from: (i32, i32),
    owner: u32,
    born: Instant,
}

impl Floor {
    fn takeable(&self, by: u32, now: Instant) -> bool {
        self.owner == 0 || self.owner == by || now.duration_since(self.born) >= OWNED
    }
}

#[derive(Default)]
pub struct Ground {
    items: HashMap<u32, Floor>,
    next: u32,
}

fn money_type(amount: u32) -> u32 {
    match amount {
        0..=9 => 1090000,
        10..=99 => 1090010,
        _ => 1090020,
    }
}

fn hp_potion(level: u32) -> u32 {
    match level {
        0..=19 => 1000000,
        20..=39 => 1000030,
        40..=69 => 1002010,
        _ => 1002020,
    }
}

fn loot(level: u32, rng: &mut impl Rng) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    if rng.gen_bool(0.55) {
        let m = rng.gen_range(level / 2 + 1..=level + 3);
        out.push((money_type(m), m));
    }
    if rng.gen_bool(0.06) {
        out.push((hp_potion(level), 0));
    }
    if rng.gen_bool(0.03) {
        out.push((if level < 40 { 1001000 } else { 1001020 }, 0));
    }
    if rng.gen_bool(0.005) {
        out.push((1088001, 0));
    }
    if rng.gen_bool(0.0002) {
        out.push((1088000, 0));
    }
    out
}

fn floor_msg(f: &Floor, act: u8, now: Instant) -> Vec<u8> {
    let mut b = vec![0u8; 174];
    b[..4].copy_from_slice(&f.id.to_le_bytes());
    b[4..8].copy_from_slice(&f.item.typ.to_le_bytes());
    b[8..10].copy_from_slice(&(f.x as u16).to_le_bytes());
    b[10..12].copy_from_slice(&(f.y as u16).to_le_bytes());
    b[14] = act;
    if act == 14 {
        b[15..19].copy_from_slice(&f.money.to_le_bytes());
    }
    let left = OWNED.saturating_sub(now.duration_since(f.born)).as_secs() as u8;
    if act == 1 && f.owner != 0 && left > 0 {
        b[20..24].copy_from_slice(&f.owner.to_le_bytes());
        b[165] = 1;
        b[170] = left;
    }
    b[166..168].copy_from_slice(&(f.from.0 as u16).to_le_bytes());
    b[168..170].copy_from_slice(&(f.from.1 as u16).to_le_bytes());
    b
}

impl World {
    pub(super) fn drop_loot(&mut self, cid: u64, i: usize) {
        let m = &self.mobs[i];
        let (map, x, y, level) = (m.map, m.x, m.y, self.d.monsters[&m.typ].level);
        let owner = self.players[&cid].h.id;
        for (k, (typ, money)) in loot(level, &mut rand::thread_rng()).into_iter().enumerate() {
            let (dx, dy) = SPOTS[k % SPOTS.len()];
            let item = Item { typ, dur: self.d.items.dur(typ), count: 1, ..Item::default() };
            self.spawn_floor(map, (x + dx, y + dy), (x, y), item, money, owner);
        }
    }

    pub(super) fn spawn_floor(&mut self, map: u32, at: (i32, i32), from: (i32, i32), item: Item, money: u32, owner: u32) {
        self.ground.next = self.ground.next.wrapping_add(1) % 1_000_000;
        let id = GROUND0 + self.ground.next;
        let f = Floor { id, item, money, map, x: at.0, y: at.1, from, owner, born: Instant::now() };
        let msg = frame(0x27b4, &floor_msg(&f, 1, f.born));
        for p in self.players.values_mut().filter(|p| p.ready && p.h.map == map && dist(p.h.x, p.h.y, at.0, at.1) <= VIEW) {
            p.seen.insert(id);
            let _ = p.tx.send(msg.clone());
        }
        self.ground.items.insert(id, f);
    }

    fn remove_floor(&mut self, id: u32) -> Option<Floor> {
        let f = self.ground.items.remove(&id)?;
        let msg = frame(0x27b4, &floor_msg(&f, 2, Instant::now()));
        for p in self.players.values_mut().filter(|p| p.h.map == f.map) {
            if p.seen.remove(&id) {
                let _ = p.tx.send(msg.clone());
            }
        }
        Some(f)
    }

    pub(super) fn pick_up(&mut self, cid: u64, body: &[u8]) {
        let id = body.get(..4).map_or(0, |b| u32::from_le_bytes(b.try_into().unwrap()));
        let (p, now) = (&self.players[&cid], Instant::now());
        let Some(f) = self.ground.items.get(&id) else { return };
        let mine = f.takeable(p.h.id, now);
        let room = f.money > 0 || !p.h.bag_full();
        if body.get(14) != Some(&3) || !p.alive || f.map != p.h.map || dist(p.h.x, p.h.y, f.x, f.y) > PICK || !mine || !room {
            return;
        }
        let f = self.remove_floor(id).unwrap();
        let h = &mut self.players.get_mut(&cid).unwrap().h;
        let hid = h.id;
        if f.money > 0 {
            h.money += f.money as u64;
            let money = h.money;
            self.send(cid, 0x27c0, &attrib(hid, &[(4, money)]));
            self.send(cid, 0x2800, &[[0; 4], f.money.to_le_bytes(), [0; 4]].concat());
            self.send(cid, 0x27b4, &floor_msg(&f, 14, now));
            return;
        }
        for (it, new) in h.add_item(f.item.typ, f.item.count, f.item.dur) {
            self.send(cid, 0x2784, &item::item_info(&it, if new { 1 } else { 3 }, 0));
        }
    }

    pub(super) fn ground_tick(&mut self, now: Instant) {
        let old: Vec<u32> = self.ground.items.values().filter(|f| now.duration_since(f.born) >= LAST).map(|f| f.id).collect();
        for id in old {
            self.remove_floor(id);
        }
    }

    pub(super) fn ground_view(&mut self, cid: u64) {
        let World { players, ground, .. } = self;
        let Some(p) = players.get_mut(&cid) else { return };
        let now = Instant::now();
        for f in ground.items.values().filter(|f| f.map == p.h.map) {
            let d = dist(p.h.x, p.h.y, f.x, f.y);
            if d <= VIEW && p.seen.insert(f.id) {
                let _ = p.tx.send(frame(0x27b4, &floor_msg(f, 1, now)));
            } else if d > VIEW && p.seen.remove(&f.id) {
                let _ = p.tx.send(frame(0x27b4, &floor_msg(f, 2, now)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn loot_rates_and_floor_packet() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(7);
        let rolls: Vec<_> = (0..20_000).map(|_| loot(10, &mut rng)).collect();
        let count = |f: &dyn Fn(&(u32, u32)) -> bool| rolls.iter().flatten().filter(|d| f(d)).count();
        let money = count(&|d| d.1 > 0);
        assert!((10_000..12_000).contains(&money), "{money}");
        assert!((60..140).contains(&count(&|d| d.0 == 1088001)));
        assert!(rolls.iter().flatten().all(|d| d.1 == 0 || (6..=13).contains(&d.1)));
        assert_eq!((count(&|d| d.0 == 1000000) > 0, count(&|d| d.0 == 1002020)), (true, 0));
        assert_eq!((money_type(9), money_type(17), money_type(150)), (1090000, 1090010, 1090020));
        let item = Item { typ: 1090010, dur: 1, count: 1, ..Item::default() };
        let now = Instant::now();
        let f = Floor { id: 7719261, item, money: 17, map: 1002, x: 537, y: 332, from: (537, 334), owner: 1282811, born: now };
        let og = "5dc97500daa1100019024c010000010000000000fb921300";
        let b = floor_msg(&f, 1, now);
        let hex: String = b[..24].iter().map(|x| format!("{x:02x}")).collect();
        assert_eq!((hex.as_str(), b.len(), &b[165..171]), (og, 174, &[1, 25, 2, 78, 1, 30][..]));
        assert_eq!((f.takeable(1282811, now), f.takeable(5, now), f.takeable(5, now + OWNED)), (true, false, true));
        assert!(Floor { owner: 0, ..f }.takeable(5, now));
        let f = Floor { id: 7719261, item: Item { typ: 1090010, dur: 1, count: 1, ..Item::default() }, money: 17, map: 1002,
            x: 537, y: 332, from: (537, 334), owner: 1282811, born: now };
        let picked = floor_msg(&f, 14, now);
        assert_eq!((picked[14], picked[15], &picked[20..24]), (14, 17, &[0u8; 4][..]));
    }
}
