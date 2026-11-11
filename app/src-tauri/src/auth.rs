//! Microsoft sign in with the device code flow, then Xbox Live, XSTS and Minecraft services.
//! The player enters the code on Microsoft's own page. No password ever reaches this program.
//! The refresh token is kept in the OS credential store only. Access tokens live in memory.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::config::{AUTH_HOSTS, MS_CLIENT_ID, SERVICE};
use crate::net::Net;

#[derive(Serialize, Clone)]
pub struct LoginPrompt {
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in_s: u64,
}

#[derive(Serialize, Clone)]
pub struct Player {
    pub name: String,
    pub head_data_url: Option<String>,
}

#[derive(Serialize)]
pub struct AuthStatus {
    pub signed_in: bool,
    pub player: Option<Player>,
    pub login: Option<LoginPrompt>,
    /// True only in test builds (cargo feature testmode). The UI shows a banner.
    pub test_mode: bool,
}

/// What the game needs. Held in memory only.
#[derive(Clone)]
pub struct Session {
    pub name: String,
    pub uuid: String,
    pub access_token: String,
}

#[derive(Default)]
pub struct Inner {
    pub player: Option<Player>,
    pub session: Option<Session>,
    pub login: Option<(LoginPrompt, Instant)>,
    /// Bumped on every login start and logout, so an old background poll knows it was cancelled.
    pub generation: u64,
    /// Shown once by the next auth_status (expired code, declined, no game on the account).
    pub last_error: Option<String>,
}

pub type Shared = Arc<Mutex<Inner>>;

/// Where the refresh token lives. A trait so tests can use memory, release builds use the OS store.
pub trait TokenStore: Send + Sync {
    fn get(&self) -> Option<String>;
    fn set(&self, v: &str) -> Result<(), String>;
    fn clear(&self);
}

pub struct KeychainStore;

impl KeychainStore {
    fn entry() -> Result<keyring::Entry, String> {
        keyring::Entry::new(SERVICE, "microsoft-refresh-token").map_err(|_| "Kunde inte nå kontolagret i systemet.".to_string())
    }
}

impl TokenStore for KeychainStore {
    fn get(&self) -> Option<String> {
        Self::entry().ok()?.get_password().ok()
    }
    fn set(&self, v: &str) -> Result<(), String> {
        Self::entry()?.set_password(v).map_err(|_| "Kunde inte spara inloggningen i systemets kontolager.".to_string())
    }
    fn clear(&self) {
        if let Ok(e) = Self::entry() {
            let _ = e.delete_credential();
        }
    }
}

const SCOPE: &str = "XboxLive.signin offline_access";
const TENANT: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0";

pub fn client_id() -> Result<&'static str, String> {
    MS_CLIENT_ID.filter(|s| !s.is_empty()).ok_or_else(|| "Inloggningen är inte inställd ännu. Be någon i teamet om hjälp.".to_string())
}

#[derive(Deserialize)]
struct DeviceCode {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    #[serde(default = "five")]
    interval: u64,
}
fn five() -> u64 {
    5
}

#[derive(Deserialize)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
}

fn net_err<T>(_: T) -> String {
    "Nätverksfel vid inloggning.".to_string()
}

async fn post_form(net: &Net, url: &str, form: &[(&str, &str)]) -> Result<(u16, Value), String> {
    crate::security::check_host(url, AUTH_HOSTS)?;
    let r = net.client().post(url).form(form).send().await.map_err(net_err)?;
    let code = r.status().as_u16();
    Ok((code, r.json::<Value>().await.unwrap_or(Value::Null)))
}

async fn post_json(net: &Net, url: &str, body: Value) -> Result<(u16, Value), String> {
    crate::security::check_host(url, AUTH_HOSTS)?;
    let r = net.client().post(url).header("Accept", "application/json").json(&body).send().await.map_err(net_err)?;
    let code = r.status().as_u16();
    Ok((code, r.json::<Value>().await.unwrap_or(Value::Null)))
}

/// Step 1 of the flow. The caller shows `LoginPrompt` and then calls `poll_device_code`.
pub async fn start_device_code(net: &Net) -> Result<(LoginPrompt, String, u64), String> {
    let cid = client_id()?;
    let (code, v) = post_form(net, &format!("{TENANT}/devicecode"), &[("client_id", cid), ("scope", SCOPE)]).await?;
    if code != 200 {
        return Err("Microsoft avvisade inloggningen.".into());
    }
    let d: DeviceCode = serde_json::from_value(v).map_err(net_err)?;
    // The page address comes from Microsoft. open_url still checks it before it is ever opened.
    Ok((LoginPrompt { user_code: d.user_code, verification_uri: d.verification_uri, expires_in_s: d.expires_in }, d.device_code, d.interval))
}

