//! The end of a film (or the last episode out): the player fades into a
//! full-screen "watch next". The pick's backdrop fills the screen, its logo
//! sits on the left and its trailer fades in through the backdrop. Play it,
//! put it on your list, look closer, or leave. Other picks run along the bottom.
use crate::api::{self, sized, Meta};
use crate::theme;
use crate::Nav;
use gpui::{div, img, linear_color_stop, linear_gradient, prelude::*, px, Context, FontWeight, ObjectFit, SharedString, Window};
use serde_json::{json, Value};
use std::time::Instant;

pub struct Finale {
    pub open: bool,
    /// what you just finished
    from: Option<Meta>,
    picks: Vec<Meta>,
    sel: usize,
    /// when it opened (the fade in), when the pick changed (its cross-fade)
    at: Instant,
    sel_at: Instant,
    /// the pick's trailer: (owner, url)
    clip: Option<(String, String)>,
    /// picks are on their way
    loading: bool,
}

const OWNER: &str = "finale:";

impl Finale {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self { open: false, from: None, picks: vec![], sel: 0, at: Instant::now(), sel_at: Instant::now(), clip: None, loading: false }
    }

    pub fn show(&mut self, from: Meta, cx: &mut Context<Self>) {
        if self.open && self.from.as_ref().map(|f| &f.id) == Some(&from.id) {
            return;
        }
        self.open = true;
        self.at = Instant::now();
        self.picks.clear();
        self.sel = 0;
        self.clip = None;
        self.loading = true;
        let (kind, id) = (from.kind.clone(), from.id.clone());
        self.from = Some(from);
        cx.spawn(async move |this, cx| {
            let picks = picks_for(&kind, &id).await;
            let _ = this.update(cx, |f, cx| {
                f.loading = false;
                f.picks = picks;
                f.select(0, cx);
            });
        })
        .detach();
        cx.notify();
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        self.open = false;
        self.clip = None;
        let v = crate::ui::video::get(cx);
        v.update(cx, |v, cx| v.stop_prefix(OWNER, cx));
        cx.notify();
    }

    /// another pick: its backdrop cross-fades in, then its trailer
    pub fn select(&mut self, i: usize, cx: &mut Context<Self>) {
        let Some(m) = self.picks.get(i).cloned() else {
            cx.notify();
            return;
        };
        self.sel = i;
        self.sel_at = Instant::now();
        self.clip = None;
        let v = crate::ui::video::get(cx);
        v.update(cx, |v, cx| v.stop_prefix(OWNER, cx));
        cx.spawn(async move |this, cx| {
            // a beat on the still first
            crate::timer::after(std::time::Duration::from_millis(1600)).await;
            let still = this.update(cx, |f, _| f.open && f.sel == i).unwrap_or(false);
            if !still {
                return;
            }
            let url = api::call::<Option<String>>("trailer_get", json!({ "kind": m.kind, "id": m.id }))
                .await
                .ok()
                .flatten()
                .filter(|k| !k.is_empty())
                .map(|k| format!("https://www.youtube.com/watch?v={k}"));
            let Some(url) = url else { return };
            let owner = format!("{OWNER}{}", m.id);
            let _ = this.update(cx, |f, cx| {
                if f.open && f.sel == i {
                    f.clip = Some((owner.clone(), url.clone()));
                    let v = crate::ui::video::get(cx);
                    v.update(cx, |x, cx| x.play_once(&owner, &url, cx));
                    cx.notify();
                }
            });
        })
        .detach();
        cx.notify();
    }

    pub fn step(&mut self, by: i32, cx: &mut Context<Self>) {
        if self.picks.is_empty() {
            return;
        }
        let n = self.picks.len() as i32;
        let i = (self.sel as i32 + by).rem_euclid(n) as usize;
        self.select(i, cx);
    }

    /// Enter: play the pick
    pub fn play(&mut self, cx: &mut Context<Self>) {
        if let Some(m) = self.picks.get(self.sel).cloned() {
            self.close(cx);
            Nav::play(&m, cx);
        }
    }
}

/// More like what you just watched, with what the billboard needs (logo,
/// backdrop, words). Ones you've already finished are left out.
async fn picks_for(kind: &str, id: &str) -> Vec<Meta> {
    let extra = api::call::<Value>("meta_enrich", json!({ "kind": kind, "id": id })).await.unwrap_or_default();
    let recs: Vec<Meta> = extra["recommendations"].as_array().into_iter().flatten().filter_map(|r| serde_json::from_value(r.clone()).ok()).collect();
    let lib = api::call::<Vec<Value>>("library_list", json!({})).await.unwrap_or_default();
    let done: std::collections::HashSet<String> = lib
        .iter()
        .filter(|i| i["state"]["timesWatched"].as_i64().unwrap_or(0) > 0 || crate::api::flag(&i["state"]["flaggedWatched"]))
        .filter_map(|i| i["_id"].as_str().map(String::from))
        .collect();
    let full = futures::future::join_all(recs.into_iter().take(12).map(|r| async move {
        let mut m = r.clone();
        if m.id.starts_with("tmdb:") {
            let v = api::call::<Value>("tmdb_resolve", json!({ "kind": r.kind, "id": r.id })).await.ok()?;
            m.id = v["id"].as_str()?.to_string();
            m.kind = v["type"].as_str().unwrap_or(&r.kind).to_string();
        }
        let v = api::call::<Value>("meta_get", json!({ "kind": m.kind, "id": m.id, "hint": Value::Null })).await.ok()?;
        let mut f: Meta = serde_json::from_value(v).ok()?;
        if f.id.is_empty() {
            f.id = m.id.clone();
        }
        if f.kind.is_empty() {
            f.kind = m.kind.clone();
        }
        // the billboard needs a picture
        f.background.as_ref()?;
        Some(f)
    }))
    .await;
    full.into_iter().flatten().filter(|m| !done.contains(&m.id)).take(6).collect()
}

