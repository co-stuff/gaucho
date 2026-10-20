/********************
 * https://github.com/ibolzov
 * gaucho - conquer mobile private server
 * with love from bolzov (●'◡'●)
 ********************/

use super::sbox::*;
use md5::{Digest, Md5};
use num_bigint::BigUint;
use rand::RngCore;

pub const HANDSHAKE_KEY: &[u8] = b"C238xs65pjy7HU9Q";
pub const SEAL_SERVER: &[u8] = b"TQServer";
pub const SEAL_CLIENT: &[u8] = b"TQClient";

const DH_PRIME_HEX: &str = "FFFFFFFFFFFFFFFFC90FDAA22168C234C4C6628B80DC1CD1\
29024E088A67CC74020BBEA63B139B22514A08798E3404DD\
EF9519B3CD3A431B302B0A6DF25F14374FE1356D6D51C245\
E485B576625E7EC6F44C42E9A637ED6B0BFF5CB6F406B7ED\
EE386BFB5A899FA5AE9F24117C4B1FE649286651ECE65381\
FFFFFFFFFFFFFFFF";

#[derive(Clone)]
pub struct Cast5 {
    km: [u32; 16],
    kr: [u32; 16],
}

#[inline]
fn split(i: u32) -> (usize, usize, usize, usize) {
    ((i >> 24) as usize, (i >> 16 & 0xff) as usize, (i >> 8 & 0xff) as usize, (i & 0xff) as usize)
}

fn f1(d: u32, km: u32, kr: u32) -> u32 {
    let (a, b, c, e) = split(d.wrapping_add(km).rotate_right(kr.wrapping_neg() & 31));
    (S2[e] ^ S1[c]).wrapping_sub(S3[a]).wrapping_add(S4[b])
}
fn f2(d: u32, km: u32, kr: u32) -> u32 {
    let (a, b, c, e) = split((d ^ km).rotate_right(kr.wrapping_neg() & 31));
    S1[c].wrapping_sub(S2[e]).wrapping_add(S3[a]) ^ S4[b]
}
fn f3(d: u32, km: u32, kr: u32) -> u32 {
    let (a, b, c, e) = split(km.wrapping_sub(d).rotate_right(kr.wrapping_neg() & 31));
    (S2[e].wrapping_add(S1[c]) ^ S3[a]).wrapping_sub(S4[b])
}

