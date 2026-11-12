//! One catalog as a page: a streaming service's films/shows, a genre, any
//! addon catalog. A grid of posters that loads more as you scroll.
use crate::api::{self, sized, Meta};
use crate::theme;
use crate::ui::card;
use crate::Nav;
use gpui::{div, img, prelude::*, px, Context, FontWeight, ObjectFit, ScrollHandle, SharedString, Window};
use serde_json::{json, Value};

#[derive(Clone, PartialEq, Debug)]
pub struct Spec {
    pub title: String,
    /// a streaming service (TMDB provider id + logo): films/shows switch
    pub service: Option<(i64, Option<String>)>,
    pub catalog: Value,
    pub genre: Option<String>,
}

pub fn service_catalog(id: i64, kind: &str, name: &str) -> Value {
    json!({
        "key": format!("couch/{kind}/couch-prov-{id}"),
        "addonId": "com.midnite.couch.builtin",
        "addonName": "couch",
        "transportUrl": "couch://builtin/manifest.json",
        "type": kind,
        "id": format!("couch-prov-{id}"),
        "name": name,
        "extra": [{ "name": "skip" }],
        "adult": false,
        "builtin": true,
    })
}

pub struct Catalog {
    pub spec: Spec,
    kind: String,
    items: Vec<Meta>,
    loading: bool,
    more: bool,
    seq: u64,
    scroll: ScrollHandle,
}

impl Catalog {
    pub fn new(spec: Spec, cx: &mut Context<Self>) -> Self {
        let kind = if spec.service.is_some() { "movie".to_string() } else { spec.catalog["type"].as_str().unwrap_or("movie").to_string() };
        let mut me = Self { spec, kind, items: vec![], loading: false, more: true, seq: 0, scroll: ScrollHandle::new() };
        me.page(true, cx);
        me
    }

    fn cat(&self) -> Value {
        match &self.spec.service {
            Some((id, _)) => service_catalog(*id, &self.kind, &self.spec.title),
            None => self.spec.catalog.clone(),
        }
    }

    fn page(&mut self, reset: bool, cx: &mut Context<Self>) {
        if reset {
            self.seq += 1;
            self.items.clear();
            self.more = true;
            self.loading = false;
        }
        if self.loading || !self.more {
            return;
        }
        self.loading = true;
        let my = self.seq;
        let cat = self.cat();
        let mut extra: Vec<(String, String)> = vec![];
        if let Some(g) = &self.spec.genre {
            extra.push(("genre".into(), g.clone()));
        }
        if !self.items.is_empty() {
            extra.push(("skip".into(), self.items.len().to_string()));
        }
        cx.spawn(async move |this, cx| {
            let r = api::call::<Vec<Meta>>("catalog_fetch", json!({ "catalog": cat, "extra": extra })).await;
            let _ = this.update(cx, |v, cx| {
                if v.seq != my {
                    return;
                }
                v.loading = false;
                match r {
                    Ok(list) => {
                        let seen: std::collections::HashSet<String> = v.items.iter().map(|m| m.id.clone()).collect();
                        let fresh: Vec<Meta> = list.into_iter().filter(|m| !seen.contains(&m.id)).collect();
                        v.more = !fresh.is_empty();
                        v.items.extend(fresh);
                    }
                    Err(_) => v.more = false,
                }
                cx.notify();
            });
        })
        .detach();
    }
}

impl Render for Catalog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let g = theme::GUTTER;
        let vw = f32::from(window.viewport_size().width);
        let cols = (((vw - g * 2.) + 14.) / (180. + 14.)).floor().max(2.);
        let w = ((vw - g * 2.) - (cols - 1.) * 14.) / cols;
        // near the bottom: the next page
        let max = f32::from(self.scroll.max_offset().height);
        let off = -f32::from(self.scroll.offset().y);
        if self.more && !self.loading && !self.items.is_empty() && max - off < 900. {
            self.page(false, cx);
        }
        let head = match &self.spec.service {
            Some((_, logo)) => {
                let svc = logo.clone();
                let kinds = [("movie", "films"), ("series", "shows")];
                div()
                    .flex()
                    .items_center()
                    .gap(px(20.))
                    .child(match svc {
                        Some(u) => img(u).h(px(54.)).max_w(px(260.)).object_fit(ObjectFit::Contain).into_any_element(),
                        None => div().text_size(px(40.)).font_weight(FontWeight::BLACK).child(self.spec.title.clone()).into_any_element(),
                    })
                    .child(div().flex().gap(px(6.)).p(px(4.)).rounded_full().bg(theme::white(0.08)).children(kinds.into_iter().map(|(k, l)| {
                        let on = self.kind == k;
                        div()
                            .id(SharedString::from(format!("ck-{k}")))
                            .px(px(16.))
                            .h(px(34.))
                            .flex()
                            .items_center()
                            .rounded_full()
                            .cursor_pointer()
                            .font_weight(FontWeight::BOLD)
                            .when(on, |d| d.bg(theme::text()).text_color(gpui::rgb(0x000000)))
                            .on_click(cx.listener(move |v, _, _, cx| {
                                if v.kind != k {
                                    v.kind = k.into();
                                    v.page(true, cx);
                                    cx.notify();
                                }
                            }))
                            .child(l)
                    })))
                    .into_any_element()
            }
            None => div()
                .flex()
                .flex_col()
                .child(div().text_size(px(40.)).font_weight(FontWeight::BLACK).child(self.spec.genre.clone().unwrap_or(self.spec.title.clone()).to_lowercase()))
                .children(self.spec.genre.as_ref().map(|_| div().text_size(px(15.)).text_color(theme::label2()).child(self.spec.title.to_lowercase())))
                .into_any_element(),
        };
        let grid = div().flex().flex_wrap().gap(px(14.)).children(self.items.iter().enumerate().map(|(i, m)| {
            let mm = m.clone();
            card::poster(SharedString::from(format!("cat-{i}")), m, w, move |_, _, cx| Nav::detail(&mm, cx))
        }));
        let _ = sized;
        div()
            .id("catalog")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(crate::ui::smooth::wheel(&self.scroll, "catalog"))
            .bg(theme::bg())
            .px(px(g))
            .pt(px(100.))
            .pb(px(80.))
            .flex()
            .flex_col()
            .gap(px(22.))
            .child(head)
            .child(grid)
            .when(self.loading, |d| d.child(div().text_color(theme::label2()).child("loading…")))
            .when(!self.loading && self.items.is_empty(), |d| d.child(div().text_color(theme::label2()).child("nothing here yet.")))
    }
}
