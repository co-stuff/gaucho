/*·············
 # with love from bolzov (◡‿◡✿)
 # https://github.com/ibolzov
 # gaucho - conquer mobile private server
 ·············*/

mod data;
mod hero;
mod net;
mod world;

use net::proto;
use std::path::Path;
use std::sync::atomic::{AtomicU32, AtomicU64};
use std::sync::Arc;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let env = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
    let binds = env("CQ2_BIND_HOST", "127.0.0.1");
    let game_host = std::env::var("CQ2_GAME_HOST").ok();
    let (acc_port, game_port) = (env("CQ2_ACCOUNT_PORT", "8020"), env("CQ2_GAME_PORT", "5816"));
    let http_port = env("CQ2_HTTP_PORT", "9528");

    let dir = env("CQ2_DATA", "../data");
    let data = data::Data::load(Path::new(&dir));
    let classes = hero::Classes::load(&Path::new(&dir).join("classes.json"));
    let store = hero::Store::load(&Path::new(&dir).join("heroes.json"));
    let social = world::Social::load(Path::new(&dir));
    let skills = world::Skills::load(Path::new(&dir));
    let accounts = world::Accounts::load(&Path::new(&dir).join("accounts.json"));
    let tokens = Arc::new(AtomicU32::new(world::HERO_ID0));
    let cids = Arc::new(AtomicU64::new(1));
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(world::World::new(data, classes, store, social, skills, accounts).run(rx));

    let mut tasks = Vec::new();
    for host in binds.split(',').map(str::trim).filter(|h| !h.is_empty()) {
        let bind = |port: &str| {
            let addr = format!("{host}:{port}");
            async move { TcpListener::bind(&addr).await.unwrap_or_else(|e| panic!("bind {addr}: {e}")) }
        };
        let (acc, game, http) = (bind(&acc_port).await, bind(&game_port).await, bind(&http_port).await);
        println!("[OK] {host}: account {acc_port}, game {game_port}, login {http_port}");
        tasks.push(tokio::spawn(net::account(acc, game_host.clone(), tokens.clone())));
        tasks.push(tokio::spawn(net::game(game, tx.clone(), cids.clone())));
        tasks.push(tokio::spawn(net::http(http, tx.clone())));
    }
    for t in tasks {
        let _ = t.await;
    }
}