impl Cast5 {
    pub fn new(raw: &[u8]) -> Self {
        let mut x = [0u8; 16];
        let n = raw.len().min(16);
        x[..n].copy_from_slice(&raw[..n]);
        let mut z = [0u8; 16];
        let w = |a: &[u8; 16], i: usize| u32::from_be_bytes([a[i], a[i + 1], a[i + 2], a[i + 3]]);
        let set = |a: &mut [u8; 16], i: usize, v: u32| a[i..i + 4].copy_from_slice(&v.to_be_bytes());
        let s = |t: &[u32; 256], b: u8| t[b as usize];

        let round_a = |x: &[u8; 16], z: &mut [u8; 16]| {
            set(z, 0, w(x, 0) ^ s(&S5, x[13]) ^ s(&S6, x[15]) ^ s(&S7, x[12]) ^ s(&S8, x[14]) ^ s(&S7, x[8]));
            set(z, 4, w(x, 8) ^ s(&S5, z[0]) ^ s(&S6, z[2]) ^ s(&S7, z[1]) ^ s(&S8, z[3]) ^ s(&S8, x[10]));
            set(z, 8, w(x, 12) ^ s(&S5, z[7]) ^ s(&S6, z[6]) ^ s(&S7, z[5]) ^ s(&S8, z[4]) ^ s(&S5, x[9]));
            set(z, 12, w(x, 4) ^ s(&S5, z[10]) ^ s(&S6, z[9]) ^ s(&S7, z[11]) ^ s(&S8, z[8]) ^ s(&S6, x[11]));
        };
        let round_b = |x: &mut [u8; 16], z: &[u8; 16]| {
            set(x, 0, w(z, 8) ^ s(&S5, z[5]) ^ s(&S6, z[7]) ^ s(&S7, z[4]) ^ s(&S8, z[6]) ^ s(&S7, z[0]));
            set(x, 4, w(z, 0) ^ s(&S5, x[0]) ^ s(&S6, x[2]) ^ s(&S7, x[1]) ^ s(&S8, x[3]) ^ s(&S8, z[2]));
            set(x, 8, w(z, 4) ^ s(&S5, x[7]) ^ s(&S6, x[6]) ^ s(&S7, x[5]) ^ s(&S8, x[4]) ^ s(&S5, z[1]));
            set(x, 12, w(z, 12) ^ s(&S5, x[10]) ^ s(&S6, x[9]) ^ s(&S7, x[11]) ^ s(&S8, x[8]) ^ s(&S6, z[3]));
        };
        const PA: [[usize; 5]; 4] = [[8, 9, 7, 6, 2], [10, 11, 5, 4, 6], [12, 13, 3, 2, 9], [14, 15, 1, 0, 12]];
        const PB: [[usize; 5]; 4] = [[3, 2, 12, 13, 8], [1, 0, 14, 15, 13], [7, 6, 8, 9, 3], [5, 4, 10, 11, 7]];
        const PC: [[usize; 5]; 4] = [[3, 2, 12, 13, 9], [1, 0, 14, 15, 12], [7, 6, 8, 9, 2], [5, 4, 10, 11, 6]];
        const PD: [[usize; 5]; 4] = [[8, 9, 7, 6, 3], [10, 11, 5, 4, 7], [12, 13, 3, 2, 8], [14, 15, 1, 0, 13]];
        let last = [&S5, &S6, &S7, &S8];
        let extract = |a: &[u8; 16], p: &[[usize; 5]; 4]| -> [u32; 4] {
            let mut o = [0u32; 4];
            for (k, r) in p.iter().enumerate() {
                o[k] = S5[a[r[0]] as usize] ^ S6[a[r[1]] as usize] ^ S7[a[r[2]] as usize]
                    ^ S8[a[r[3]] as usize] ^ last[k][a[r[4]] as usize];
            }
            o
        };

        let mut k = [0u32; 32];
        for half in 0..2 {
            let base = half * 16;
            round_a(&x, &mut z);
            k[base..base + 4].copy_from_slice(&extract(&z, &PA));
            round_b(&mut x, &z);
            k[base + 4..base + 8].copy_from_slice(&extract(&x, &PB));
            round_a(&x, &mut z);
            k[base + 8..base + 12].copy_from_slice(&extract(&z, &PC));
            round_b(&mut x, &z);
            k[base + 12..base + 16].copy_from_slice(&extract(&x, &PD));
        }
        let mut km = [0u32; 16];
        let mut kr = [0u32; 16];
        km.copy_from_slice(&k[..16]);
        for i in 0..16 {
            kr[i] = ((k[16 + i] & 0x1f) + 0x10) & 0x1f;
        }
        Cast5 { km, kr }
    }

    pub fn encrypt_block(&self, b: [u8; 8]) -> [u8; 8] {
        let mut l = u32::from_be_bytes([b[0], b[1], b[2], b[3]]);
        let mut r = u32::from_be_bytes([b[4], b[5], b[6], b[7]]);
        let (km, kr) = (&self.km, &self.kr);
        for i in (0..16).step_by(2) {
            let fa = [f1, f2, f3][i % 3];
            l ^= fa(r, km[i], kr[i]);
            let fb = [f1, f2, f3][(i + 1) % 3];
            r ^= fb(l, km[i + 1], kr[i + 1]);
        }
        let mut o = [0u8; 8];
        o[..4].copy_from_slice(&r.to_be_bytes());
        o[4..].copy_from_slice(&l.to_be_bytes());
        o
    }
}

