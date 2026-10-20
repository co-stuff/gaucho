/*=====================
 * with love from bolzov (づ◕‿◕)づ
 * https://github.com/ibolzov
 * gaucho - conquer mobile private server
 =====================*/

use super::*;

const HELP: &str = "/level N | /tp map x y | /quest id | /title id | /ride | /guild create|join|leave N | /money N | /item type [n] [dur] | /heal | /new [ip] | /who";

impl World {
    pub(super) fn command(&mut self, cid: u64, text: &str) -> bool {
        let Some(cmd) = text.strip_prefix('/') else { return false };
        if self.account_command(cid, cmd) {
            return true;
        }
        let name = self.players[&cid].h.name.clone();
        let allowed = std::env::var("CQ2_GM").map_or(true, |v| v.split(',').any(|n| n.trim().eq_ignore_ascii_case(&name)));
        if !allowed {
            return false;
        }
        let mut words = cmd.split_whitespace();
        let verb = words.next().unwrap_or("").to_lowercase();
        let args: Vec<i64> = words.filter_map(|a| a.parse().ok()).collect();
        let arg = |i: usize| args.get(i).copied().unwrap_or(0).max(0);
        println!("[GM] {name}: /{cmd}");
        let reply = match verb.as_str() {
            "level" if arg(0) > 0 => self.gm_level(cid, arg(0).min(140) as u32),
            "tp" if args.len() == 3 => {
                self.change_map(cid, arg(0) as u32, arg(1) as i32, arg(2) as i32);
                format!("tp {} {} {}", arg(0), arg(1), arg(2))
            }
            "quest" => match self.d.quests.iter().position(|q| q.id == arg(0) as u32) {
                Some(i) => {
                    self.quest_accept(cid, i);
                    self.quest_guide(cid, true);
                    format!("quest {} {}", self.d.quests[i].id, self.d.quests[i].name)
                }
                None => format!("quest {} does not exist; the first one is {}", arg(0), self.d.quests.first().map_or(0, |q| q.id)),
            },
            "title" => self.gm_title(cid, arg(0) as u32),
            "ride" => {
                let on = !self.players[&cid].h.riding;
                self.ride(cid, on);
                format!("ride {on}")
            }
            "guild" => self.gm_guild(cid, &cmd.split_whitespace().skip(1).collect::<Vec<_>>()),
            "money" => {
                let p = self.players.get_mut(&cid).unwrap();
                p.h.money = arg(0) as u64;
                let id = p.h.id;
                self.send(cid, 0x27c0, &attrib(id, &[(4, arg(0) as u64)]));
                format!("money {}", arg(0))
            }
            "item" if arg(0) > 0 => {
                let (count, dur) = (arg(1).clamp(1, 999) as u32, arg(2).min(65535) as u16);
                self.give_item(cid, arg(0) as u32, count, dur);
                format!("item {} x{count} dur {dur}", arg(0))
            }
            "heal" => {
                self.revive(cid);
                "heal".into()
            }
            "new" => {
                let ip = cmd.split_whitespace().nth(1).filter(|a| a.parse::<std::net::IpAddr>().is_ok())
                    .map_or_else(|| self.players[&cid].addr.ip().to_string(), str::to_string);
                self.fresh.insert(ip.clone());
                format!("new: the next login from {ip} creates a character")
            }
            "who" => self.players.values().filter(|p| p.h.id != 0)
                .map(|p| format!("{} lv{} {}({},{})", p.h.name, p.h.level, p.h.map, p.h.x, p.h.y))
                .collect::<Vec<_>>().join(" | "),
            _ => HELP.into(),
        };
        self.send(cid, 0x27c3, &talk(2005, &reply));
        true
    }

    fn gm_level(&mut self, cid: u64, level: u32) -> String {
        let p = self.players.get_mut(&cid).unwrap();
        let c = self.classes.get(p.h.class);
        p.h.level = level;
        p.h.exp = 0;
        for i in 0..4 {
            p.h.stats[i] = c.base[i] + c.growth[i] * (level as u16 - 1);
        }
        p.h.hp = p.h.max_hp(c);
        p.h.mp = p.h.max_mp(c);
        let (id, map) = (p.h.id, p.h.map);
        self.sync_attribs(cid);
        self.learn_skills(cid);
        self.send_view(map, id, 0x27c0, &attrib(id, &[(12, level as u64)]));
        self.send_view(map, id, 0x273e, &Pb::new().v(1, id as u64).v(9, ms()).v(12, 92).v(14, level as u64).out());
        format!("level {level}")
    }
}
