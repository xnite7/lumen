//! "Your month on couch": a shareable card (a PNG) of a month's watching,
//! ready to paste into Discord. Hours, titles, films / shows, the top genre
//! and a wall of the posters, in couch's colours.
use crate::api;
use crate::theme;
use gpui::{div, img, prelude::*, px, Context, FontWeight, ObjectFit, Window};
use serde_json::{json, Value};
use std::sync::Arc;

const MONTHS: [&str; 12] = ["january", "february", "march", "april", "may", "june", "july", "august", "september", "october", "november", "december"];

pub struct Month {
    pub open: bool,
    year: i32,
    /// 1..=12
    month: u32,
    card: Option<Arc<gpui::Image>>,
    png: Option<Vec<u8>>,
    busy: bool,
    empty: bool,
}

impl Month {
    pub fn new(_cx: &mut Context<Self>) -> Self {
        Self { open: false, year: 2026, month: 1, card: None, png: None, busy: false, empty: false }
    }

    /// Open on the month that just ended (in its first week), else this one.
    pub fn show(&mut self, cx: &mut Context<Self>) {
        use chrono::Datelike;
        let now = chrono::Local::now();
        let (mut y, mut m) = (now.year(), now.month());
        if now.day() <= 7 {
            if m == 1 {
                y -= 1;
                m = 12;
            } else {
                m -= 1;
            }
        }
        self.year = y;
        self.month = m;
        self.open = true;
        self.make(cx);
    }

    fn shift(&mut self, by: i32, cx: &mut Context<Self>) {
        let mut i = self.year * 12 + self.month as i32 - 1 + by;
        use chrono::Datelike;
        let now = chrono::Local::now();
        i = i.min(now.year() * 12 + now.month() as i32 - 1);
        self.year = i.div_euclid(12);
        self.month = (i.rem_euclid(12) + 1) as u32;
        self.make(cx);
    }

