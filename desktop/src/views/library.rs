//! My Lists: Plan to Watch, Watching, Completed, Favourites and your own
//! couch lists. With Simkl these are your Simkl lists; without it they come
//! from what you've added and watched in couch.
use crate::api::{self, Meta};
use crate::theme;
use crate::ui::card;
use crate::ui::input::{Submitted, TextInput};
use crate::Nav;
use gpui::{div, prelude::*, px, Context, Entity, FontWeight, ScrollHandle, SharedString, Window};
use serde_json::{json, Value};

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Plan,
    Watching,
    Completed,
    Favs,
    Lists,
    Downloads,
}

const TABS: &[(Tab, &str)] = &[(Tab::Plan, "Plan to Watch"), (Tab::Watching, "Watching"), (Tab::Completed, "Completed"), (Tab::Favs, "Favourites"), (Tab::Lists, "Lists"), (Tab::Downloads, "Downloads")];

pub struct Library {
    tab: Tab,
    kind: &'static str,
    plan: Vec<Meta>,
    watching: Vec<Meta>,
    completed: Vec<Meta>,
    favs: Vec<Meta>,
    lists: Vec<(String, String, Vec<Meta>)>,
    open_list: Option<usize>,
    loaded: bool,
    new_name: Entity<TextInput>,
    rename: Entity<TextInput>,
    scroll: ScrollHandle,
    /// saved to this computer (and saving), refreshed while the tab is open
    downloads: Vec<Value>,
}

fn metas(v: &Value) -> Vec<Meta> {
    v.as_array().into_iter().flatten().filter_map(|m| serde_json::from_value(m.clone()).ok()).filter(|m: &Meta| !m.id.is_empty()).collect()
}

impl Library {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let new_name = cx.new(|cx| TextInput::new("name it…", cx));
        cx.subscribe(&new_name, |_, i, _: &Submitted, cx| {
            let n = i.read(cx).content.clone();
            if crate::views::addto::new_list(&n, None).is_some() {
                i.update(cx, |i, cx| i.set_text("", cx));
                Self::load(cx);
            }
        })
        .detach();
        let rename = cx.new(|cx| TextInput::new("rename…", cx));
        cx.subscribe(&rename, |l: &mut Self, i, _: &Submitted, cx| {
            let n: String = i.read(cx).content.trim().chars().take(60).collect();
            if let (Some(k), false) = (l.open_list, n.is_empty()) {
                let mut all = crate::views::addto::load();
                if let Some(x) = all.get_mut(k) {
                    x["name"] = json!(n);
                    crate::views::addto::save(&all);
                }
                i.update(cx, |i, cx| i.set_text("", cx));
                Self::load(cx);
            }
        })
        .detach();
        let me = Self { downloads: vec![], tab: Tab::Plan, kind: "all", plan: vec![], watching: vec![], completed: vec![], favs: vec![], lists: vec![], open_list: None, loaded: false, new_name, rename, scroll: ScrollHandle::new() };
        Self::load(cx);
        me
    }

    pub fn load(cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let simkl = api::call::<Value>("my_lists", json!({})).await.unwrap_or(json!({ "simkl": false }));
            let (mut plan, mut watching, mut completed) = (vec![], vec![], vec![]);
            if simkl["simkl"].as_bool() == Some(true) {
                plan = metas(&simkl["plan"]);
                watching = metas(&simkl["watching"]);
                completed = metas(&simkl["completed"]);
            } else {
                // sort the couch library into the same three lists
                let items = api::call::<Vec<Value>>("library_list", json!({})).await.unwrap_or_default();
                let mut items: Vec<Value> = items;
                items.sort_by(|a, b| b["state"]["lastWatched"].as_str().unwrap_or("").cmp(a["state"]["lastWatched"].as_str().unwrap_or("")));
                for i in items {
                    let st = &i["state"];
                    let times = st["timesWatched"].as_i64().unwrap_or(0);
                    let flagged = crate::api::flag(&st["flaggedWatched"]);
                    if i["removed"].as_bool() == Some(true) && times == 0 && !flagged {
                        continue;
                    }
                    if i["temp"].as_bool() == Some(true) && st["timeOffset"].as_f64().unwrap_or(0.) == 0. && times == 0 {
                        continue;
                    }
                    let m = Meta {
                        id: i["_id"].as_str().unwrap_or("").into(),
                        kind: i["type"].as_str().unwrap_or("").into(),
                        name: i["name"].as_str().unwrap_or("").into(),
                        poster: i["poster"].as_str().map(|s| s.into()),
                        background: i["background"].as_str().map(|s| s.into()),
                        logo: i["logo"].as_str().map(|s| s.into()),
                        ..Default::default()
                    };
                    let done = flagged || times > 0;
                    let going = !done && (st["timeOffset"].as_f64().unwrap_or(0.) > 0. || st["watchedIds"].as_array().map_or(false, |a| !a.is_empty()));
                    if done {
                        completed.push(m)
                    } else if going {
                        watching.push(m)
                    } else {
                        plan.push(m)
                    }
                }
            }
            let favs = api::call::<Value>("crew_social", json!({ "method": "GET", "path": "/v1/favs", "body": Value::Null })).await.map(|v| metas(&v["favs"])).unwrap_or_default();
            let lists = api::call::<Value>("couch_lists_get", json!({})).await.unwrap_or(json!([]));
            let lists: Vec<(String, String, Vec<Meta>)> = lists
                .as_array()
                .into_iter()
                .flatten()
                .map(|l| (l["id"].as_str().unwrap_or("").to_string(), l["name"].as_str().unwrap_or("").to_string(), metas(&l["items"])))
                .collect();
            let _ = this.update(cx, |l, cx| {
                l.plan = plan;
                l.watching = watching;
                l.completed = completed;
                l.favs = favs;
                l.lists = lists;
                l.loaded = true;
                cx.notify();
            });
        })
        .detach();
    }
}