/// Waits until the player has entered the code. `alive` returns false when the login was cancelled.
pub async fn poll_device_code(net: &Net, device_code: &str, interval: u64, expires_in: u64, alive: impl Fn() -> bool) -> Result<Tokens, String> {
    let cid = client_id()?;
    let end = Instant::now() + Duration::from_secs(expires_in);
    let mut wait = interval.max(3);
    while Instant::now() < end {
        tokio::time::sleep(Duration::from_secs(wait)).await;
        if !alive() {
            return Err("Inloggningen avbröts.".into());
        }
        let (code, v) = post_form(net, &format!("{TENANT}/token"), &[
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"), ("client_id", cid), ("device_code", device_code),
        ]).await?;
        if code == 200 {
            return serde_json::from_value(v).map_err(net_err);
        }
        match v.get("error").and_then(|e| e.as_str()) {
            Some("authorization_pending") => {}
            Some("slow_down") => wait += 5,
            Some("authorization_declined") => return Err("Inloggningen nekades.".into()),
            Some("expired_token") | Some("bad_verification_code") => return Err("Koden gick ut. Försök igen.".into()),
            _ => return Err("Inloggningen misslyckades.".into()),
        }
    }
    Err("Koden gick ut. Försök igen.".into())
}

pub async fn refresh(net: &Net, refresh_token: &str) -> Result<Tokens, String> {
    let cid = client_id()?;
    let (code, v) = post_form(net, &format!("{TENANT}/token"), &[
        ("grant_type", "refresh_token"), ("client_id", cid), ("refresh_token", refresh_token), ("scope", SCOPE),
    ]).await?;
    if code != 200 {
        return Err("Du behöver logga in igen.".into());
    }
    serde_json::from_value(v).map_err(net_err)
}

/// Microsoft access token to a Minecraft session (name, uuid, game token). 404 on the profile means no game.
pub async fn minecraft_session(net: &Net, ms_access: &str) -> Result<(Session, Option<String>), String> {
    let (c, xbl) = post_json(net, "https://user.auth.xboxlive.com/user/authenticate", json!({
        "Properties": {"AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={ms_access}")},
        "RelyingParty": "http://auth.xboxlive.com", "TokenType": "JWT"
    })).await?;
    if c != 200 { return Err("Xbox Live svarade med fel.".into()); }
    let xbl_token = xbl["Token"].as_str().ok_or_else(|| net_err(()))?.to_string();
    let uhs = xbl["DisplayClaims"]["xui"][0]["uhs"].as_str().ok_or_else(|| net_err(()))?.to_string();

    let (c, xsts) = post_json(net, "https://xsts.auth.xboxlive.com/xsts/authorize", json!({
        "Properties": {"SandboxId": "RETAIL", "UserTokens": [xbl_token]},
        "RelyingParty": "rp://api.minecraftservices.com/", "TokenType": "JWT"
    })).await?;
    if c == 401 {
        return Err(match xsts["XErr"].as_u64() {
            Some(2148916233) => "Kontot saknar Xbox profil. Skapa en på xbox.com.".to_string(),
            Some(2148916235) => "Xbox Live finns inte i ditt land.".to_string(),
            Some(2148916236) | Some(2148916237) => "Kontot behöver verifieras av en vuxen.".to_string(),
            Some(2148916238) => "Kontot är ett barnkonto och måste läggas till i en familj.".to_string(),
            _ => "Xbox Live nekade inloggningen.".to_string(),
        });
    }
    if c != 200 { return Err("Xbox Live svarade med fel.".into()); }
    let xsts_token = xsts["Token"].as_str().ok_or_else(|| net_err(()))?;

    let (c, mc) = post_json(net, "https://api.minecraftservices.com/authentication/login_with_xbox", json!({
        "identityToken": format!("XBL3.0 x={uhs};{xsts_token}")
    })).await?;
    if c != 200 { return Err("Minecraft nekade inloggningen.".into()); }
    let mc_token = mc["access_token"].as_str().ok_or_else(|| net_err(()))?.to_string();

    crate::security::check_host("https://api.minecraftservices.com/minecraft/profile", AUTH_HOSTS)?;
    let r = net.client().get("https://api.minecraftservices.com/minecraft/profile").bearer_auth(&mc_token).send().await.map_err(net_err)?;
    if r.status().as_u16() == 404 { return Err("Det här kontot äger inte Minecraft Java.".into()); }
    if !r.status().is_success() { return Err("Kunde inte hämta spelarprofilen.".into()); }
    let p: Value = r.json().await.map_err(net_err)?;
    let name = p["name"].as_str().ok_or_else(|| net_err(()))?.to_string();
    let uuid = p["id"].as_str().ok_or_else(|| net_err(()))?.to_string();
    let skin = p["skins"].as_array().and_then(|a| a.iter().find(|s| s["state"] == "ACTIVE")).and_then(|s| s["url"].as_str()).map(|s| s.to_string());
    Ok((Session { name, uuid, access_token: mc_token }, skin))
}

