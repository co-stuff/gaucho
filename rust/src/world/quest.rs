/*••••••••••••••••••
 * with love from bolzov (人◕‿◕)
 * https://github.com/ibolzov
 * gaucho - conquer mobile private server
 ••••••••••••••••••*/

use super::*;
use crate::data::Quest;

pub const DONE: u32 = u32::MAX;
const READ_TIME: Duration = Duration::from_secs(2);
const CINE_TIME: Duration = Duration::from_secs(60);
const NO_CINE_RES: u32 = 1123;

fn task_status(act: u16, task: u32, status: u32) -> Vec<u8> {
    let mut b = [act.to_le_bytes(), 1u16.to_le_bytes()].concat();
    for v in [0, task, status, 0] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b
}

fn task_track(task: u32, count: u32, since: u32) -> Vec<u8> {
    let mut b = vec![0, 0, 1, 0];
    for v in [task, count, 0, 0, 0, 0, 0, since] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.extend_from_slice(&[0; 32]);
    b
}

fn act(id: u32, action: u64) -> Pb {
    Pb::new().v(1, id as u64).v(9, ms()).v(12, action)
}

pub(super) fn lua(id: u32, text: &str) -> Vec<u8> {
    Pb::new().v(1, id as u64).v(3, 0).v(9, ms()).v(12, 258).v(13, 0).v(14, 0).v(15, 0).v(19, 0).v(22, 0)
        .b(24, text.as_bytes()).out()
}

fn open_plot(name: &str, plot: u32) -> String {
    format!("Dlg_OpenPlot|{{['playerName']='{name}',['id']={plot},['PlayerName']={{'{name}'}},['view']='DlgNpcDialogue'}}")
}

fn arg_after<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let rest = &text[text.find(key)? + key.len()..];
    Some(&rest[..rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(rest.len())])
}

fn hunt_spot(d: &Data, q: &Quest, from: (u32, u32)) -> Option<(u32, u32)> {
    d.spawns.get(&q.map)?.iter()
        .filter(|s| d.monsters.get(&s.typ).is_some_and(|m| m.name.eq_ignore_ascii_case(&q.kill)))
        .map(|s| (s.x as u32 + s.cx as u32 / 2, s.y as u32 + s.cy as u32 / 2))
        .min_by_key(|&(x, y)| x.abs_diff(from.0).pow(2) + y.abs_diff(from.1).pow(2))
}

impl World {
    fn current(&self, cid: u64) -> Option<(usize, Quest)> {
        let id = self.players.get(&cid)?.h.quest;
        let i = self.d.quests.iter().position(|q| q.id == id)?;
        Some((i, self.d.quests[i].clone()))
    }

    pub(super) fn quest_login(&mut self, cid: u64) {
        match self.players[&cid].h.quest {
            DONE => {}
            0 if !self.d.quests.is_empty() => {
                self.quest_accept(cid, 0);
                self.quest_guide(cid, false);
            }
            _ => {
                let Some((i, q)) = self.current(cid) else { return };
                let count = self.players[&cid].h.tasks.get(&q.id).copied().unwrap_or(0);
                self.quest_accept(cid, i);
                self.set_count(cid, q.id, count);
                self.quest_guide(cid, false);
            }
        }
    }

    pub(super) fn quest_accept(&mut self, cid: u64, i: usize) {
        let q = self.d.quests[i].clone();
        let p = self.players.get_mut(&cid).unwrap();
        p.h.quest = q.id;
        p.h.tasks.clear();
        p.h.tasks.insert(q.id, 0);
        p.quest_timer = (q.mode == "auto").then(|| Instant::now() + READ_TIME);
        p.cine = None;
        self.send(cid, 0x2788, &task_status(1, q.id, 0));
        self.send(cid, 0x2798, &task_track(q.id, 0, unix() as u32));
    }

    fn set_count(&mut self, cid: u64, task: u32, count: u32) {
        self.players.get_mut(&cid).unwrap().h.tasks.insert(task, count);
        self.send(cid, 0x2798, &task_track(task, count, unix() as u32));
    }

    pub(super) fn quest_guide(&mut self, cid: u64, fly: bool) {
        let Some((i, q)) = self.current(cid) else { return };
        let hunting = q.count > 0 && self.players[&cid].h.tasks.get(&q.id).copied().unwrap_or(0) < q.count;
        let prev = i.checked_sub(1).and_then(|j| self.d.quests[j].path);
        let mut target = prev.filter(|p| p[1] == q.npc).map_or((q.npc, q.x as u32, q.y as u32 + 2), |p| (p[1], p[2], p[3]));
        if hunting {
            let from = prev.filter(|p| p[1] == 0).map_or((target.1, target.2), |p| (p[2], p[3]));
            if let Some((x, y)) = hunt_spot(&self.d, &q, from) {
                target = (0, x, y);
            }
        }
        let (npc, x, y) = target;
        if fly && q.map != self.players[&cid].h.map {
            self.change_map(cid, q.map, x as i32, y as i32);
        }
        let id = self.players[&cid].h.id;
        let mut b = act(id, 162).v(3, q.map as u64);
        if npc == 0 {
            b = b.v(4, 1);
        }
        self.send(cid, 0x273e, &b.v(10, npc as u64).v(13, 0).v(14, x as u64).v(15, y as u64).out());
    }