fn chip(id: SharedString, label: &str, on: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px(px(16.))
        .h(px(38.))
        .flex()
        .items_center()
        .rounded_full()
        .text_size(px(15.))
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .when(on, |d| d.bg(theme::text()).text_color(gpui::rgb(0x000000)))
        .when(!on, |d| d.bg(theme::white(0.1)).hover(|s| s.bg(theme::white(0.18))))
        .child(label.to_string())
}

impl Render for Library {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let g = theme::GUTTER;
        let vw = f32::from(window.viewport_size().width);
        let cols = (((vw - g * 2.) + 14.) / (176. + 14.)).floor().max(2.);
        let w = ((vw - g * 2.) - (cols - 1.) * 14.) / cols;
        let mut page = div().id("library").size_full().overflow_y_scroll().track_scroll(&self.scroll)
            .child(crate::ui::smooth::wheel(&self.scroll, "library")).pt(px(96.)).px(px(g)).flex().flex_col().gap(px(22.));
        page = page
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .gap(px(18.))
                    .child(div().text_size(px(40.)).font_weight(FontWeight::BLACK).child("my lists"))
                    .child(
                        div()
                            .id("lists-following")
                            .px(px(16.))
                            .h(px(38.))
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .rounded_full()
                            .bg(theme::white(0.08))
                            .hover(|s| s.bg(theme::white(0.16)))
                            .cursor_pointer()
                            .text_size(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .on_click(|_, _, cx| crate::Nav::go(crate::Route::Following, cx))
                            .child(crate::ui::icon("heart", 17.))
                            .child("following"),
                    ),
            )
            .child(div().flex_none().flex().gap(px(8.)).flex_wrap().children(TABS.iter().map(|(t, label)| {
                let t = *t;
                chip(SharedString::from(format!("lt-{label}")), label, self.tab == t).on_click(cx.listener(move |l, _, _, cx| {
                    l.tab = t;
                    l.open_list = None;
                    cx.notify();
                }))
            })))
            .child(div().flex_none().flex().gap(px(8.)).children([("all", "Everything"), ("series", "Shows"), ("movie", "Movies")].into_iter().map(|(k, label)| {
                chip(SharedString::from(format!("lk-{k}")), label, self.kind == k).on_click(cx.listener(move |l, _, _, cx| {
                    l.kind = k;
                    cx.notify();
                }))
            })));

