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
