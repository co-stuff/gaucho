/*~~~~~~~~~~~~~~~~~
 # with love from bolzov ٩(◕‿◕)۶
 # https://github.com/ibolzov
 # gaucho - conquer mobile private server
 ~~~~~~~~~~~~~~~~~*/

pub mod crypto;
pub mod proto;
mod sbox;

use self::crypto::{AccountCipher, Cfb, Handshake, SEAL_CLIENT, SEAL_SERVER};
use crate::world::Ev;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{self, UnboundedSender};
use tokio::time::{sleep, timeout};

pub type Tx = UnboundedSender<Vec<u8>>;

pub fn frame(ty: u16, body: &[u8]) -> Vec<u8> {
    let mut f = Vec::with_capacity(body.len() + 12);
    f.extend_from_slice(&((body.len() + 4) as u16).to_le_bytes());
    f.extend_from_slice(&ty.to_le_bytes());
    f.extend_from_slice(body);
    f.extend_from_slice(SEAL_SERVER);
    f
}

pub fn take_packets(buf: &mut Vec<u8>) -> Vec<(u16, Vec<u8>)> {
    let is_seal = |w: &[u8]| w == SEAL_SERVER || w == SEAL_CLIENT;
    let mut out = Vec::new();
    loop {
        if buf.len() >= 8 && is_seal(&buf[..8]) {
            buf.drain(..8);
            continue;
        }
        if buf.len() < 4 {
            break;
        }
        let len = u16::from_le_bytes([buf[0], buf[1]]) as usize;
        if len < 4 {
            let skip = buf.windows(8).skip(1).position(is_seal).map_or(1, |p| p + 1);
            buf.drain(..skip);
            continue;
        }
        if buf.len() < len {
            break;
        }
        out.push((u16::from_le_bytes([buf[2], buf[3]]), buf[4..len].to_vec()));
        buf.drain(..len);
    }
    out
}

pub async fn account(listener: TcpListener, game_host: Option<String>, tokens: Arc<AtomicU32>) {
    loop {
        let Ok((mut s, addr)) = listener.accept().await else { continue };
        let local = s.local_addr().map(|a| a.ip().to_string()).unwrap_or_default();
        let (host, tokens) = (game_host.clone().unwrap_or(local), tokens.clone());
        tokio::spawn(async move {
            sleep(Duration::from_millis(300)).await;
            let token = tokens.fetch_add(1, Ordering::Relaxed);
            let mut body = vec![0u8; 0x120];
            body[..4].copy_from_slice(&token.to_le_bytes());
            body[0x10..0x10 + host.len()].copy_from_slice(host.as_bytes());
            let mut pkt = (body.len() as u16 + 4).to_le_bytes().to_vec();
            pkt.extend_from_slice(&0x278cu16.to_le_bytes());
            pkt.extend_from_slice(&body);
            AccountCipher::new().encrypt(&mut pkt);
            if s.write_all(&pkt).await.is_err() {
                return;
            }
            println!("[ACCOUNT] {addr} -> redirect to {host} (token {token})");
            let mut sink = [0u8; 1024];
            let _ = timeout(Duration::from_secs(300), async {
                while matches!(s.read(&mut sink).await, Ok(n) if n > 0) {}
            }).await;
        });
    }
}

pub async fn game(listener: TcpListener, world: UnboundedSender<Ev>, next: Arc<AtomicU64>) {
    loop {
        let Ok((s, addr)) = listener.accept().await else { continue };
        let cid = next.fetch_add(1, Ordering::Relaxed);
        let world = world.clone();
        tokio::spawn(async move {
            if let Err(e) = session(s, addr, cid, &world).await {
                println!("[GAME {cid}] {addr} closed: {e}");
            }
            let _ = world.send(Ev::Leave { cid });
        });
    }
}

