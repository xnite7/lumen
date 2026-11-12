//! The release calendar: what's coming, day by day — next episodes of what
//! you watch, films on your list, things from who you follow, and the big
//! releases (switch those off up top).
use crate::api::{self, sized, Meta};
use crate::theme;
use crate::Nav;
use gpui::{div, img, prelude::*, px, Context, FontWeight, ObjectFit, ScrollHandle, SharedString, Window};
use serde_json::{json, Value};

pub struct Calendar {
    items: Option<Vec<Value>>,
    hits: bool,
    mine_only: bool,
    scroll: ScrollHandle,
}

impl Calendar {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let hits = couch::state::get().core.settings().general.calendar_hits;
        let me = Self { items: None, hits, mine_only: false, scroll: ScrollHandle::new() };
        Self::load(hits, cx);
        me
    }
    fn load(hits: bool, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let r = api::call::<Vec<Value>>("calendar_get", json!({ "hits": hits })).await.unwrap_or_default();
            let _ = this.update(cx, |c, cx| {
                c.items = Some(r);
                cx.notify();
            });
        })
        .detach();
    }
}

fn day_name(d: &str, today: &str) -> String {
    if d == today {
        return "today".into();
    }
    let tomorrow = couch::integrations::tmdb::day_str(1);
    if d == tomorrow {
        return "tomorrow".into();
    }
    for k in 2..7 {
        if d == couch::integrations::tmdb::day_str(k) {
            return weekday(d);
        }
    }
    format!("{} {}", weekday(d), &d[5..].replace('-', "/"))
}

fn weekday(d: &str) -> String {
    // Zeller-ish: days since 1970-01-01 (a Thursday)
    let p: Vec<i64> = d.split('-').filter_map(|x| x.parse().ok()).collect();
    if p.len() != 3 {
        return d.into();
    }
    let (y, m, dd) = (p[0], p[1], p[2]);
    let a = (14 - m) / 12;
    let yy = y + 4800 - a;
    let mm = m + 12 * a - 3;
    let jdn = dd + (153 * mm + 2) / 5 + 365 * yy + yy / 4 - yy / 100 + yy / 400 - 32045;
    ["monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday"][(jdn % 7) as usize].into()
}

impl Render for Calendar {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let g = theme::GUTTER;
        let vw = f32::from(window.viewport_size().width);
        let cols = (((vw - g * 2.) + 12.) / (290. + 12.)).floor().max(2.);
        let w = ((vw - g * 2.) - (cols - 1.) * 12.) / cols;
        let today = couch::integrations::tmdb::today_str();
        let mine_only = self.mine_only;
        let hits = self.hits;
        let toggle = |id: &'static str, label: &str, on: bool| {
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
        };
        let mut page = div()
            .id("calendar")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(crate::ui::smooth::wheel(&self.scroll, "calendar"))
            .bg(theme::bg())
            .pt(px(96.))
            .px(px(g))
            .flex()
            .flex_col()
            .gap(px(18.))
            .child(
                div()
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_size(px(40.)).font_weight(FontWeight::BLACK).child("calendar"))
                    .child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .child(toggle("cal-all", "Everything", !mine_only).on_click(cx.listener(|c, _, _, cx| {
                                c.mine_only = false;
                                cx.notify();
                            })))
                            .child(toggle("cal-mine", "Just mine", mine_only).on_click(cx.listener(|c, _, _, cx| {
                                c.mine_only = true;
                                cx.notify();
                            })))
                            .child(toggle("cal-hits", if hits { "big releases: on" } else { "big releases: off" }, hits).on_click(cx.listener(|c, _, _, cx| {
                                c.hits = !c.hits;
                                let on = c.hits;
                                let core = couch::state::get().core.clone();
                                core.settings.write().general.calendar_hits = on;
                                let snap = core.settings();
                                let _ = core.db.set("settings", &snap);
                                c.items = None;
                                Self::load(on, cx);
                                cx.notify();
                            }))),
                    ),
            );
        let Some(items) = &self.items else {
            return page.child(div().text_color(theme::label2()).child("loading…"));
        };
        let shown: Vec<&Value> = items.iter().filter(|i| i["date"].as_str().unwrap_or("") >= today.as_str() && (!mine_only || i["source"] != "hit")).collect();
        if shown.is_empty() {
            return page.child(div().text_color(theme::label2()).child("nothing on the calendar yet. add shows to your list or follow a studio."));
        }
        let mut days: Vec<(String, Vec<&Value>)> = vec![];
        for i in shown {
            let d = i["date"].as_str().unwrap_or("").to_string();
            match days.last_mut() {
                Some((k, l)) if *k == d => l.push(i),
                _ => days.push((d, vec![i])),
            }
        }
        for (d, list) in days {
            page = page
                .child(
                    div()
                        .flex_none()
                        .mt(px(12.))
                        .text_size(px(24.))
                        .font_weight(FontWeight::BLACK)
                        .when(d == today, |x| x.text_color(theme::lamp()))
                        .child(day_name(&d, &today)),
                )
                .child(div().flex_none().flex().flex_wrap().gap(px(12.)).children(list.into_iter().enumerate().map(|(n, i)| {
                    let art = i["still"].as_str().map(|s| s.to_string()).or(i["background"].as_str().map(|u| sized(u, "w780"))).or(i["poster"].as_str().map(|u| sized(u, "w342")));
                    let src = i["source"].as_str().unwrap_or("");
                    let (tag, tbg, tfg) = match src {
                        "yours" => ("yours", theme::neon(), theme::text()),
                        "follow" => ("following", theme::text(), gpui::rgb(0x000000)),
                        _ => ("hit", theme::lamp(), gpui::rgb(0x1c1000)),
                    };
                    let m = Meta {
                        id: i["imdb"].as_str().or(i["id"].as_str()).unwrap_or("").into(),
                        kind: i["type"].as_str().unwrap_or("movie").into(),
                        name: i["name"].as_str().unwrap_or("").into(),
                        poster: i["poster"].as_str().map(|s| s.into()),
                        background: i["background"].as_str().map(|s| s.into()),
                        ..Default::default()
                    };
                    let ep = i["episode"].as_str().filter(|e| !e.starts_with("Episode ")).map(|e| format!(" · {e}")).unwrap_or_default();
                    div()
                        .id(SharedString::from(format!("cal-{d}-{n}")))
                        .w(px(w))
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .cursor_pointer()
                        .on_click(move |_, _, cx| Nav::detail(&m, cx))
                        .child(
                            div()
                                .relative()
                                .w_full()
                                .h(px(w * 9. / 16.))
                                .rounded(px(14.))
                                .overflow_hidden()
                                .bg(theme::surface2())
                                .children(art.map(|u| img(u).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover)))
                                .child(div().absolute().left(px(8.)).top(px(8.)).px(px(9.)).py(px(3.)).rounded_full().bg(tbg).text_color(tfg).text_size(px(11.)).font_weight(FontWeight::BLACK).child(tag)),
                        )
                        .child(div().text_size(px(16.)).font_weight(FontWeight::BOLD).truncate().child(i["name"].as_str().unwrap_or("").to_string()))
                        .child(div().text_size(px(13.)).text_color(theme::label2()).truncate().child(format!("{}{}", i["label"].as_str().unwrap_or(""), ep)))
                })));
        }
        page.child(div().h(px(80.)))
    }
}
