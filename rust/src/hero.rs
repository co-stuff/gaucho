/*+++++++++++++++
 + https://github.com/ibolzov
 + gaucho - conquer mobile private server
 + with love from bolzov ʕ•ᴥ•ʔ
 +++++++++++++++*/

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Hero {
    pub id: u32,
    pub name: String,
    pub class: u32,
    pub level: u32,
    pub exp: u64,
    pub money: u64,
    pub emoney: u32,
    pub stats: [u16; 4],
    pub add_points: u16,
    pub hp: i32,
    pub mp: i32,
    pub look: u32,
    pub hair: u32,
    pub reborn: u8,
    pub map: u32,
    pub x: i32,
    pub y: i32,
    pub title: u32,
    pub wing: u32,
    pub titles: Vec<u32>,
    pub mount: u32,
    pub riding: bool,
    pub guild: u32,
    pub guild_rank: u32,
    pub vip: u32,
    pub pk: u32,
    pub chi: Vec<u32>,
    pub jianghu: u32,
    pub quest: u32,
    pub tasks: HashMap<u32, u32>,
    pub friends: Vec<u32>,
    pub items: Vec<Item>,
    pub skills: Vec<Skill>,
    pub ip: String,
    pub account: String,
    pub last_login: u64,
    pub download_reward: bool,
    pub mail_claimed: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Item {
    pub id: u32,
    pub typ: u32,
    pub dur: u16,
    pub count: u32,
    pub pos: u8,
    pub bound: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Skill {
    pub typ: u32,
    pub level: u16,
    pub exp: u32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct Class {
    pub name: String,
    pub create: Vec<u16>,
    pub looks: Vec<u32>,
    pub hair: u32,
    pub base: [u16; 4],
    pub growth: [u16; 4],
    pub hp_base: i32,
    pub hp_per: [i32; 4],
    pub mp_per: [i32; 4],
    pub atk_base: i32,
    pub atk_per_str: i32,
    pub def_base: i32,
    pub def_per_level: i32,
    pub kit: Vec<[u32; 3]>,
    pub weapon: [u32; 2],
}

impl Default for Class {
    fn default() -> Self {
        Class {
            name: String::new(),
            create: vec![],
            looks: vec![],
            hair: 2,
            base: [5, 2, 3, 0],
            growth: [1, 1, 1, 0],
            hp_base: 2000,
            hp_per: [3, 3, 24, 3],
            mp_per: [0, 0, 0, 5],
            atk_base: 50,
            atk_per_str: 1,
            def_base: 0,
            def_per_level: 1,
            kit: vec![],
            weapon: [0, 0],
        }
    }
}

pub const STR: usize = 0;
pub const VIT: usize = 2;
const ITEM_ID0: u32 = 10_000_000;
pub const BAG: usize = 100;
const WEAPON: u8 = 4;

impl Hero {
    pub fn new(id: u32, name: &str, class_id: u32, c: &Class, map: u32, pos: (i32, i32)) -> Hero {
        let mut h = Hero {
            id,
            name: name.to_string(),
            class: class_id,
            level: 1,
            money: 1000,
            stats: c.base,
            look: c.looks.first().copied().unwrap_or(6142007),
            hair: c.hair,
            map,
            x: pos.0,
            y: pos.1,
            ..Default::default()
        };
        h.hp = h.max_hp(c);
        h.mp = h.max_mp(c);
        for &[typ, count, dur] in &c.kit {
            h.add_item(typ, count, dur as u16);
        }
        if let [typ @ 1..=u32::MAX, dur] = c.weapon {
            let id = h.add_item(typ, 1, dur as u16)[0].0.id;
            h.equip(id, WEAPON);
        }
        h
    }

    pub fn add_item(&mut self, typ: u32, count: u32, dur: u16) -> Vec<(Item, bool)> {
        self.add(typ, count, dur, false)
    }

    pub fn add_gift(&mut self, typ: u32, count: u32, dur: u16) -> Vec<(Item, bool)> {
        self.add(typ, count, dur, true)
    }

    fn add(&mut self, typ: u32, count: u32, dur: u16, bound: bool) -> Vec<(Item, bool)> {
        if let Some(it) = self.items.iter_mut().find(|i| dur <= 1 && i.typ == typ && i.pos == 0 && i.bound == bound) {
            it.count += count;
            return vec![(it.clone(), false)];
        }
        let (n, each) = if dur <= 1 { (1, count) } else { (count, 1) };
        (0..n).map(|_| {
            let it = Item { id: self.new_item_id(), typ, dur, count: each, pos: 0, bound };
            self.items.push(it.clone());
            (it, true)
        }).collect()
    }

    fn new_item_id(&self) -> u32 {
        self.items.iter().map(|i| i.id + 1).max().unwrap_or(0).max(ITEM_ID0)
    }

    pub fn receive(&mut self, mut it: Item) -> Item {
        it.id = self.new_item_id();
        it.pos = 0;
        self.items.push(it.clone());
        it
    }

    pub fn equip(&mut self, id: u32, pos: u8) -> Option<Vec<Item>> {
        if pos == 0 || !self.items.iter().any(|i| i.id == id) {
            return None;
        }
        let mut moved = vec![];
        for it in &mut self.items {
            if it.id == id {
                it.pos = pos;
            } else if it.pos == pos {
                it.pos = 0;
                moved.push(it.clone());
            }
        }
        Some(moved)
    }

    pub fn bag_full(&self) -> bool {
        self.items.iter().filter(|i| i.pos == 0).count() >= BAG
    }

    pub fn take(&mut self, id: u32, n: u32) -> Option<Item> {
        let i = self.items.iter().position(|it| it.id == id && it.pos == 0 && it.count >= n)?;
        self.items[i].count -= n;
        let it = self.items[i].clone();
        if it.count == 0 {
            self.items.remove(i);
        }
        Some(it)
    }

    pub fn unequip(&mut self, id: u32) -> Option<Item> {
        let it = self.items.iter_mut().find(|it| it.id == id && it.pos != 0)?;
        it.pos = 0;
        Some(it.clone())
    }

    fn weigh(&self, per: &[i32; 4]) -> i32 {
        self.stats.iter().zip(per).map(|(s, k)| *s as i32 * k).sum()
    }

    pub fn max_hp(&self, c: &Class) -> i32 {
        c.hp_base + self.weigh(&c.hp_per)
    }

    pub fn max_mp(&self, c: &Class) -> i32 {
        self.weigh(&c.mp_per)
    }

    pub fn attack(&self, c: &Class) -> (i32, i32) {
        let a = c.atk_base + self.stats[STR] as i32 * c.atk_per_str;
        (a, a + self.level as i32 / 2)
    }

    pub fn defense(&self, c: &Class) -> i32 {
        c.def_base + self.level as i32 * c.def_per_level + self.stats[VIT] as i32 / 2
    }

    pub fn battle_power(&self) -> u32 {
        let gear: u32 = self.items.iter().filter(|i| i.pos != 0).map(|i| match i.typ % 10 {
            6 => 1,
            7 => 2,
            8 => 3,
            9 => 4,
            _ => 0,
        }).sum();
        self.level + self.reborn as u32 * 5 + gear
    }

    pub fn exp_to_next(&self) -> u64 {
        let l = self.level as u64;
        100 + l * l * 60
    }

    pub fn gain_exp(&mut self, amount: u64, c: &Class) -> bool {
        self.exp += amount;
        let mut up = false;
        while self.level < 140 && self.exp >= self.exp_to_next() {
            self.exp -= self.exp_to_next();
            self.level += 1;
            for i in 0..4 {
                self.stats[i] += c.growth[i];
            }
            up = true;
        }
        if up {
            self.hp = self.max_hp(c);
            self.mp = self.max_mp(c);
        }
        up
    }

    pub fn userinfo(&self, base: &[u8]) -> Vec<u8> {
        let mut b = base.get(..0xa4).map_or_else(|| vec![0; 0xa4], <[u8]>::to_vec);
        put(&mut b, 0x00, &self.id.to_le_bytes());
        put(&mut b, 0x06, &self.look.to_le_bytes());
        put(&mut b, 0x0a, &self.hair.to_le_bytes());
        put(&mut b, 0x0e, &self.money.to_le_bytes());
        put(&mut b, 0x16, &self.emoney.to_le_bytes());
        put(&mut b, 0x1a, &self.exp.to_le_bytes());
        for (i, v) in self.stats.iter().enumerate() {
            put(&mut b, 0x36 + 2 * i, &v.to_le_bytes());
        }
        put(&mut b, 0x3e, &self.add_points.to_le_bytes());
        put(&mut b, 0x44, &(self.mp.max(0) as u16).to_le_bytes());
        put(&mut b, 0x48, &self.level.to_le_bytes());
        put(&mut b, 0x4c, &self.class.to_le_bytes());
        put(&mut b, 0x59, &[self.reborn]);
        b.push(3);
        for s in [self.name.as_bytes(), b"", b"NOMATE_NAME@@"] {
            b.extend_from_slice(&(s.len() as u16).to_le_bytes());
            b.extend_from_slice(s);
        }
        b
    }

    pub fn attri_info(&self, c: &Class) -> Vec<u8> {
        let (lo, hi) = self.attack(c);
        let mut v = [0u32; 44];
        v[0] = self.id;
        v[1] = self.max_hp(c) as u32;
        v[2] = self.max_mp(c) as u32;
        v[3] = hi as u32;
        v[4] = lo as u32;
        v[5] = self.defense(c) as u32;
        v[36] = self.battle_power();
        v.iter().flat_map(|x| x.to_le_bytes()).collect()
    }
}

fn put(b: &mut [u8], at: usize, v: &[u8]) {
    b[at..at + v.len()].copy_from_slice(v);
}

pub struct Classes(pub HashMap<u32, Class>, Class);

impl Classes {
    pub fn load(path: &Path) -> Classes {
        let map = std::fs::read_to_string(path).ok()
            .and_then(|t| serde_json::from_str::<HashMap<String, Class>>(&t).ok())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(k, v)| Some((k.parse().ok()?, v)))
            .collect();
        Classes(map, Class::default())
    }

    pub fn get(&self, id: u32) -> &Class {
        self.0.get(&id).unwrap_or(&self.1)
    }

    pub fn by_index(&self, index: u16) -> Option<(u32, &Class)> {
        self.0.iter().find(|(_, c)| c.create.contains(&index)).map(|(id, c)| (*id, c))
    }
}

pub struct Store {
    path: PathBuf,
    pub heroes: HashMap<String, Hero>,
    dirty: bool,
}

impl Store {
    pub fn load(path: &Path) -> Store {
        let heroes = std::fs::read_to_string(path).ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        Store { path: path.to_path_buf(), heroes, dirty: false }
    }

    pub fn put(&mut self, key: &str, h: &Hero) {
        self.heroes.insert(key.to_string(), h.clone());
        self.dirty = true;
    }

    pub fn latest(&self, owned: impl Fn(&Hero) -> bool) -> Option<&String> {
        self.heroes.iter().filter(|(_, h)| owned(h)).max_by_key(|(_, h)| h.last_login).map(|(k, _)| k)
    }

    pub fn next_id(&self, floor: u32) -> u32 {
        self.heroes.values().map(|h| h.id + 1).max().unwrap_or(floor).max(floor)
    }

    pub fn flush(&mut self) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        let tmp = self.path.with_extension("tmp");
        if let Ok(txt) = serde_json::to_string_pretty(&self.heroes) {
            if std::fs::write(&tmp, txt).is_ok() {
                let _ = std::fs::rename(&tmp, &self.path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_match_capture_and_level_up() {
        let c = Class::default();
        let mut h = Hero::new(1, "a", 3000, &c, 1002, (430, 388));
        assert_eq!((h.max_hp(&c), h.max_mp(&c), h.attack(&c).0), (2093, 0, 55));
        assert!(h.gain_exp(h.exp_to_next(), &c));
        assert_eq!((h.level, h.max_hp(&c)), (2, 2123));
        let mut s = Store::load(Path::new("nonexistent.json"));
        s.put("k", &h);
        h.ip = "1.2.3.4".into();
        s.put("k", &h);
        assert_eq!(s.latest(|h| h.ip == "1.2.3.4").map(String::as_str), Some("k"));
        assert_eq!((s.next_id(0), s.next_id(100)), (2, 100));
        let ui = h.userinfo(&[0u8; 0xa4]);
        assert_eq!((&ui[..4], ui[0x48], &ui[0xa4..0xa8]), (&1u32.to_le_bytes()[..], 2, &[3u8, 1, 0, b'a'][..]));
    }

    #[test]
    fn items_stack_and_equip() {
        let c = Class { kit: vec![[1002020, 1, 1]], ..Class::default() };
        let mut h = Hero::new(1, "a", 5000, &c, 1002, (430, 388));
        assert_eq!(h.items.len(), 1);
        let more = h.add_item(1002020, 10, 1);
        assert_eq!((more.len(), more[0].1, h.items[0].count), (1, false, 11));
        let swords = h.add_item(608007, 2, 1698);
        assert_eq!((swords.len(), swords[0].0.id, swords[1].0.id, h.items.len()), (2, ITEM_ID0 + 1, ITEM_ID0 + 2, 3));
        assert_eq!(h.equip(ITEM_ID0 + 1, 4), Some(vec![]));
        let moved = h.equip(ITEM_ID0 + 2, 4).unwrap();
        assert_eq!((moved[0].id, moved[0].pos, h.items[2].pos), (ITEM_ID0 + 1, 0, 4));
        assert_eq!((h.equip(ITEM_ID0 + 9, 4), h.equip(ITEM_ID0 + 1, 0)), (None, None));
        let back: Hero = serde_json::from_str(&serde_json::to_string(&h).unwrap()).unwrap();
        assert_eq!(back.items, h.items);
        assert_eq!(h.take(ITEM_ID0, 12), None);
        assert_eq!(h.take(ITEM_ID0, 1).map(|i| i.count), Some(10));
        assert_eq!(h.take(ITEM_ID0 + 2, 1), None);
        assert_eq!(h.unequip(ITEM_ID0 + 2).map(|i| i.pos), Some(0));
        let armed = Hero::new(2, "b", 3000, &Class { weapon: [608007, 1698], ..c }, 1002, (430, 388));
        assert_eq!(armed.items.iter().map(|i| (i.typ, i.dur, i.pos)).collect::<Vec<_>>(), vec![(1002020, 1, 0), (608007, 1698, WEAPON)]);
        assert_eq!(h.take(ITEM_ID0 + 2, 1).map(|i| i.count), Some(0));
        assert!(h.items.iter().all(|i| i.id != ITEM_ID0 + 2) && !h.bag_full());
        let gift = h.add_gift(1002020, 2, 1);
        assert_eq!((gift[0].1, gift[0].0.bound, h.add_item(1002020, 1, 1)[0].0.count), (true, true, 11));
    }
}