    fn make(&mut self, cx: &mut Context<Self>) {
        self.busy = true;
        self.card = None;
        self.png = None;
        self.empty = false;
        cx.notify();
        let (y, m) = (self.year, self.month);
        cx.spawn(async move |this, cx| {
            let png = card(y, m).await;
            let _ = this.update(cx, |me, cx| {
                if (me.year, me.month) != (y, m) {
                    return;
                }
                me.busy = false;
                match png {
                    Some(b) => {
                        me.card = Some(Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Png, b.clone())));
                        me.png = Some(b);
                    }
                    None => me.empty = true,
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn file_name(&self) -> String {
        format!("couch-{}-{:02}.png", self.year, self.month)
    }
}

/// What a month was: (titles, films, shows, episodes, hours, top genre, posters)
async fn card(year: i32, month: u32) -> Option<Vec<u8>> {
    let rows = api::call::<Vec<Value>>("wrapped_data", json!({ "year": year })).await.unwrap_or_default();
    let pre = format!("{year:04}-{month:02}");
    let mut rows: Vec<Value> = rows.into_iter().filter(|r| r["at"].as_str().map_or(false, |a| a.starts_with(&pre))).collect();
    if rows.is_empty() {
        return None;
    }
    // finished first, then the newest
    rows.sort_by_key(|r| (r["status"] != "completed", std::cmp::Reverse(r["at"].as_str().unwrap_or("").to_string())));
    let films = rows.iter().filter(|r| r["type"] == "movie").count();
    let shows = rows.iter().filter(|r| r["type"] == "series").count();
    let eps: i64 = rows.iter().filter(|r| r["type"] == "series").map(|r| r["eps"].as_i64().unwrap_or(0)).sum();
    let mins: i64 = rows
        .iter()
        .map(|r| {
            r["watchedMinutes"].as_i64().unwrap_or_else(|| {
                let m = r["minutes"].as_i64().unwrap_or(if r["type"] == "movie" { 110 } else { 40 });
                if r["type"] == "series" { m * r["eps"].as_i64().unwrap_or(1).max(1) } else { m }
            })
        })
        .sum();
    let mut genres: std::collections::HashMap<String, usize> = Default::default();
    for r in &rows {
        for g in r["genres"].as_array().into_iter().flatten().filter_map(|g| g.as_str()) {
            *genres.entry(g.to_lowercase()).or_default() += 1;
        }
    }
    let top_genre = genres.into_iter().max_by_key(|g| g.1).map(|g| g.0);
    // the posters, fetched and inlined
    let core = couch::state::get().core.clone();
    let mut posters: Vec<String> = vec![];
    for r in rows.iter().filter(|r| r["poster"].as_str().is_some()).take(8) {
        let u = api::sized(r["poster"].as_str().unwrap_or(""), "w342");
        let c = core.clone();
        if let Ok(Ok((b, _))) = couch::rt::spawn(async move { couch::images::load(&c, &u).await }).await {
            if let Ok(im) = image::load_from_memory(&b) {
                let im = im.resize_to_fill(300, 450, image::imageops::FilterType::CatmullRom);
                let mut out = std::io::Cursor::new(Vec::new());
                if im.to_rgb8().write_to(&mut out, image::ImageFormat::Jpeg).is_ok() {
                    use base64::Engine;
                    posters.push(base64::engine::general_purpose::STANDARD.encode(out.into_inner()));
                }
            }
        }
    }
    let me: Value = core.db.get("discord_user").unwrap_or_default();
    let who = me["global_name"].as_str().or(me["name"].as_str()).or(me["username"].as_str()).unwrap_or("you").to_string();
    let svg = svg(year, month, rows.len(), films, shows, eps, mins / 60, top_genre, &posters, &who);
    couch::rt::spawn_blocking(move || render(&svg)).await.ok().flatten()
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[allow(clippy::too_many_arguments)]
fn svg(year: i32, month: u32, titles: usize, films: usize, shows: usize, eps: i64, hours: i64, genre: Option<String>, posters: &[String], who: &str) -> String {
    let (w, h) = (1080, 1350);
    let mname = MONTHS[(month as usize - 1) % 12];
    let stat = |x: i32, y: i32, n: String, label: &str| {
        format!(r##"<text x="{x}" y="{y}" font-family="Satoshi" font-weight="900" font-size="86" fill="#ffffff">{n}</text><text x="{x}" y="{}" font-family="Satoshi" font-weight="500" font-size="28" fill="#b9a9bd">{label}</text>"##, y + 44)
    };
    let mut s = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">
<defs>
<linearGradient id="bg" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#170b1a"/><stop offset="1" stop-color="#07050a"/></linearGradient>
<radialGradient id="glow" cx="0.85" cy="0.05" r="0.75"><stop offset="0" stop-color="#e0378a" stop-opacity="0.55"/><stop offset="1" stop-color="#e0378a" stop-opacity="0"/></radialGradient>
<linearGradient id="fade" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#07050a" stop-opacity="0"/><stop offset="0.62" stop-color="#07050a" stop-opacity="1"/></linearGradient>
</defs>
<rect width="{w}" height="{h}" fill="url(#bg)"/>
<rect width="{w}" height="{h}" fill="url(#glow)"/>
<text x="80" y="120" font-family="Satoshi" font-weight="700" font-size="30" fill="#e0378a" letter-spacing="2">midnite couch</text>
<text x="80" y="230" font-family="Satoshi" font-weight="900" font-size="{big}" fill="#ffffff">{who}'s {mname}</text>
<text x="80" y="290" font-family="Satoshi" font-weight="500" font-size="34" fill="#b9a9bd">on the couch, {year}</text>
"##,
        who = esc(&who.to_lowercase()),
        // long names get a smaller headline, so it always fits the card
        big = (104. * (16. / (who.chars().count() + mname.len() + 3).max(16) as f32)).round().max(44.),
    );
    s.push_str(&stat(80, 430, format!("{hours}h"), "watched"));
    s.push_str(&stat(400, 430, titles.to_string(), if titles == 1 { "title" } else { "titles" }));
    s.push_str(&stat(720, 430, films.to_string(), if films == 1 { "film" } else { "films" }));
    s.push_str(&stat(80, 590, shows.to_string(), if shows == 1 { "show" } else { "shows" }));
    if eps > 0 {
        s.push_str(&stat(400, 590, eps.to_string(), "episodes"));
    }
    if let Some(g) = genre {
        s.push_str(&format!(r##"<text x="80" y="730" font-family="Satoshi" font-weight="500" font-size="34" fill="#b9a9bd">mostly <tspan font-weight="900" fill="#ffffff">{}</tspan></text>"##, esc(&g)));
    }
    // the posters: a row of four, then another, sliding under a fade at the bottom
    let (pw, ph, gap) = (215, 322, 20);
    for (i, p) in posters.iter().take(8).enumerate() {
        let (col, row) = ((i % 4) as i32, (i / 4) as i32);
        let x = 80 + col * (pw + gap);
        let y = 790 + row * (ph + gap);
        s.push_str(&format!(
            r##"<clipPath id="c{i}"><rect x="{x}" y="{y}" width="{pw}" height="{ph}" rx="18"/></clipPath><image x="{x}" y="{y}" width="{pw}" height="{ph}" preserveAspectRatio="xMidYMid slice" clip-path="url(#c{i})" href="data:image/jpeg;base64,{p}"/>"##
        ));
    }
    s.push_str(&format!(r##"<rect x="0" y="1060" width="{w}" height="290" fill="url(#fade)"/>"##));
    s.push_str(r##"<text x="80" y="1290" font-family="Satoshi" font-weight="700" font-size="30" fill="#ffffff" fill-opacity="0.75">couch · midnite™</text>"##);
    s.push_str("</svg>");
    s
}

/// SVG to PNG, with couch's own fonts
fn render(svg: &str) -> Option<Vec<u8>> {
    let mut opt = resvg::usvg::Options::default();
    let mut db = resvg::usvg::fontdb::Database::new();
    for f in ["fonts/Satoshi-Black.ttf", "fonts/Satoshi-Bold.ttf", "fonts/Satoshi-Medium.ttf", "fonts/Satoshi-Regular.ttf"] {
        if let Some(b) = crate::assets::bytes(f) {
            db.load_font_data(b.to_vec());
        }
    }
    opt.fontdb = Arc::new(db);
    let tree = resvg::usvg::Tree::from_str(svg, &opt).ok()?;
    let size = tree.size().to_int_size();
    let mut pm = resvg::tiny_skia::Pixmap::new(size.width(), size.height())?;
    resvg::render(&tree, resvg::tiny_skia::Transform::default(), &mut pm.as_mut());
    pm.encode_png().ok()
}

impl Render for Month {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().into_any_element();
        }
        let vh = f32::from(window.viewport_size().height);
        let ch = (vh - 200.).clamp(300., 900.);
        let btn = |id: &'static str, label: String, strong: bool| {
            div()
                .id(id)
                .px(px(18.))
                .h(px(40.))
                .flex()
                .items_center()
                .rounded(px(12.))
                .text_size(px(14.))
                .font_weight(FontWeight::SEMIBOLD)
                .cursor_pointer()
                .when(strong, |d| d.bg(theme::text()).text_color(gpui::rgb(0x000000)))
                .when(!strong, |d| d.bg(theme::white(0.1)).hover(|s| s.bg(theme::white(0.16))))
                .child(label)
        };
        let title = format!("{} {}", MONTHS[(self.month as usize - 1) % 12], self.year);
        div()
            .id("month")
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .occlude()
            .bg(theme::black(0.82))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(18.))
            .on_click(cx.listener(|m, _, _, cx| {
                m.open = false;
                cx.notify();
            }))
            .child(
                div()
                    .id("month-card")
                    .h(px(ch))
                    .w(px(ch * 0.8))
                    .rounded(px(18.))
                    .overflow_hidden()
                    .bg(theme::white(0.04))
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_click(|_, _, cx| cx.stop_propagation())
                    .child(match (&self.card, self.busy, self.empty) {
                        (Some(c), _, _) => img(c.clone()).size_full().object_fit(ObjectFit::Contain).into_any_element(),
                        (_, true, _) => div().text_color(theme::label2()).child("putting your month together…").into_any_element(),
                        _ => div().text_color(theme::label2()).child(format!("nothing watched in {title} yet.")).into_any_element(),
                    }),
            )
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(btn("month-prev", "‹".into(), false).on_click(cx.listener(|m, _, _, cx| {
                        cx.stop_propagation();
                        m.shift(-1, cx);
                    })))
                    .child(div().w(px(170.)).flex().items_center().justify_center().font_weight(FontWeight::BLACK).child(title))
                    .child(btn("month-next", "›".into(), false).on_click(cx.listener(|m, _, _, cx| {
                        cx.stop_propagation();
                        m.shift(1, cx);
                    })))
                    .child(div().w(px(16.)))
                    .when(self.png.is_some(), |d| {
                        d.child(btn("month-copy", "copy for Discord".into(), true).on_click(cx.listener(|m, _, _, cx| {
                            cx.stop_propagation();
                            if let Some(c) = &m.card {
                                cx.write_to_clipboard(gpui::ClipboardItem::new_image(c));
                                crate::Nav::toast("copied. paste it in Discord".into(), cx);
                            }
                        })))
                        .child(btn("month-save", "save".into(), false).on_click(cx.listener(|m, _, _, cx| {
                            cx.stop_propagation();
                            let Some(b) = m.png.clone() else { return };
                            let dir = dirs::picture_dir().unwrap_or_else(std::env::temp_dir).join("couch");
                            let path = dir.join(m.file_name());
                            let ok = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&path, b)).is_ok();
                            crate::Nav::toast(if ok { format!("saved to Pictures\\couch\\{}", m.file_name()) } else { "couldn't save it".into() }, cx);
                        })))
                    })
                    .child(btn("month-close", "close".into(), false).on_click(cx.listener(|m, _, _, cx| {
                        cx.stop_propagation();
                        m.open = false;
                        cx.notify();
                    }))),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn card_renders() {
        use base64::Engine;
        let im = image::RgbImage::from_fn(300, 450, |x, y| image::Rgb([(x / 2) as u8, (y / 2) as u8, 120]));
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(im).write_to(&mut out, image::ImageFormat::Jpeg).unwrap();
        let p = base64::engine::general_purpose::STANDARD.encode(out.into_inner());
        let ps = vec![p; 6];
        let s = super::svg(2026, 10, 14, 5, 9, 63, 41, Some("thriller".into()), &ps, "sultan saud");
        let png = super::render(&s).expect("renders");
        std::fs::write("/tmp/month-card.png", &png).ok();
        assert!(png.len() > 10_000);
    }
}