        if self.tab == Tab::Downloads {
            return page.child(self.downloads_view(window, cx)).child(div().h(px(60.)));
        }
        let list: Vec<Meta> = match self.tab {
            Tab::Plan => self.plan.clone(),
            Tab::Watching => self.watching.clone(),
            Tab::Completed => self.completed.clone(),
            Tab::Favs => self.favs.clone(),
            Tab::Downloads => vec![],
            Tab::Lists => match self.open_list {
                Some(i) => self.lists.get(i).map(|l| l.2.clone()).unwrap_or_default(),
                None => {
                    // the lists themselves, as stacks of posters
                    let tiles = self.lists.iter().enumerate().map(|(i, (_, name, items))| {
                        div()
                            .id(SharedString::from(format!("list-{i}")))
                            .w(px(w))
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .cursor_pointer()
                            .on_click(cx.listener(move |l, _, _, cx| {
                                l.open_list = Some(i);
                                cx.notify();
                            }))
                            .child(
                                div().relative().w_full().h(px(w * 1.5)).children(items.iter().take(3).enumerate().rev().map(|(k, m)| {
                                    let art = m.poster.clone().map(|u| crate::api::sized(&u, "w342"));
                                    div()
                                        .absolute()
                                        .top(px(k as f32 * 10.))
                                        .left(px(k as f32 * 10.))
                                        .w(px(w - 20.))
                                        .h(px(w * 1.5 - 20.))
                                        .rounded(px(10.))
                                        .overflow_hidden()
                                        .bg(theme::surface2())
                                        .children(art.map(|u| gpui::img(u).size_full().object_fit(gpui::ObjectFit::Cover)))
                                })),
                            )
                            .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).child(name.clone()))
                            .child(div().text_size(px(13.)).text_color(theme::label3()).child(format!("{} titles", items.len())))
                    });
                    let new_tile = div()
                        .w(px(w))
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(
                            div()
                                .w(px(w - 20.))
                                .h(px(w * 1.5 - 20.))
                                .rounded(px(10.))
                                .bg(theme::white(0.06))
                                .flex()
                                .flex_col()
                                .items_center()
                                .justify_center()
                                .gap(px(12.))
                                .px(px(12.))
                                .child(crate::ui::icon("plus", 30.))
                                .child(div().w_full().min_w_0().h(px(40.)).px(px(10.)).flex().items_center().overflow_hidden().rounded(px(10.)).bg(theme::white(0.08)).text_size(px(14.)).child(self.new_name.clone()))
                                .child(div().text_size(px(12.)).text_color(theme::label3()).child("enter to make it")),
                        )
                        .child(div().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).child("new list"));
                    page = page.child(div().flex().flex_wrap().gap(px(14.)).child(new_tile).children(tiles));
                    return page.child(div().h(px(60.)));
                }
            },
        };
        if let (Tab::Lists, Some(i)) = (self.tab, self.open_list) {
            if let Some((_, name, _)) = self.lists.get(i) {
                page = page.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .child(chip("lists-back".into(), "← lists", false).on_click(cx.listener(|l, _, _, cx| {
                            l.open_list = None;
                            cx.notify();
                        })))
                        .child(div().text_size(px(22.)).font_weight(FontWeight::BLACK).child(name.clone()))
                        .child(div().w(px(240.)).h(px(38.)).px(px(12.)).flex().items_center().rounded_full().bg(theme::white(0.08)).text_size(px(14.)).child(self.rename.clone()))
                        .child(chip("list-del".into(), "delete list", false).on_click(cx.listener(move |l, _, _, cx| {
                            let mut all = crate::views::addto::load();
                            if i < all.len() {
                                all.remove(i);
                                crate::views::addto::save(&all);
                            }
                            l.open_list = None;
                            Self::load(cx);
                        }))),
                );
            }
        }
        let list: Vec<Meta> = list.into_iter().filter(|m| self.kind == "all" || m.kind == self.kind).collect();
        if list.is_empty() && self.loaded {
            page = page.child(div().text_color(theme::label2()).child("nothing here yet."));
        }
        page.child(div().flex().flex_wrap().gap(px(14.)).children(list.into_iter().enumerate().map(|(i, m)| {
            let mm = m.clone();
            card::poster(SharedString::from(format!("lib-{i}")), &m, w, move |_, _, cx| Nav::detail(&mm, cx))
        })))
        .child(div().h(px(60.)))
    }
}

fn size(b: f64) -> String {
    if b >= 1e9 { format!("{:.1} GB", b / 1e9) } else { format!("{:.0} MB", b / 1e6) }
}