pub struct Cfb {
    c: Cast5,
    iv: [u8; 8],
    num: usize,
}

impl Cfb {
    pub fn new(c: Cast5, iv: [u8; 8]) -> Self {
        Cfb { c, iv, num: 0 }
    }
    pub fn encrypt(&mut self, data: &mut [u8]) {
        for b in data {
            if self.num == 0 {
                self.iv = self.c.encrypt_block(self.iv);
            }
            *b ^= self.iv[self.num];
            self.iv[self.num] = *b;
            self.num = (self.num + 1) & 7;
        }
    }
    pub fn decrypt(&mut self, data: &mut [u8]) {
        for b in data {
            if self.num == 0 {
                self.iv = self.c.encrypt_block(self.iv);
            }
            let ct = *b;
            *b ^= self.iv[self.num];
            self.iv[self.num] = ct;
            self.num = (self.num + 1) & 7;
        }
    }
}

pub fn handshake_cfb() -> Cfb {
    Cfb::new(Cast5::new(HANDSHAKE_KEY), [0; 8])
}

pub struct AccountCipher {
    t1: [u8; 256],
    t2: [u8; 256],
    i1: usize,
    i2: usize,
}

impl AccountCipher {
    pub fn new() -> Self {
        fn table(seed: i32, a: i32, b: i32, c: i32) -> [u8; 256] {
            let mut t = [0u8; 256];
            let mut v = seed & 0xff;
            for e in t.iter_mut() {
                *e = v as u8;
                let sv = v as u8 as i8 as i32;
                v = ((sv * a + b) * sv + c) & 0xff;
            }
            t
        }
        AccountCipher { t1: table(0x9d, -6, 15, 19), t2: table(0x62, -92, 121, 109), i1: 0, i2: 0 }
    }
    pub fn encrypt(&mut self, data: &mut [u8]) {
        for b in data {
            *b = (*b ^ 0xab).rotate_left(4) ^ self.t1[self.i1] ^ self.t2[self.i2];
            self.i1 += 1;
            if self.i1 > 255 {
                self.i1 = 0;
                if self.i2 < 255 {
                    self.i2 += 1;
                }
            }
        }
    }
}

pub struct Handshake {
    p: BigUint,
    private: BigUint,
    pub_hex: String,
    pub iv_dec: [u8; 8],
    pub iv_enc: [u8; 8],
}

impl Handshake {
    pub fn new() -> Self {
        let p = BigUint::parse_bytes(DH_PRIME_HEX.as_bytes(), 16).unwrap();
        let mut rng = rand::thread_rng();
        let mut buf = [0u8; 128];
        rng.fill_bytes(&mut buf);
        let two = BigUint::from(2u32);
        let private = BigUint::from_bytes_be(&buf) % (&p - &two) + &two;
        let pub_hex = two.modpow(&private, &p).to_str_radix(16).to_uppercase();
        let (mut iv_dec, mut iv_enc) = ([0u8; 8], [0u8; 8]);
        rng.fill_bytes(&mut iv_dec);
        rng.fill_bytes(&mut iv_enc);
        Handshake { p, private, pub_hex, iv_dec, iv_enc }
    }

    pub fn server_hello(&self) -> Vec<u8> {
        let mut rng = rand::thread_rng();
        let mut nonce = [0u8; 8];
        rng.fill_bytes(&mut nonce);
        let fields: [&[u8]; 6] = [&nonce, &self.iv_dec, &self.iv_enc, DH_PRIME_HEX.as_bytes(), b"2", self.pub_hex.as_bytes()];
        let mut tlv = Vec::new();
        for f in fields {
            tlv.extend_from_slice(&(f.len() as u32).to_le_bytes());
            tlv.extend_from_slice(f);
        }
        let mut out = vec![0u8; 11];
        rng.fill_bytes(&mut out);
        out.extend_from_slice(&((tlv.len() + SEAL_SERVER.len() + 4) as u32).to_le_bytes());
        out.extend_from_slice(&tlv);
        out.extend_from_slice(SEAL_SERVER);
        handshake_cfb().encrypt(&mut out);
        out
    }

