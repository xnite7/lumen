//! Calling the engine from the UI: the same commands the old webview used,
//! by name, run on couch's own runtime.
use serde::de::DeserializeOwned;
use serde_json::Value;

/// `api::call::<Vec<Meta>>("hero_items", json!({})).await`
pub async fn call<T: DeserializeOwned + Send + 'static>(cmd: &'static str, args: Value) -> Result<T, String> {
    let h = couch::rt::spawn(async move {
        let mut v = couch::dispatch::invoke(cmd, args).await?;
        if cmd.starts_with("crew") {
            unmangle(&mut v);
        }
        serde_json::from_value::<T>(v).map_err(|e| format!("{cmd}: {e}"))
    });
    h.await.map_err(|e| e.to_string())?
}

/// The crew server used to read names as Latin-1, so "✿ mari ✿" came back
/// as "â\u{9c}¿ mari â\u{9c}¿": put such strings back together.
fn unmangle(v: &mut Value) {
    match v {
        Value::String(s) if s.chars().any(|c| c as u32 >= 0x80) && s.chars().all(|c| (c as u32) < 0x100) => {
            if let Ok(fixed) = String::from_utf8(s.chars().map(|c| c as u8).collect()) {
                *s = fixed;
            }
        }
        Value::Array(a) => a.iter_mut().for_each(unmangle),
        Value::Object(o) => o.values_mut().for_each(unmangle),
        _ => {}
    }
}

/// A title in a row or the billboard (the fields the UI uses).
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Meta {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    pub poster: Option<String>,
    pub background: Option<String>,
    pub logo: Option<String>,
    pub description: Option<String>,
    pub release_info: Option<Value>,
    pub imdb_rating: Option<Value>,
    pub genres: Option<Vec<String>>,
    pub runtime: Option<Value>,
}

impl Meta {
    pub fn year(&self) -> Option<String> {
        match &self.release_info {
            Some(Value::String(s)) => Some(s.chars().take(4).collect()),
            Some(Value::Number(n)) => Some(n.to_string()),
            _ => None,
        }
    }
}

/// Same sizing rules as the webview's img(): TMDB and metahub sizes.
pub fn sized(url: &str, size: &str) -> String {
    if url.contains("image.tmdb.org/t/p/") && size.starts_with('w') {
        if let Some(i) = url.find("/t/p/") {
            let rest = &url[i + 5..];
            if let Some(j) = rest.find('/') {
                return format!("{}/t/p/{}{}", &url[..i], size, &rest[j..]);
            }
        }
    }
    if url.contains("images.metahub.space") {
        let m = match size {
            "small" | "w342" => "small",
            "medium" | "w500" | "w780" => "medium",
            _ => "large",
        };
        for s in ["/small/", "/medium/", "/large/"] {
            if url.contains(s) {
                return url.replacen(s, &format!("/{m}/"), 1);
            }
        }
    }
    url.to_string()
}

pub fn json(v: Value) -> Value {
    v
}

/// IMDb's still for one episode ("tt123:1:4"), via metahub — used when a
/// title's own episode picture is missing or won't load.
pub fn imdb_episode(video_id: &str) -> Option<String> {
    let p: Vec<&str> = video_id.split(':').collect();
    let tt = p.first().filter(|t| t.starts_with("tt"))?;
    let (s, e) = (p.get(1)?.parse::<i64>().ok()?, p.get(2)?.parse::<i64>().ok()?);
    (s >= 0 && e > 0).then(|| format!("https://episodes.metahub.space/{tt}/{s}/{e}/w780.jpg"))
}