impl Library {
    /// Downloads: what's saved and what's saving (play, pause / carry on, delete).
    fn downloads_view(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        // fresh every second while it's on screen
        let list = couch::downloads::list(&couch::state::get().core).as_array().cloned().unwrap_or_default();
        self.downloads = list.clone();
        if list.iter().any(|d| d["done"] != true) {
            cx.spawn(async move |this, cx| {
                crate::timer::after(std::time::Duration::from_secs(1)).await;
                let _ = this.update(cx, |_, cx| cx.notify());
            })
            .detach();
        }
        let _ = window;
        if list.is_empty() {
            return div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(div().text_color(theme::label2()).child("nothing saved yet."))
                .child(div().text_size(px(13.)).text_color(theme::label3()).child("on a title's page, Download saves it (or the episode you're on) to watch without internet."))
                .into_any_element();
        }
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .max_w(px(900.))
            .children(list.into_iter().enumerate().map(|(i, d)| {
                let id = d["id"].as_str().unwrap_or("").to_string();
                let m: Meta = serde_json::from_value(d["meta"].clone()).unwrap_or_default();
                let done = d["done"] == true;
                let active = d["active"] == true;
                let (got, total) = (d["got"].as_f64().unwrap_or(0.), d["total"].as_f64().or(d["size"].as_f64()).unwrap_or(0.));
                let frac = if done { 1. } else if total > 0. { (got / total).clamp(0., 1.) } else { 0. };
                let status = if done {
                    format!("on this computer · {}", size(total.max(got)))
                } else if let Some(e) = d["error"].as_str() {
                    format!("stopped: {e}")
                } else if active {
                    let sp = d["speed"].as_f64().unwrap_or(0.);
                    let left = if sp > 0. && total > got { format!(" · {} min left", ((total - got) / sp / 60.).ceil() as i64) } else { String::new() };
                    format!("saving · {} of {} · {}/s{left}", size(got), size(total), size(sp))
                } else {
                    format!("paused · {} saved", size(got))
                };
                let art = m.background.clone().or(m.poster.clone());
                let (id1, id2, id3) = (id.clone(), id.clone(), id.clone());
                let (m1, m2) = (m.clone(), m.clone());
                let btn = |key: String, ic: &'static str| {
                    div().id(SharedString::from(key)).size(px(38.)).flex_none().rounded_full().flex().items_center().justify_center().cursor_pointer().bg(theme::white(0.08)).hover(|s| s.bg(theme::white(0.16))).child(crate::ui::icon(ic, 18.))
                };
                div()
                    .flex()
                    .items_center()
                    .gap(px(14.))
                    .p(px(10.))
                    .rounded(px(16.))
                    .bg(theme::white(0.04))
                    .child(
                        div()
                            .relative()
                            .w(px(160.))
                            .h(px(90.))
                            .flex_none()
                            .rounded(px(10.))
                            .overflow_hidden()
                            .bg(theme::surface2())
                            .children(art.map(|u| gpui::img(crate::net::fit(&api::sized(&u, "w500"), 160.)).size_full().object_fit(gpui::ObjectFit::Cover)))
                            .when(!done, |d| d.child(div().absolute().left_0().bottom_0().w_full().h(px(3.)).bg(theme::white(0.25)).child(div().h_full().w(px(160. * frac as f32)).bg(theme::neon())))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(div().truncate().text_size(px(16.)).font_weight(FontWeight::SEMIBOLD).child(d["label"].as_str().unwrap_or(&m.name).to_string()))
                            .child(div().text_size(px(13.)).text_color(theme::label3()).child(status)),
                    )
                    // play (from the file once it's all here)
                    .when(done, |d| {
                        d.child(btn(format!("dl-play-{i}"), "play").on_click(move |_, _, cx| {
                            if id1.contains(':') {
                                Nav::play_episode(&m1, id1.clone(), cx)
                            } else {
                                Nav::play(&m1, cx)
                            }
                        }))
                    })
                    // pause / carry on
                    .when(!done, |d| {
                        d.child(btn(format!("dl-pp-{i}"), if active { "pause" } else { "download" }).on_click(cx.listener(move |_, _, _, cx| {
                            if active {
                                couch::downloads::pause(&id2);
                            } else {
                                let vid = id2.contains(':').then(|| id2.clone());
                                crate::playback::download(m2.clone(), vid, cx);
                            }
                            cx.notify();
                        })))
                    })
                    .child(btn(format!("dl-del-{i}"), "trash").on_click(cx.listener(move |_, _, _, cx| {
                        couch::downloads::remove(&couch::state::get().core, &id3);
                        cx.notify();
                    })))
                    .into_any_element()
            }))
            .into_any_element()
    }
}