/// Runs the whole chain from a Microsoft token pair and stores the new refresh token.
pub async fn finish(net: &Net, store: &dyn TokenStore, shared: &Shared, gen: u64, t: Tokens) -> Result<(), String> {
    let (session, skin) = minecraft_session(net, &t.access_token).await?;
    let head = match skin { Some(u) => crate::head::fetch_head(net, &u).await.ok(), None => None };
    if let Some(rt) = &t.refresh_token { store.set(rt)?; }
    let mut g = shared.lock().map_err(|_| "Internt fel.".to_string())?;
    if g.generation != gen { return Err("Inloggningen avbröts.".into()); }
    g.player = Some(Player { name: session.name.clone(), head_data_url: head });
    g.session = Some(session);
    g.login = None;
    Ok(())
}

pub fn status(shared: &Shared) -> Result<AuthStatus, String> {
    let mut g = shared.lock().unwrap();
    if let Some(e) = g.last_error.take() { return Err(e); }
    let login = g.login.as_ref().map(|(p, started)| {
        let left = p.expires_in_s.saturating_sub(started.elapsed().as_secs());
        LoginPrompt { user_code: p.user_code.clone(), verification_uri: p.verification_uri.clone(), expires_in_s: left }
    });
    if login.as_ref().map(|l| l.expires_in_s == 0).unwrap_or(false) { g.login = None; }
    Ok(AuthStatus { signed_in: g.player.is_some(), player: g.player.clone(), login, test_mode: cfg!(feature = "testmode") })
}

pub fn logout(shared: &Shared, store: &dyn TokenStore) {
    let mut g = shared.lock().unwrap();
    g.generation += 1;
    g.player = None;
    g.session = None;
    g.login = None;
    g.last_error = None;
    drop(g);
    store.clear();
}

#[cfg(test)]
pub struct MemStore(pub Mutex<Option<String>>);
#[cfg(test)]
impl TokenStore for MemStore {
    fn get(&self) -> Option<String> { self.0.lock().unwrap().clone() }
    fn set(&self, v: &str) -> Result<(), String> { *self.0.lock().unwrap() = Some(v.into()); Ok(()) }
    fn clear(&self) { *self.0.lock().unwrap() = None; }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn logout_clears_everything_and_cancels() {
        let shared: Shared = Arc::new(Mutex::new(Inner::default()));
        let store = MemStore(Mutex::new(Some("rt".into())));
        {
            let mut g = shared.lock().unwrap();
            g.player = Some(Player { name: "A".into(), head_data_url: None });
            g.login = Some((LoginPrompt { user_code: "X".into(), verification_uri: "https://microsoft.com/link".into(), expires_in_s: 900 }, Instant::now()));
        }
        let gen = shared.lock().unwrap().generation;
        logout(&shared, &store);
        let s = status(&shared).unwrap();
        assert!(!s.signed_in && s.login.is_none());
        assert_ne!(shared.lock().unwrap().generation, gen);
        assert!(store.get().is_none());
    }
    #[test]
    fn no_client_id_gives_clear_error() {
        if MS_CLIENT_ID.is_none() { assert!(client_id().is_err()); }
    }
}

/// Test builds only. An offline player: name from the OS user, token "0". Online mode servers reject it,
/// so it cannot join the public server. Nothing is stored in the keychain.
#[cfg(feature = "testmode")]
pub fn offline_session() -> Session {
    use sha1::{Digest, Sha1};
    let raw = std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_default();
    let mut name: String = raw.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '_').take(12).collect();
    if name.len() < 3 { name = "Tester".into(); }
    let name = format!("T_{name}");
    let h = Sha1::digest(format!("OfflinePlayer:{name}").as_bytes());
    let mut b = [0u8; 16];
    b.copy_from_slice(&h[..16]);
    b[6] = (b[6] & 0x0f) | 0x30;
    b[8] = (b[8] & 0x3f) | 0x80;
    Session { name, uuid: hex::encode(b), access_token: "0".into() }
}

#[cfg(test)]
mod testmode_tests {
    #[cfg(feature = "testmode")]
    #[test]
    fn offline_session_is_offline_only() {
        let s = super::offline_session();
        assert_eq!(s.access_token, "0");
        assert!(s.name.starts_with("T_") && s.name.len() >= 5 && s.name.len() <= 16);
        assert_eq!(s.uuid.len(), 32);
    }
}
