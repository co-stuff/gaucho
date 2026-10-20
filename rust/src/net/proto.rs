/*============
 # with love from bolzov (●'◡'●)
 # https://github.com/ibolzov
 # gaucho - conquer mobile private server
 ============*/

pub fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    while v > 0x7f {
        out.push((v as u8 & 0x7f) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

pub fn varint_bytes(v: u64) -> Vec<u8> {
    let mut o = Vec::with_capacity(10);
    put_varint(&mut o, v);
    o
}

fn read_varint(b: &[u8], p: &mut usize) -> Option<u64> {
    let mut v = 0u64;
    for shift in (0..64).step_by(7) {
        let x = *b.get(*p)?;
        *p += 1;
        v |= ((x & 0x7f) as u64) << shift;
        if x < 0x80 {
            return Some(v);
        }
    }
    None
}

#[derive(Default)]
pub struct Pb(pub Vec<u8>);

impl Pb {
    pub fn new() -> Self {
        Pb(Vec::with_capacity(64))
    }
    pub fn v(mut self, field: u32, val: u64) -> Self {
        put_varint(&mut self.0, (field as u64) << 3);
        put_varint(&mut self.0, val);
        self
    }
    pub fn b(mut self, field: u32, data: &[u8]) -> Self {
        put_varint(&mut self.0, (field as u64) << 3 | 2);
        put_varint(&mut self.0, data.len() as u64);
        self.0.extend_from_slice(data);
        self
    }
    pub fn out(self) -> Vec<u8> {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    V(u64),
    B(Vec<u8>),
    F64([u8; 8]),
    F32([u8; 4]),
}

#[derive(Clone, Debug, Default)]
pub struct Msg(pub Vec<(u32, Val)>);

impl Msg {
    pub fn parse(b: &[u8]) -> Option<Msg> {
        let mut p = 0;
        let mut f = Vec::new();
        while p < b.len() {
            let tag = read_varint(b, &mut p)?;
            let field = (tag >> 3) as u32;
            let val = match tag & 7 {
                0 => Val::V(read_varint(b, &mut p)?),
                2 => {
                    let n = read_varint(b, &mut p)? as usize;
                    let v = b.get(p..p + n)?.to_vec();
                    p += n;
                    Val::B(v)
                }
                1 => {
                    let v = b.get(p..p + 8)?.try_into().ok()?;
                    p += 8;
                    Val::F64(v)
                }
                5 => {
                    let v = b.get(p..p + 4)?.try_into().ok()?;
                    p += 4;
                    Val::F32(v)
                }
                _ => return None,
            };
            f.push((field, val));
        }
        Some(Msg(f))
    }

    pub fn parse_lossy(b: &[u8]) -> Msg {
        Msg::parse(b).unwrap_or_default()
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut o = Vec::new();
        for (field, v) in &self.0 {
            let f = (*field as u64) << 3;
            match v {
                Val::V(x) => {
                    put_varint(&mut o, f);
                    put_varint(&mut o, *x);
                }
                Val::B(x) => {
                    put_varint(&mut o, f | 2);
                    put_varint(&mut o, x.len() as u64);
                    o.extend_from_slice(x);
                }
                Val::F64(x) => {
                    put_varint(&mut o, f | 1);
                    o.extend_from_slice(x);
                }
                Val::F32(x) => {
                    put_varint(&mut o, f | 5);
                    o.extend_from_slice(x);
                }
            }
        }
        o
    }

    pub fn get(&self, field: u32) -> u64 {
        self.0.iter().find_map(|(f, v)| match v {
            Val::V(x) if *f == field => Some(*x),
            _ => None,
        }).unwrap_or(0)
    }

    pub fn set(&mut self, field: u32, val: u64) -> &mut Self {
        match self.0.iter_mut().find(|(f, v)| *f == field && matches!(v, Val::V(_))) {
            Some(e) => e.1 = Val::V(val),
            None => self.0.push((field, Val::V(val))),
        }
        self
    }

    pub fn set_nth(&mut self, field: u32, n: usize, val: u64) -> &mut Self {
        if let Some(e) = self.0.iter_mut().filter(|(f, v)| *f == field && matches!(v, Val::V(_))).nth(n) {
            e.1 = Val::V(val);
        }
        self
    }

    pub fn set_nth_bytes(&mut self, field: u32, n: usize, data: &[u8]) -> &mut Self {
        if let Some(e) = self.0.iter_mut().filter(|(f, v)| *f == field && matches!(v, Val::B(_))).nth(n) {
            e.1 = Val::B(data.to_vec());
        }
        self
    }

    pub fn bytes(&self, field: u32, n: usize) -> Option<&[u8]> {
        self.0.iter().filter_map(|(f, v)| match v {
            Val::B(x) if *f == field => Some(x.as_slice()),
            _ => None,
        }).nth(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_edit() {
        let raw = Pb::new().v(1, 1282811).b(107, b"sixseven").v(2, 0).b(107, b"").out();
        let mut m = Msg::parse(&raw).unwrap();
        assert_eq!(m.encode(), raw);
        assert_eq!(m.get(1), 1282811);
        m.set(1, 7).set(30, 3).set_nth_bytes(107, 0, b"Hero0001");
        let m2 = Msg::parse(&m.encode()).unwrap();
        assert_eq!((m2.get(1), m2.get(30), m2.bytes(107, 0)), (7, 3, Some(&b"Hero0001"[..])));
        assert_eq!(m2.bytes(107, 1), Some(&b""[..]));
    }
}