    pub fn client_reply(&self, wire: &[u8]) -> Option<Cast5> {
        if wire.len() < 8 {
            return None;
        }
        let mut d = wire.to_vec();
        handshake_cfb().decrypt(&mut d);
        let seal = d.windows(8).rposition(|w| w == SEAL_CLIENT)?;
        let pre = &d[..seal];
        let start = pre.iter().rposition(|b| !b.is_ascii_hexdigit()).map_or(0, |i| i + 1);
        if start >= pre.len() {
            return None;
        }
        let client = BigUint::parse_bytes(&pre[start..], 16)?;
        let shared = client.modpow(&self.private, &self.p).to_bytes_be();
        Some(Cast5::new(&md5_key(&shared)[..16]))
    }
}

pub fn md5_key(secret: &[u8]) -> Vec<u8> {
    let s = &secret[..secret.iter().position(|&b| b == 0).unwrap_or(secret.len())];
    let hex = |b: &[u8]| Md5::digest(b).iter().map(|x| format!("{x:02x}")).collect::<String>();
    let m1 = hex(s);
    let m2 = hex(format!("{m1}{m1}").as_bytes());
    format!("{m1}{m2}").into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    #[test]
    fn matches_python() {
        let c = Cast5::new(HANDSHAKE_KEY);
        assert_eq!((c.km[0], c.kr[0], c.km[1], c.kr[1]), (0x7012dcbf, 7, 0x94ff927d, 0x12));
        assert_eq!(hex(&c.encrypt_block([0, 1, 2, 3, 4, 5, 6, 7])), "16d940b7748180a3");
        let mut m = b"hello world, TQServer!".to_vec();
        handshake_cfb().encrypt(&mut m);
        assert_eq!(hex(&m), "a476b069c942c5e3331715ee88c8642296e88ebead0f");
        handshake_cfb().decrypt(&mut m);
        assert_eq!(&m, b"hello world, TQServer!");
        assert_eq!(
            String::from_utf8(md5_key(&[0x0a, 0x0b, 0x00, 0xff])).unwrap(),
            "5b1ad04637eedf255ed4f452cd26b3ed9bf1331b8f65ab18b5f58f13ccaf53f7"
        );
        let mut a: Vec<u8> = (0..20).collect();
        AccountCipher::new().encrypt(&mut a);
        assert_eq!(hex(&a), "45587b6249045f5e7d6093ea816cb706f4892a13");
    }

    #[test]
    fn dh_roundtrip() {
        let hs = Handshake::new();
        let mut hello = hs.server_hello();
        handshake_cfb().decrypt(&mut hello);
        assert!(hello.ends_with(SEAL_SERVER));
        let p = BigUint::parse_bytes(DH_PRIME_HEX.as_bytes(), 16).unwrap();
        let cpriv = BigUint::from(123456789u64);
        let cpub = BigUint::from(2u32).modpow(&cpriv, &p).to_str_radix(16);
        let mut reply = [b"\x10\x00\x00\x00xyz".as_slice(), cpub.as_bytes(), SEAL_CLIENT].concat();
        handshake_cfb().encrypt(&mut reply);
        let server_key = hs.client_reply(&reply).expect("reply parsed");
        let spub = BigUint::parse_bytes(hs.pub_hex.as_bytes(), 16).unwrap();
        let client_key = Cast5::new(&md5_key(&spub.modpow(&cpriv, &p).to_bytes_be())[..16]);
        assert_eq!(server_key.encrypt_block([9; 8]), client_key.encrypt_block([9; 8]));
    }
}
