//! Sign in with Discord (OAuth2 + PKCE, public client: no secret ships in
//! the app). Only the `identify` scope: couch learns your Discord id, name
//! and avatar, which become your name on the couch. Nothing else.
//!
//! Computer: the browser comes back to http://127.0.0.1:53682/discord.
//! iPhone: it comes back through couch://auth/discord.
//! Both must be listed as redirects on the Discord application.

use crate::core::Core;
use crate::error::{AppError, AppResult};
use base64::Engine;
use serde_json::{json, Value};
use sha2::Digest;
use std::sync::Mutex;
use std::time::Duration;

pub const CLIENT_ID: &str = "1568062298721886290";
pub const DESKTOP_PORT: u16 = 53682;
pub const DESKTOP_REDIRECT: &str = "http://127.0.0.1:53682/discord";
#[allow(dead_code)]
pub const MOBILE_REDIRECT: &str = "couch://auth/discord";

struct Pending {
    verifier: String,
    state: String,
    redirect: String,
}
static PENDING: Mutex<Option<Pending>> = Mutex::new(None);

fn token(len: usize) -> String {
    use rand::Rng;
    const C: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
    let mut r = rand::rng();
    (0..len).map(|_| C[r.random_range(0..C.len())] as char).collect()
}

/// The URL to open in the browser.
pub fn start(redirect: &str) -> String {
    let verifier = token(64);
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(verifier.as_bytes()));
    let state = token(24);
    let url = format!(
        "https://discord.com/oauth2/authorize?response_type=code&client_id={CLIENT_ID}&scope=identify&redirect_uri={}&state={state}&code_challenge={challenge}&code_challenge_method=S256&prompt=consent",
        urlencoding::encode(redirect)
    );
    *PENDING.lock().unwrap() = Some(Pending { verifier, state, redirect: redirect.to_string() });
    url
}

/// `url` is whatever came back (full URL or just the path + query).
pub async fn finish(core: &Core, url: &str) -> AppResult<Value> {
    let q = url.split_once('?').map(|(_, q)| q).unwrap_or("");
    let get = |k: &str| {
        q.split('&').find_map(|kv| {
            let (a, b) = kv.split_once('=')?;
            (a == k).then(|| urlencoding::decode(b).map(|s| s.into_owned()).unwrap_or_default())
        })
    };
    if let Some(e) = get("error") {
        return Err(AppError::msg(format!("Discord said no ({e})")));
    }
    let code = get("code").ok_or_else(|| AppError::msg("Discord didn't send a sign-in code"))?;
    let p = PENDING.lock().unwrap().take().ok_or_else(|| AppError::msg("sign-in expired, try again"))?;
    if get("state").as_deref() != Some(p.state.as_str()) {
        return Err(AppError::msg("sign-in didn't match, try again"));
    }
    let client = core.http.client();
    let mut form = vec![
        ("client_id", CLIENT_ID.to_string()),
        ("grant_type", "authorization_code".to_string()),
        ("code", code.clone()),
        ("redirect_uri", p.redirect.clone()),
        ("code_verifier", p.verifier.clone()),
    ];
    // With "Public Client" on, PKCE alone is enough. Otherwise the secret
    // comes from the build (repo secret DISCORD_CLIENT_SECRET), never the source.
    if let Some(secret) = option_env!("DISCORD_CLIENT_SECRET").filter(|s| !s.is_empty()) {
        form.push(("client_secret", secret.to_string()));
    }
    let tok: Value = client
        .post("https://discord.com/api/oauth2/token")
        .form(&form)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| AppError::msg(e.to_string()))?
        .json()
        .await
        .map_err(|e| AppError::msg(e.to_string()))?;
    let access = tok
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::msg(format!("Discord sign-in failed: {}", tok.get("error_description").or(tok.get("error")).and_then(|v| v.as_str()).unwrap_or("no token"))))?;
    let me: Value = client
        .get("https://discord.com/api/users/@me")
        .bearer_auth(access)
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| AppError::msg(e.to_string()))?
        .json()
        .await
        .map_err(|e| AppError::msg(e.to_string()))?;
    let id = me.get("id").and_then(|v| v.as_str()).ok_or_else(|| AppError::msg("Discord didn't say who you are"))?;
    let name = me.get("global_name").and_then(|v| v.as_str()).filter(|s| !s.is_empty()).or(me.get("username").and_then(|v| v.as_str())).unwrap_or("you");
    let avatar = me.get("avatar").and_then(|v| v.as_str()).map(|a| format!("https://cdn.discordapp.com/avatars/{id}/{a}.png?size=128"));
    let user = json!({ "id": id, "name": name, "username": me.get("username"), "avatar": avatar });
    core.db.set("discord_user", &user)?;
    // your Discord name is your name on the couch, unless you set one
    {
        let mut st = core.settings.write();
        if st.integrations.crew_name.trim().is_empty() {
            st.integrations.crew_name = name.to_string();
        }
        core.db.set("settings", &*st)?;
    }
    Ok(user)
}

pub fn user(core: &Core) -> Option<Value> {
    core.db.get::<Value>("discord_user")
}

pub fn logout(core: &Core) -> AppResult<()> {
    core.db.del("discord_user")
}