async fn session(s: TcpStream, addr: SocketAddr, cid: u64, world: &UnboundedSender<Ev>) -> std::io::Result<()> {
    s.set_nodelay(true)?;
    let (mut rd, mut wr) = s.into_split();
    let hs = Handshake::new();
    wr.write_all(&hs.server_hello()).await?;

    let mut buf = Vec::new();
    let mut chunk = vec![0u8; 8192];
    let mut key = None;
    for _ in 0..10 {
        let n = timeout(Duration::from_secs(5), rd.read(&mut chunk)).await??;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&chunk[..n]);
        while buf.len() >= 0x114 && buf[..4] == [0x14, 0x01, 0x02, 0x0a] {
            buf.drain(..0x114);
        }
        if buf.len() >= 10 {
            key = hs.client_reply(&buf[2..]).or_else(|| hs.client_reply(&buf));
            if key.is_some() {
                break;
            }
        }
    }
    let Some(key) = key else {
        println!("[GAME {cid}] {addr} handshake failed ({} B)", buf.len());
        return Ok(());
    };
    println!("[GAME {cid}] {addr} session up");

    let (mut enc, mut dec) = (Cfb::new(key.clone(), hs.iv_dec), Cfb::new(key, hs.iv_enc));
    let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let _ = world.send(Ev::Join { cid, tx, addr });
    let writer = tokio::spawn(async move {
        while let Some(mut p) = rx.recv().await {
            enc.encrypt(&mut p);
            if wr.write_all(&p).await.is_err() {
                break;
            }
        }
    });
    let mut acc = Vec::new();
    let res = loop {
        let n = match rd.read(&mut chunk).await {
            Ok(0) => break Ok(()),
            Ok(n) => n,
            Err(e) => break Err(e),
        };
        let mut d = chunk[..n].to_vec();
        dec.decrypt(&mut d);
        acc.extend_from_slice(&d);
        for (ty, body) in take_packets(&mut acc) {
            let _ = world.send(Ev::Packet { cid, ty, body });
        }
    };
    writer.abort();
    res
}

fn query<'a>(q: &'a str, k: &str) -> &'a str {
    q.split('&').find_map(|kv| kv.strip_prefix(k)?.strip_prefix('=')).unwrap_or("")
}

fn unescape(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match (b[i], s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok())) {
            (b'%', Some(v)) => {
                out.push(v);
                i += 3;
            }
            (c, _) => {
                out.push(if c == b'+' { b' ' } else { c });
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub async fn http(listener: TcpListener, world: UnboundedSender<Ev>) {
    loop {
        let Ok((mut s, addr)) = listener.accept().await else { continue };
        let world = world.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; 4096];
            let Ok(Ok(n)) = timeout(Duration::from_secs(5), s.read(&mut buf)).await else { return };
            let req = String::from_utf8_lossy(&buf[..n]).into_owned();
            let (path, q) = req.split_whitespace().nth(1).unwrap_or("").split_once('?').unwrap_or(("", ""));
            let body = match path.rsplit('/').next().unwrap_or("") {
                "login" | "register" => {
                    let (reply, rx) = tokio::sync::oneshot::channel();
                    let _ = world.send(Ev::Sdk {
                        ip: addr.ip().to_string(),
                        account: unescape(query(q, "a")),
                        pass: unescape(query(q, "p")),
                        register: path.ends_with("register"),
                        reply,
                    });
                    match rx.await {
                        Ok(Ok(l)) => format!("ok {} {}", l.account, l.token),
                        Ok(Err(e)) => format!("err {e}"),
                        Err(_) => "err Server busy".into(),
                    }
                }
                _ => return println!("[HTTP] {addr} {}", req.lines().next().unwrap_or("")),
            };
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = s.write_all((head + &body).as_bytes()).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framing_roundtrip_with_seals() {
        let mut stream = frame(0x273e, b"abc");
        stream.extend(frame(0x27dd, b""));
        stream.extend_from_slice(b"\x02\x00junk");
        stream.extend(frame(0x2799, b"lost"));
        stream.extend(frame(0x2799, b"z"));
        let mut buf = stream[..5].to_vec();
        assert!(take_packets(&mut buf).is_empty());
        buf.extend_from_slice(&stream[5..]);
        let got = take_packets(&mut buf);
        assert_eq!(got, vec![(0x273e, b"abc".to_vec()), (0x27dd, vec![]), (0x2799, b"z".to_vec())]);
    }

    #[test]
    fn query_and_unescape() {
        let q = "a=juan%40mail.com&p=5f4dcc3b&r=1";
        assert_eq!((unescape(query(q, "a")), query(q, "p"), query(q, "x")), ("juan@mail.com".into(), "5f4dcc3b", ""));
    }
}