impl Render for Finale {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let vs = window.viewport_size();
        let vw = f32::from(vs.width);
        let ease = |t: f32| 1. - (1. - t.clamp(0., 1.)).powi(3);
        let a = ease(self.at.elapsed().as_secs_f32() / 0.7);
        let b = ease(self.sel_at.elapsed().as_secs_f32() / 0.5);
        if a < 1. || b < 1. {
            window.request_animation_frame();
        }
        let me = cx.entity().downgrade();
        let from = self.from.as_ref().map(|f| f.name.clone()).unwrap_or_default();
        let mut page = div()
            .id("finale")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .occlude()
            .bg(gpui::black())
            .opacity(a)
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation());
        match self.picks.get(self.sel).cloned() {
            Some(m) => {
                // the trailer, fading in through the backdrop
                let vid = crate::ui::video::get(cx);
                let clip = self.clip.as_ref().and_then(|(owner, _)| {
                    let owner = owner.clone();
                    vid.update(cx, |x, _| {
                        let f = x.frame(&owner, window)?;
                        let t = x.since.map_or(0., |t| (t.elapsed().as_secs_f32() / 1.2).min(1.));
                        Some((f, t))
                    })
                });
                if clip.as_ref().map_or(false, |c| c.1 < 1.) {
                    window.request_animation_frame();
                }
                let (p1, p2, p3) = (me.clone(), me.clone(), m.clone());
                let (pm, im) = (m.clone(), m.clone());
                let hero = crate::ui::hero::hero(
                    &m,
                    vs.height,
                    clip,
                    Box::new(move |_, _, cx| {
                        let pm = pm.clone();
                        let _ = p1.update(cx, |f, cx| f.close(cx));
                        Nav::play(&pm, cx);
                    }),
                    Box::new(move |_, _, cx| {
                        let im = im.clone();
                        let _ = p2.update(cx, |f, cx| f.close(cx));
                        Nav::detail(&im, cx);
                    }),
                    Box::new(move |_, _, cx| Nav::add(&p3, cx)),
                );
                page = page.child(div().absolute().top_0().left_0().size_full().opacity(0.35 + 0.65 * b).child(hero));
            }
            None => {
                // nothing to suggest (yet): the film you finished, dimmed
                page = page.child(
                    div().absolute().top_0().left_0().size_full().flex().items_center().justify_center().text_color(theme::label3()).text_size(px(18.)).child(if self.loading { "finding something for next…" } else { "that's everything for now" }),
                );
            }
        }
        // top: what you finished, and a way out
        let shut = me.clone();
        page = page
            .child(div().absolute().top_0().left_0().w_full().h(px(160.)).bg(linear_gradient(180., linear_color_stop(theme::black(0.7), 0.), linear_color_stop(theme::black(0.), 1.))))
            .child(
                div()
                    .absolute()
                    .top(px(34.))
                    .left(px(theme::GUTTER))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::label3()).child("you finished"))
                    .child(div().text_size(px(20.)).font_weight(FontWeight::BOLD).text_color(theme::text()).child(from)),
            )
            .child(
                div()
                    .id("finale-x")
                    .absolute()
                    .top(px(30.))
                    .right(px(theme::GUTTER))
                    .h(px(44.))
                    .px(px(18.))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .bg(theme::white(0.12))
                    .border_1()
                    .border_color(theme::white(0.14))
                    .text_size(px(15.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .cursor_pointer()
                    .hover(|s| s.bg(theme::white(0.22)))
                    .on_click(move |_, _, cx| {
                        let _ = shut.update(cx, |f, cx| f.close(cx));
                    })
                    .child(crate::ui::icon("close", 16.))
                    .child("done"),
            );
        // bottom: the other picks
        if self.picks.len() > 1 {
            let cw = ((vw - theme::GUTTER * 2. - 14. * 5.) / 6.).clamp(140., 200.);
            let ch = cw * 9. / 16.;
            let cards = self.picks.iter().enumerate().map(|(i, m)| {
                let on = i == self.sel;
                let art = m.background.clone().map(|u| crate::net::fit(&sized(&u, "w780"), cw));
                let pick = me.clone();
                div()
                    .id(SharedString::from(format!("finale-{i}")))
                    .flex_none()
                    .w(px(cw))
                    .h(px(ch))
                    .rounded(px(12.))
                    .overflow_hidden()
                    .relative()
                    .bg(theme::surface2())
                    .cursor_pointer()
                    .border_2()
                    .border_color(if on { theme::white(0.95) } else { theme::white(0.0) })
                    .opacity(if on { 1. } else { 0.62 })
                    .hover(|s| s.opacity(1.))
                    .on_click(move |_, _, cx| {
                        let _ = pick.update(cx, |f, cx| f.select(i, cx));
                    })
                    .children(art.map(|u| img(u).absolute().top_0().left_0().size_full().object_fit(ObjectFit::Cover)))
                    .child(div().absolute().left_0().bottom_0().w_full().h(px(ch * 0.6)).bg(linear_gradient(180., linear_color_stop(theme::black(0.), 0.), linear_color_stop(theme::black(0.8), 1.))))
                    .child(div().absolute().left(px(10.)).right(px(10.)).bottom(px(8.)).text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).line_clamp(1).text_ellipsis().child(m.name.clone()))
            });
            page = page.child(
                div()
                    .absolute()
                    .left(px(theme::GUTTER))
                    .bottom(px(20.))
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(theme::label2()).child("watch next"))
                    .child(div().flex().gap(px(14.)).children(cards)),
            );
        }
        page
    }
}
