//! Have you watched a title, is it in one of your lists: asked by every card
//! on every frame, so the answers are kept until the library changes.
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static C: RefCell<(u64, std::time::Instant, HashMap<String, (bool, bool)>)> = RefCell::new((u64::MAX, std::time::Instant::now(), HashMap::new()));
}

fn base(id: &str) -> &str {
    id.split(':').next().unwrap_or(id)
}

/// (watched, in My List or one of your lists)
pub fn status(id: &str) -> (bool, bool) {
    let id = base(id);
    if id.is_empty() || !id.starts_with("tt") {
        return (false, false);
    }
    let g0 = crate::views::detail::lib_gen();
    C.with(|c| {
        let mut c = c.borrow_mut();
        // the player moves things too: look again every half minute
        if c.0 != g0 || c.1.elapsed().as_secs() > 30 {
            *c = (g0, std::time::Instant::now(), HashMap::new());
        }
        if let Some(v) = c.2.get(id) {
            return *v;
        }
        let item = couch::commands::library_item(couch::state::get(), id.to_string());
        let watched = item.as_ref().map_or(false, |i| i["state"]["timesWatched"].as_i64().unwrap_or(0) > 0 || crate::api::flag(&i["state"]["flaggedWatched"]));
        let listed = item.as_ref().map_or(false, |i| i["removed"].as_bool() != Some(true) && i["temp"].as_bool() != Some(true))
            || crate::views::addto::load().iter().any(|l| l["items"].as_array().map_or(false, |a| a.iter().any(|x| x["id"].as_str().map_or(false, |x| base(x) == id))));
        c.2.insert(id.to_string(), (watched, listed));
        (watched, listed)
    })
}

thread_local! {
    /// titles' own languages, learned as their pages load
    static LANG: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}

pub fn set_lang(id: &str, lang: &str) {
    LANG.with(|l| {
        l.borrow_mut().insert(base(id).to_string(), lang.to_string());
    });
}

/// a small flag for a title that isn't in English (from its language)
pub fn flag_for(id: &str, lang: Option<&str>) -> Option<String> {
    let l = lang.map(String::from).or_else(|| LANG.with(|m| m.borrow().get(base(id)).cloned()))?;
    let cc = match l.as_str() {
        "en" | "" => return None,
        "ja" => "jp",
        "ko" => "kr",
        "zh" | "cn" => "cn",
        "hi" | "ta" | "te" | "ml" | "kn" | "bn" | "mr" | "pa" | "gu" => "in",
        "es" => "es",
        "fr" => "fr",
        "de" => "de",
        "it" => "it",
        "pt" => "br",
        "ru" => "ru",
        "tr" => "tr",
        "ar" => "ae",
        "sv" => "se",
        "da" => "dk",
        "no" | "nb" | "nn" => "no",
        "fi" => "fi",
        "pl" => "pl",
        "nl" => "nl",
        "th" => "th",
        "id" => "id",
        "ms" => "my",
        "tl" => "ph",
        "vi" => "vn",
        "fa" => "ir",
        "he" => "il",
        "el" => "gr",
        "cs" => "cz",
        "hu" => "hu",
        "ro" => "ro",
        "uk" => "ua",
        "is" => "is",
        "et" => "ee",
        "sr" => "rs",
        "hr" => "hr",
        "ur" => "pk",
        "cy" => "gb",
        _ => return None,
    };
    Some(format!("https://flagcdn.com/w40/{cc}.png"))
}
