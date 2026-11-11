//! My Lists: Plan to Watch, Watching, Completed, Favourites and your own
//! couch lists. With Simkl these are your Simkl lists; without it they come
//! from what you've added and watched in couch.
use crate::api::{self, Meta};
use crate::theme;
use crate::ui::card;
use crate::Nav;
use gpui::{div, prelude::*, px, Context, FontWeight, ScrollHandle, SharedString, Window};
use serde_json::{json, Value};

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Plan,
    Watching,
    Completed,
    Favs,
    Lists,
}

const TABS: &[(Tab, &str)] = &[(Tab::Plan, "Plan to Watch"), (Tab::Watching, "Watching"), (Tab::Completed, "Completed"), (Tab::Favs, "Favourites"), (Tab::Lists, "Lists")];

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
    scroll: ScrollHandle,
}

fn metas(v: &Value) -> Vec<Meta> {
    v.as_array().into_iter().flatten().filter_map(|m| serde_json::from_value(m.clone()).ok()).filter(|m: &Meta| !m.id.is_empty()).collect()
}

impl Library {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let me = Self { tab: Tab::Plan, kind: "all", plan: vec![], watching: vec![], completed: vec![], favs: vec![], lists: vec![], open_list: None, loaded: false, scroll: ScrollHandle::new() };
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
                    let flagged = st["flaggedWatched"].as_bool() == Some(true);
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
        .font_weight(FontWeight::BOLD)
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
        let mut page = div().id("library").size_full().overflow_y_scroll().track_scroll(&self.scroll).bg(theme::bg()).pt(px(96.)).px(px(g)).flex().flex_col().gap(px(22.));
        page = page
            .child(div().flex_none().text_size(px(40.)).font_weight(FontWeight::BLACK).child("my lists"))
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

        let list: Vec<Meta> = match self.tab {
            Tab::Plan => self.plan.clone(),
            Tab::Watching => self.watching.clone(),
            Tab::Completed => self.completed.clone(),
            Tab::Favs => self.favs.clone(),
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
                                        .border_1()
                                        .border_color(theme::black(0.6))
                                        .children(art.map(|u| gpui::img(u).size_full().object_fit(gpui::ObjectFit::Cover)))
                                })),
                            )
                            .child(div().text_size(px(16.)).font_weight(FontWeight::BOLD).child(name.clone()))
                            .child(div().text_size(px(13.)).text_color(theme::label3()).child(format!("{} titles", items.len())))
                    });
                    page = page.child(div().flex().flex_wrap().gap(px(14.)).children(tiles));
                    if self.lists.is_empty() && self.loaded {
                        page = page.child(div().text_color(theme::label2()).child("no lists yet. the + on any title makes one."));
                    }
                    return page.child(div().h(px(60.)));
                }
            },
        };
        if let (Tab::Lists, Some(i)) = (self.tab, self.open_list) {
            if let Some((_, name, _)) = self.lists.get(i) {
                page = page.child(div().flex().items_center().gap(px(12.)).child(chip("lists-back".into(), "← lists", false).on_click(cx.listener(|l, _, _, cx| {
                    l.open_list = None;
                    cx.notify();
                }))).child(div().text_size(px(22.)).font_weight(FontWeight::BLACK).child(name.clone())));
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