    fn quest_complete(&mut self, cid: u64) {
        let Some((i, q)) = self.current(cid) else { return };
        let id = self.players[&cid].h.id;
        println!("[QUEST] {} completed {} {}", self.players[&cid].h.name, q.id, q.name);
        self.send(cid, 0x2788, &task_status(4, q.id, 1));
        if q.exp > 0 {
            self.give_exp(cid, q.exp);
        }
        let weapon = self.classes.get(self.players[&cid].h.class).weapon;
        for &[typ, count, dur] in &q.items {
            let (typ, dur) = if (400..700).contains(&(typ / 1000)) && weapon[0] != 0 { (weapon[0], weapon[1]) } else { (typ, dur) };
            self.give_item(cid, typ, count, dur as u16);
        }
        if q.exp > 0 {
            let text = format!("STR_ID_tRewardTemplate[Currency]@@<STR_ID_tRewardTemplate[RewardExp]@@{}@@>@@", q.exp);
            self.send(cid, 0x27c3, &talk(2005, &text));
        }
        if i + 1 < self.d.quests.len() {
            self.quest_accept(cid, i + 1);
        } else {
            let p = self.players.get_mut(&cid).unwrap();
            p.h.quest = DONE;
            p.h.tasks.clear();
        }
        let (key, hero) = { let p = &self.players[&cid]; (p.key.clone(), p.h.clone()) };
        self.store.put(&key, &hero);
        self.store.flush();
        if let Some([map, x, y]) = q.fly {
            self.change_map(cid, map, x as i32, y as i32);
        }
        if let Some([map, npc, x, y]) = q.path {
            let mut b = act(id, 162).v(3, map as u64);
            if npc == 0 {
                b = b.v(4, 1);
            }
            self.send(cid, 0x273e, &b.v(10, npc as u64).v(13, 0).v(14, x as u64).v(15, y as u64).out());
        } else if self.current(cid).is_some() {
            self.quest_guide(cid, true);
        }
    }

    pub(super) fn quest_npc(&mut self, cid: u64, npc: u32, untype: u64) -> bool {
        let Some((_, q)) = self.current(cid) else { return false };
        if q.npc != npc {
            return false;
        }
        let (id, name) = { let h = &self.players[&cid].h; (h.id, h.name.clone()) };
        let count = self.players[&cid].h.tasks.get(&q.id).copied().unwrap_or(0);
        let killed = q.kill.is_empty() || count >= q.count;
        if matches!(q.mode.as_str(), "submit" | "auto") || (untype == 1 && killed) {
            self.quest_complete(cid);
        } else if q.reading {
            self.send(cid, 0x273e, &lua(id, "LuaUIMgr_PlaySound|1,sound/role_action_process.mp3,1,0"));
            let text = format!("STR_ID_tClientTask_Text[Reading][{}]@@", q.id);
            self.send(cid, 0x273e, &act(id, 164).v(3, 200).v(13, 1).v(22, 2).b(24, text.as_bytes()).out());
            self.players.get_mut(&cid).unwrap().quest_timer = Some(Instant::now() + READ_TIME);
        } else if let Some(&plot) = q.plots.first() {
            self.send(cid, 0x273e, &lua(id, &open_plot(&name, plot)));
        } else if killed && matches!(q.mode.as_str(), "main" | "kill") {
            self.quest_complete(cid);
        } else {
            return false;
        }
        true
    }

    pub(super) fn quest_lua(&mut self, cid: u64, text: &str) {
        if text.contains("ClientTaskTemplate") || text.contains("DlgOverSend") {
            println!("[QUEST] {} -> {text}", self.players[&cid].h.name);
        }
        let Some((_, q)) = self.current(cid) else { return };
        let id = self.players[&cid].h.id;
        let nums: Vec<u32> = text.split("</N>").skip(1).filter_map(|s| s.split("</").next()?.parse().ok()).collect();
        if text.contains("ClientTaskTemplate_CompleteMain") {
            if nums.first() == Some(&q.id) {
                self.quest_complete(cid);
            }
        } else if text.contains("ClientTaskTemplate_PlayPlot") {
            let (cine, next) = (nums.first().copied().unwrap_or(0), nums.get(1).copied().unwrap_or(0));
            self.players.get_mut(&cid).unwrap().cine = Some((cine as u64, next, Instant::now() + CINE_TIME));
            if self.players[&cid].res == NO_CINE_RES {
                self.quest_cine_ack(cid, cine as u64);
            } else {
                self.send(cid, 0x273e, &act(id, 456).v(5, cine as u64).out());
            }
        } else if text.contains("ClientTaskTemplate_QuestinfoFunc") {
            self.quest_guide(cid, true);
        } else if text.contains("DlgOverSend") && text.contains("['Type']=1") {
            let start = arg_after(text, "['StartId']=").and_then(|s| s.parse().ok()).unwrap_or(0);
            let unmet = !q.kill.is_empty() && self.players[&cid].h.tasks.get(&q.id).copied().unwrap_or(0) < q.count;
            if !unmet && (start / 10 == q.id || q.plots.contains(&start)) {
                self.quest_complete(cid);
            }
        }
    }

