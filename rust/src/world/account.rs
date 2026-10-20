/*=====================
 > https://github.com/ibolzov
 > gaucho - conquer mobile private server
 > with love from bolzov (◡‿◡✿)
 =====================*/

use super::*;
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub(super) const SDK_RES: u32 = 1123;

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Account {
    pass: String,
}

pub struct Accounts {
    path: PathBuf,
    list: HashMap<String, Account>,
}

impl Accounts {
    pub fn load(path: &Path) -> Accounts {
        let list = std::fs::read_to_string(path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        Accounts { path: path.to_path_buf(), list }
    }

    fn save(&self) {
        if let Ok(t) = serde_json::to_string_pretty(&self.list) {
            let _ = std::fs::write(&self.path, t);
        }
    }
}

#[derive(Debug)]
pub struct SdkLogin {
    pub account: String,
    pub token: String,
}

fn digest(pass: &str) -> String {
    Md5::digest(pass.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

fn valid_account(a: &str) -> bool {
    (3..=24).contains(&a.chars().count()) && a.chars().all(|c| c.is_ascii_alphanumeric() || "_.@-".contains(c))
}

impl World {
    pub(super) fn sdk(&mut self, ip: String, account: &str, pass: &str, register: bool) -> Result<SdkLogin, &'static str> {
        let key = account.trim().to_lowercase();
        if !valid_account(&key) || pass.is_empty() {
            return Err("Account must be 3 to 24 letters, digits or _.@-");
        }
        match self.accounts.list.get(&key) {
            Some(a) if a.pass != digest(pass) => {
                println!("[ACCOUNT] {key} from {ip}: wrong password");
                return Err(if register { "That account already exists" } else { "Wrong password" });
            }
            Some(_) => {}
            None => {
                self.accounts.list.insert(key.clone(), Account { pass: digest(pass) });
                self.accounts.save();
                println!("[ACCOUNT] created {key}");
            }
        }
        println!("[ACCOUNT] {key} signed in from {ip}");
        self.bound.insert(ip, key.clone());
        Ok(SdkLogin { account: key, token: format!("{:016x}", rand::random::<u64>()) })
    }

    pub(super) fn account_of(&self, cid: u64) -> Option<String> {
        let p = &self.players[&cid];
        (p.res == SDK_RES).then(|| self.bound.get(&p.addr.ip().to_string()).cloned()).flatten()
    }
}
