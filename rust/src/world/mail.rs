/*··················
 # https://github.com/ibolzov
 # gaucho - conquer mobile private server
 # with love from bolzov (◕‿◕)
 ··················*/

use super::*;

const MAIL_ID: u64 = 1_188_654;
const MAIL_GIFT: u64 = 574_625;
const MAIL_ITEM: u32 = 3_323_512;
const MAIL_TTL: u64 = 2_592_000;
const MAIL_KEY: &str = "STR_ID_tValentineCoat";

fn mail_list(action: u64, claimed: bool) -> Vec<u8> {
    let entry = Pb::new().v(1, MAIL_ID).v(2, 0).v(3, 0).v(4, MAIL_TTL).v(6, claimed as u64)
        .v(7, if claimed { 0 } else { MAIL_GIFT })
        .b(8, format!("{MAIL_KEY}[Sender]@@").as_bytes()).b(9, format!("{MAIL_KEY}[Title]@@").as_bytes())
        .v(10, 0).v(11, 0).out();
    Pb::new().v(1, action).v(2, 1).b(3, &entry).out()
}

impl World {
    pub(super) fn mail_list_req(&mut self, cid: u64) {
        let claimed = self.players[&cid].h.mail_claimed;
        self.send(cid, 0x32ca, &mail_list(0, claimed));
    }

    pub(super) fn mail_action(&mut self, cid: u64, body: &[u8]) {
        let m = Msg::parse_lossy(body);
        match m.get(1) {
            1 if m.get(2) == MAIL_ID => {
                let text = format!("{MAIL_KEY}[Content]@@");
                self.send(cid, 0x32c9, &Pb::new().v(1, MAIL_ID).b(2, text.as_bytes()).out());
            }
            7 if m.get(3) == MAIL_ID => {
                let h = &mut self.players.get_mut(&cid).unwrap().h;
                if !std::mem::replace(&mut h.mail_claimed, true) {
                    self.give_item(cid, MAIL_ITEM, 1, 1);
                }
                self.send(cid, 0x32ca, &mail_list(1, true));
            }
            a => println!("[MAIL] action {a} {:?}", (m.get(2), m.get(3))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mail_list_matches_capture() {
        let hex = |s: &str| (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect::<Vec<u8>>();
        let mut want = hex("080110011a5608aec6481000180020809a9e0130013800421f5354525f49445f7456616c656e74696e65436f61745b53656e6465725d40404a1e5354525f49445f7456616c656e74696e65436f61745b5469746c655d404050005800");
        assert_eq!(mail_list(1, true), want);
        want = hex("080010011a5808aec6481000180020809a9e01300038a18923421f5354525f49445f7456616c656e74696e65436f61745b53656e6465725d40404a1e5354525f49445f7456616c656e74696e65436f61745b5469746c655d404050005800");
        assert_eq!(mail_list(0, false), want);
    }
}
