//! Sign in with Discord (OAuth2 + PKCE, public client: no secret ships in
//! the app). Scopes `identify` (your id, name and avatar: who you are on the
//! couch) and `guilds` (which servers you're in, so couch can check you're in
//! the midnite server). Nothing else.
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
/// The Discord server couch is for (repo secret DISCORD_GUILD_ID). Unset = open to anyone.
pub const GUILD_ID: Option<&str> = option_env!("COUCH_GUILD_ID");
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
        "https://discord.com/oauth2/authorize?response_type=code&client_id={CLIENT_ID}&scope=identify%20guilds&redirect_uri={}&state={state}&code_challenge={challenge}&code_challenge_method=S256&prompt=consent",
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
    save_tokens(core, &tok)?;
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

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn save_tokens(core: &Core, tok: &Value) -> AppResult<()> {
    if let Some(a) = tok.get("access_token").and_then(|v| v.as_str()) {
        core.db.set_secret("discord_access", Some(a))?;
    }
    if let Some(r) = tok.get("refresh_token").and_then(|v| v.as_str()) {
        core.db.set_secret("discord_refresh", Some(r))?;
    }
    let ttl = tok.get("expires_in").and_then(|e| e.as_u64()).unwrap_or(604_800);
    core.db.set("discord_expires", &json!(now() + ttl))?;
    core.db.set("discord_scopes", &json!(tok.get("scope").and_then(|s| s.as_str()).unwrap_or("")))?;
    Ok(())
}

/// A working access token (refreshed when it's close to expiring).
pub async fn access_token(core: &Core) -> Option<String> {
    let exp = core.db.get::<Value>("discord_expires").and_then(|v| v.as_u64()).unwrap_or(0);
    if exp > now() + 3600 {
        return core.db.secret("discord_access");
    }
    let refresh = core.db.secret("discord_refresh")?;
    let mut form = vec![("client_id", CLIENT_ID.to_string()), ("grant_type", "refresh_token".to_string()), ("refresh_token", refresh)];
    if let Some(secret) = option_env!("DISCORD_CLIENT_SECRET").filter(|s| !s.is_empty()) {
        form.push(("client_secret", secret.to_string()));
    }
    let tok: Value = core.http.client().post("https://discord.com/api/oauth2/token").form(&form).timeout(Duration::from_secs(15)).send().await.ok()?.json().await.ok()?;
    tok.get("access_token")?;
    save_tokens(core, &tok).ok()?;
    core.db.secret("discord_access")
}

/// Are you in the midnite server? "member" / "outsider" / "signin" (needs
/// a fresh sign-in, e.g. from before the server check) / "unknown" (offline).
pub async fn membership(core: &Core) -> &'static str {
    let Some(guild) = GUILD_ID.filter(|g| !g.is_empty()) else { return "member" };
    if user(core).is_none() {
        return "signin";
    }
    let scopes = core.db.get::<Value>("discord_scopes").and_then(|v| v.as_str().map(|s| s.to_string())).unwrap_or_default();
    if !scopes.split_whitespace().any(|s| s == "guilds") {
        return "signin";
    }
    let Some(tok) = access_token(core).await else { return "signin" };
    let res = core.http.client().get("https://discord.com/api/users/@me/guilds").bearer_auth(&tok).timeout(Duration::from_secs(12)).send().await;
    let Ok(res) = res else { return cached(core) };
    if res.status().as_u16() == 401 {
        return "signin";
    }
    let Ok(list) = res.json::<Value>().await else { return cached(core) };
    let inside = list.as_array().map_or(false, |a| a.iter().any(|g| g.get("id").and_then(|i| i.as_str()) == Some(guild)));
    let _ = core.db.set("discord_member", &json!(inside));
    if inside { "member" } else { "outsider" }
}

/// Offline: trust the last answer.
fn cached(core: &Core) -> &'static str {
    match core.db.get::<Value>("discord_member").and_then(|v| v.as_bool()) {
        Some(true) => "member",
        Some(false) => "outsider",
        None => "unknown",
    }
}

pub fn user(core: &Core) -> Option<Value> {
    core.db.get::<Value>("discord_user")
}

pub fn logout(core: &Core) -> AppResult<()> {
    let _ = core.db.set_secret("discord_access", None);
    let _ = core.db.set_secret("discord_refresh", None);
    let _ = core.db.set_secret("crew_session", None);
    let _ = core.db.del("discord_member");
    core.db.del("discord_user")
}