    pub(super) fn quest_cine_ack(&mut self, cid: u64, cine: u64) {
        let p = self.players.get_mut(&cid).unwrap();
        if p.cine.map(|c| c.0) != Some(cine) {
            return;
        }
        let next = p.cine.take().map_or(0, |c| c.1);
        if next != 0 {
            let (id, name) = (p.h.id, p.h.name.clone());
            self.send(cid, 0x273e, &lua(id, &open_plot(&name, next)));
        } else {
            self.quest_complete(cid);
        }
    }

    pub(super) fn quest_kill(&mut self, cid: u64, typ: u32) {
        let Some((_, q)) = self.current(cid) else { return };
        let Some(t) = self.d.monsters.get(&typ) else { return };
        if q.kill.is_empty() || t.name != q.kill.to_lowercase() {
            return;
        }
        let count = self.players[&cid].h.tasks.get(&q.id).copied().unwrap_or(0);
        if count >= q.count {
            return;
        }
        self.set_count(cid, q.id, count + 1);
        if count + 1 >= q.count && q.mode == "kill" {
            self.quest_complete(cid);
        }
    }

    pub(super) fn quest_tick(&mut self, now: Instant) {
        let stuck: Vec<(u64, u64)> = self.players.iter()
            .filter_map(|(c, p)| p.cine.filter(|x| now >= x.2).map(|x| (*c, x.0))).collect();
        for (cid, cine) in stuck {
            println!("[QUEST] cinematic {cine} not acked, moving on");
            self.quest_cine_ack(cid, cine);
        }
        let due: Vec<u64> = self.players.iter()
            .filter(|(_, p)| p.quest_timer.is_some_and(|t| now >= t))
            .map(|(c, _)| *c).collect();
        for cid in due {
            self.players.get_mut(&cid).unwrap().quest_timer = None;
            let Some((_, q)) = self.current(cid) else { continue };
            if !q.reading {
                self.quest_complete(cid);
                continue;
            }
            let (id, name) = { let h = &self.players[&cid].h; (h.id, h.name.clone()) };
            let count = self.players[&cid].h.tasks.get(&q.id).copied().unwrap_or(0);
            self.set_count(cid, q.id, count + 1);
            match q.plots.first() {
                Some(&plot) => self.send(cid, 0x273e, &lua(id, &open_plot(&name, plot))),
                None => self.quest_complete(cid),
            }
            self.send(cid, 0x273e, &act(id, 164).v(13, 2).out());
        }
    }

    pub(super) fn change_map(&mut self, cid: u64, map: u32, x: i32, y: i32) {
        let (id, old) = { let h = &self.players[&cid].h; (h.id, h.map) };
        self.forget(old, id);
        let p = self.players.get_mut(&cid).unwrap();
        p.seen.clear();
        (p.h.map, p.h.x, p.h.y) = (map, x, y);
        let (m, xx, yy) = (map as u64, x as u64, y as u64);
        self.send(cid, 0x273e, &act(id, 86).v(3, m).v(10, m).v(13, 1).v(14, xx).v(15, yy).v(17, m).out());
        if map != old {
            let flags = self.d.map_flags.get(&map).cloned().unwrap_or_else(|| "13,46".into());
            self.send(cid, 0x27e5, &Pb::new().v(1, m).v(2, m).b(3, flags.as_bytes()).out());
            self.send(cid, 0x273e, &act(id, 104).v(3, 0xffff_ffff).v(14, xx).v(15, yy).out());
            self.send(cid, 0x273e, &act(id, 157).v(3, 2).v(14, xx).v(15, yy).out());
        }
        self.refresh_view(cid);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quest_packets_match_capture() {
        assert_eq!(task_status(4, 13510, 1), [4, 0, 1, 0, 0, 0, 0, 0, 0xc6, 0x34, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0]);
        let t = task_track(13516, 0, 0x6a945d2c);
        assert_eq!((t.len(), &t[4..8], &t[32..36]), (68, &13516u32.to_le_bytes()[..], &[0x2c, 0x5d, 0x94, 0x6a][..]));
        let plot = open_plot("sixseven", 135071);
        assert_eq!(plot, "Dlg_OpenPlot|{['playerName']='sixseven',['id']=135071,['PlayerName']={'sixseven'},['view']='DlgNpcDialogue'}");
        let over = "</F>DlgOverSend</S>{['StartId']=135081,['EndId']=135082,['Type']=1}";
        assert_eq!(arg_after(over, "['StartId']="), Some("135081"));
    }

    #[test]
    fn kill_quest_guides_to_its_monsters() {
        let d = Data::load(std::path::Path::new("../data"));
        let q = d.quests.iter().find(|q| q.id == 13517).unwrap();
        assert_eq!(hunt_spot(&d, q, (532, 333)), Some((576, 365)));
    }
}
