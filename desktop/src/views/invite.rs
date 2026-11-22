//! A watch party invite, the paper way: an envelope slides onto the screen,
//! the wax seal cracks, the flap lifts and a ticket rises out with the film,
//! the time and who's hosting. "I'm in" stamps it ADMITTED and it flies off;
//! "can't make it" tucks it back in and the envelope slides away.
use crate::api::sized;
use crate::parties::{self, Parties};
use crate::theme;
use gpui::{
    div, img, prelude::*, px, Context, Entity, FontWeight, ImageSource, ObjectFit, Resource, SharedString, Window,
};
use serde_json::Value;
use std::time::Instant;

#[derive(Clone, Copy, PartialEq)]
enum Leave {
    In,
    Out,
    Later,
}

pub struct Invite {
    parties: Entity<Parties>,
    showing: Option<(Value, Option<Instant>)>,
    leaving: Option<(Leave, Instant)>,
}

fn art(name: &str) -> ImageSource {
    ImageSource::Resource(Resource::Embedded(SharedString::from(format!("party/{name}"))))
}

// easing
fn clamp01(x: f32) -> f32 {
    x.clamp(0., 1.)
}
fn out_cubic(t: f32) -> f32 {
    1. - (1. - t).powi(3)
}
fn in_cubic(t: f32) -> f32 {
    t * t * t
}
fn out_back(t: f32) -> f32 {
    let (c1, c3) = (1.4, 2.4);
    1. + c3 * (t - 1.).powi(3) + c1 * (t - 1.).powi(2)
}
/// progress of a phase [a, b] ms at time `ms`
fn ph(ms: f32, a: f32, b: f32) -> f32 {
    clamp01((ms - a) / (b - a))
}

impl Invite {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let parties = parties::get(cx);
        cx.observe(&parties, |v, p, cx| {
            if v.showing.is_none() {
                if let Some(first) = p.read(cx).invites.first().cloned() {
                    v.showing = Some((first, None));
                    v.leaving = None;
                }
            }
            cx.notify();
        })
        .detach();
        let showing = parties.read(cx).invites.first().cloned().map(|p| (p, None));
        Self { parties, showing, leaving: None }
    }

    fn leave(&mut self, how: Leave, cx: &mut Context<Self>) {
        if self.leaving.is_some() {
            return;
        }
        self.leaving = Some((how, Instant::now()));
        cx.notify();
    }

    fn finish(&mut self, cx: &mut Context<Self>) {
        let Some((party, _)) = self.showing.take() else { return };
        let how = self.leaving.take().map(|l| l.0);
        self.parties.update(cx, |p, cx| match how {
            Some(Leave::In) => p.rsvp(&party, true, cx),
            Some(Leave::Out) => p.rsvp(&party, false, cx),
            _ => p.later(&party, cx),
        });
        // the next one, if more came in
        if let Some(next) = self.parties.read(cx).invites.first().cloned() {
            self.showing = Some((next, None));
        }
        cx.notify();
    }
}

fn fmt_when(at_ms: f64) -> (String, String) {
    use chrono::TimeZone;
    match chrono::Local.timestamp_millis_opt(at_ms as i64).single() {
        Some(t) => (t.format("%a %b %-d").to_string().to_uppercase(), t.format("%-I:%M %p").to_string()),
        None => (String::new(), String::new()),
    }
}

fn hash(s: &str) -> u32 {
    s.chars().fold(7u32, |h, c| h.wrapping_mul(31).wrapping_add(c as u32))
}

impl Render for Invite {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some((p, t0)) = self.showing.clone() else { return div().into_any_element() };
        // the clock starts on the first frame actually drawn
        let t0 = match t0 {
            Some(t) => t,
            None => {
                let now = Instant::now();
                if let Some(s) = self.showing.as_mut() {
                    s.1 = Some(now);
                }
                now
            }
        };
        let vp = window.viewport_size();
        let (vw, vh) = (f32::from(vp.width), f32::from(vp.height));
        let ms = t0.elapsed().as_secs_f32() * 1000.;
        let lv = self.leaving.map(|(k, at)| (k, at.elapsed().as_secs_f32() * 1000.));
        // done leaving: hand over
        if let Some((k, lms)) = lv {
            let end = match k {
                Leave::In => 1700.,
                Leave::Out => 1250.,
                Leave::Later => 650.,
            };
            if lms >= end {
                cx.defer_in(window, |v, _, cx| v.finish(cx));
            }
        }
        let animating = ms < 3400. || lv.is_some();
        if animating {
            window.request_animation_frame();
        }

        // ------------------------------------------------------- geometry
        let ew = (vw - 32.).min(620.);
        let s = ew / 620.;
        let eh = 388. * s;
        let answers_h = 74.;
        let ex = (vw - ew) / 2.;
        let ey = ((vh - eh - answers_h) / 2.).max(140. * s) + 60. * s;

        // the envelope slides in from the right, a little high, overshooting
        let k_in = out_back(ph(ms, 0., 1000.));
        let mut dx = (1. - k_in) * (vw * 1.2);
        let mut dy = (1. - k_in) * (-0.1 * vh);
        if let Some((k, lms)) = lv {
            match k {
                Leave::In => dy -= in_cubic(ph(lms, 950., 1650.)) * vh * 1.3,
                Leave::Out => dx -= in_cubic(ph(lms, 550., 1200.)) * vw * 1.3,
                Leave::Later => dx -= in_cubic(ph(lms, 0., 600.)) * vw * 1.3,
            }
        }
        let (x0, y0) = (ex + dx, ey + dy);

        // seal: pops, then cracks apart
        let pop = out_back(ph(ms, 1100., 1500.));
        let crack = in_cubic(ph(ms, 1500., 2200.));
        // flap: folds up (closed shrinks, then the open one grows)
        let flap_close = 1. - out_cubic(ph(ms, 1700., 2000.));
        let flap_open = out_cubic(ph(ms, 2000., 2300.));
        // ticket: rises out behind the pocket, then comes in front and settles
        let th = eh * 0.82;
        let rise1 = out_cubic(ph(ms, 2050., 2545.));
        let rise2 = out_cubic(ph(ms, 2545., 2950.));
        let mut ty = -0.86 * th * rise1 + (0.86 - 0.5) * th * rise2;
        let mut in_front = ms >= 2545.;
        if let Some((Leave::Out, lms)) = lv {
            let k = out_cubic(ph(lms, 0., 500.));
            ty *= 1. - k;
            if k > 0.05 {
                in_front = false;
            }
        }
        let answers_a = if lv.is_some() { 0. } else { ph(ms, 2850., 3350.) };
        let dim_a = if let Some((k, lms)) = lv {
            let start = if k == Leave::Later { 300. } else { 1100. };
            1. - ph(lms, start, start + 500.)
        } else {
            ph(ms, 0., 500.)
        };

        // ------------------------------------------------------- contents
        let t = &p["title"];
        let title = t["name"].as_str().unwrap_or("").to_uppercase();
        let poster = t["poster"].as_str().map(|u| sized(u, "w342"));
        let (date, hour) = fmt_when(p["at"].as_f64().unwrap_or(0.));
        let pid = p["id"].as_str().unwrap_or("").to_string();
        let me_id = self.parties.read(cx).me.clone();
        let seat = format!("{}{}", (b'A' + (hash(&pid) % 13) as u8) as char, hash(&format!("{pid}{me_id}")) % 18 + 1);
        let number = format!("{:06}", hash(&format!("{pid}n")) % 1_000_000);
        let host = p["hostName"].as_str().unwrap_or("someone").to_string();
        let going = p["rsvp"].as_object().map_or(0, |o| o.values().filter(|r| r["yes"].as_bool() == Some(true)).count());
        let me_name = couch::state::get().core.db.get::<Value>("discord_user").and_then(|u| u["name"].as_str().map(|s| s.to_string())).unwrap_or("you".into());
        let meta = [t["year"].as_str().unwrap_or(""), if t["type"] == "series" { "SERIES" } else { "FEATURE" }, t["runtime"].as_str().unwrap_or("")]
            .into_iter()
            .filter(|x| !x.is_empty())
            .collect::<Vec<_>>()
            .join(" · ")
            .to_uppercase();
        let note = p["note"].as_str().filter(|n| !n.is_empty()).map(|n| n.to_string());
        let ink = gpui::rgb(0x3b1029);
        let pink = gpui::rgb(0xc4206f);
        let sp = |v: f32| px(v * s);

        let tw = ew * 0.9;
        let ticket = div()
            .absolute()
            .left(px(x0 + ew * 0.05))
            .top(px(y0 + eh * 0.08 + ty))
            .w(px(tw))
            .h(px(th))
            .child(img(art("ticket.webp")).absolute().top_0().left_0().size_full())
            .child(
                // the main part: poster, kicker, title, facts, the grid, host, note
                div()
                    .absolute()
                    .left(sp(16.))
                    .top(sp(16.))
                    .w(px(tw * 0.765 - 26. * s))
                    .h(px(th - 32. * s))
                    .flex()
                    .gap(sp(14.))
                    .text_color(ink)
                    .child(
                        div().w(sp(112.)).h_full().flex_none().rounded(sp(4.)).overflow_hidden().bg(gpui::rgb(0x2a1420)).border_2().border_color(gpui::rgb(0xf7eedd)).children(poster.clone().map(|u| img(u).size_full().object_fit(ObjectFit::Cover))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(sp(4.))
                            .child(div().flex().justify_between().text_size(sp(9.)).font_weight(FontWeight::BLACK).text_color(pink).child("MIDNITE™ COUCH").child("PRESENTS"))
                            .child(div().h(px(1.)).bg(gpui::rgba(0x3b10294d)))
                            .child(div().text_size(sp(30.)).line_height(sp(30.)).font_weight(FontWeight::BLACK).line_clamp(2).child(title))
                            .child(div().text_size(sp(10.)).font_weight(FontWeight::SEMIBOLD).opacity(0.6).child(meta))
                            .child(div().flex_1())
                            .child(
                                div()
                                    .flex()
                                    .border_t_1()
                                    .border_b_1()
                                    .border_color(gpui::rgba(0x3b102955))
                                    .children([("DATE", date.clone()), ("DOORS", hour.clone()), ("SEAT", format!("COUCH {seat}"))].into_iter().enumerate().map(|(i, (k, v))| {
                                        div()
                                            .flex_1()
                                            .py(sp(4.))
                                            .px(sp(4.))
                                            .when(i > 0, |d| d.border_l_1().border_color(gpui::rgba(0x3b102955)))
                                            .child(div().text_size(sp(8.)).font_weight(FontWeight::SEMIBOLD).opacity(0.6).child(k))
                                            .child(div().text_size(sp(14.)).font_weight(FontWeight::BLACK).child(v))
                                    })),
                            )
                            .child(div().flex().items_center().gap(sp(6.)).text_size(sp(11.)).child(
                                div().size(sp(16.)).rounded_full().bg(ink).text_color(gpui::rgb(0xf7eedd)).flex().items_center().justify_center().text_size(sp(9.)).font_weight(FontWeight::BLACK).child(host.chars().next().unwrap_or('?').to_string()),
                            ).child(format!("hosted by {host}{}", if going > 1 { format!(" · {} more going", going - 1) } else { String::new() })))
                            .children(note.map(|n| div().text_size(sp(12.)).italic().text_color(gpui::rgb(0x2a2a55)).child(format!("“{n}”")))),
                    ),
            )
            .child(
                // the stub: ADMIT ONE, the barcode, the number
                div()
                    .absolute()
                    .left(px(tw * 0.765 + 8. * s))
                    .top(sp(16.))
                    .w(px(tw * 0.235 - 24. * s))
                    .h(px(th - 32. * s))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(sp(10.))
                    .text_color(pink)
                    .child(div().text_size(sp(15.)).font_weight(FontWeight::BLACK).child("ADMIT"))
                    .child(div().text_size(sp(15.)).font_weight(FontWeight::BLACK).child("ONE"))
                    .child(div().flex().gap(px(1.5 * s)).h(sp(36.)).children((0..22).map(|i| {
                        let w = 1. + ((hash(&format!("{pid}bars")) >> (i % 16)) & 3) as f32 * 0.8;
                        div().w(px(w * s)).h_full().bg(ink)
                    })))
                    .child(div().text_size(sp(10.)).font_weight(FontWeight::SEMIBOLD).text_color(ink).child(format!("№ {number}"))),
            )
            .when(matches!(lv, Some((Leave::In, _))), |d| {
                let k = out_back(ph(lv.unwrap().1, 0., 420.));
                d.child(
                    div().absolute().top_0().left_0().size_full().flex().items_center().justify_center().child(
                        div()
                            .px(sp(16.))
                            .py(sp(4.))
                            .border_3()
                            .border_color(gpui::rgba(0xc4206fdd))
                            .rounded(sp(6.))
                            .text_color(gpui::rgba(0xc4206fdd))
                            .text_size(sp(26. + (1. - k) * 40.))
                            .font_weight(FontWeight::BLACK)
                            .opacity(k * 0.9)
                            .child("ADMITTED"),
                    ),
                )
            });

        let mut stage = div().absolute().top_0().left_0().size_full();
        // the open flap stands above the envelope, behind everything
        if flap_open > 0. {
            let fh = eh * 0.56 * flap_open;
            stage = stage.child(img(art("flap-open.webp")).absolute().left(px(x0)).top(px(y0 - fh)).w(px(ew)).h(px(fh)));
        }
        stage = stage.child(img(art("back.webp")).absolute().left(px(x0)).top(px(y0)).w(px(ew)).h(px(eh)));
        let mut ticket = Some(ticket);
        if !in_front {
            stage = stage.child(ticket.take().unwrap());
        }
        // the front pocket, with the address and the stamp
        stage = stage.child(
            div()
                .absolute()
                .left(px(x0))
                .top(px(y0))
                .w(px(ew))
                .h(px(eh))
                .child(img(art("front.webp")).absolute().top_0().left_0().size_full())
                .child(div().absolute().left(px(ew * 0.09)).bottom(px(eh * 0.2)).text_size(sp(26.)).italic().text_color(gpui::rgba(0x2a2a55dd)).child(format!("to {me_name}")))
                .child(div().absolute().left(px(ew * 0.09)).bottom(px(eh * 0.1)).text_size(sp(15.)).italic().text_color(gpui::rgba(0x2a2a5599)).child(format!("from {host}")))
                .child(img(art("postmark.png")).absolute().right(px(ew * 0.2)).bottom(px(eh * 0.2)).w(sp(120.)).h(sp(70.)).opacity(0.8))
                .child(
                    div()
                        .absolute()
                        .right(px(ew * 0.06))
                        .bottom(px(eh * 0.12))
                        .w(sp(86.))
                        .h(sp(104.))
                        .p(sp(5.))
                        .bg(gpui::rgb(0xfbf6ec))
                        .shadow_md()
                        .child(div().size_full().overflow_hidden().bg(gpui::rgb(0x34485e)).children(poster.map(|u| img(u).size_full().object_fit(ObjectFit::Cover)))),
                ),
        );
        if flap_close > 0. {
            stage = stage.child(img(art("flap-closed.webp")).absolute().left(px(x0)).top(px(y0)).w(px(ew)).h(px(eh * 0.56 * flap_close)));
        }
        // the wax seal, popping in then cracking apart
        if crack < 1. && pop > 0. {
            let sw = ew * 0.2 * (0.7 + 0.3 * pop);
            let cxp = x0 + ew / 2. - sw / 2.;
            let cyp = y0 + eh * 0.56 - sw / 2.;
            let a = (1. - crack) * clamp01(pop * 2.);
            stage = stage
                .child(img(art("seal-l.png")).absolute().left(px(cxp - crack * 40. * s)).top(px(cyp + crack * 70. * s)).size(px(sw)).opacity(a))
                .child(img(art("seal-r.png")).absolute().left(px(cxp + crack * 40. * s)).top(px(cyp + crack * 80. * s)).size(px(sw)).opacity(a));
        }
        if let Some(t) = ticket {
            stage = stage.child(t);
        }

        let parties = self.parties.clone();
        let _ = parties;
        let btn = |id: &'static str, label: &'static str, primary: bool| {
            div()
                .id(id)
                .h(px(48.))
                .px(px(26.))
                .flex()
                .items_center()
                .rounded_full()
                .text_size(px(16.))
                .font_weight(FontWeight::BLACK)
                .cursor_pointer()
                .when(primary, |d| d.bg(gpui::rgb(0xd9257f)).text_color(theme::text()).shadow_lg().hover(|s| s.bg(gpui::rgb(0xe8338f))))
                .when(!primary, |d| d.bg(gpui::rgb(0xd8c9ad)).text_color(gpui::rgb(0x3b1029)).hover(|s| s.bg(gpui::rgb(0xe6d8bc))))
                .child(label)
        };
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .child(
                div()
                    .id("invite-dim")
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full()
                    .bg(gpui::rgba(0x060308b8))
                    .opacity(dim_a)
                    .on_click(cx.listener(|v, _, _, cx| v.leave(Leave::Later, cx))),
            )
            .child(stage)
            .child(
                div()
                    .absolute()
                    .left_0()
                    .w_full()
                    .top(px(ey + eh + 26.))
                    .flex()
                    .justify_center()
                    .gap(px(12.))
                    .opacity(answers_a)
                    .child(btn("inv-yes", "I'm in", true).on_click(cx.listener(|v, _, _, cx| v.leave(Leave::In, cx))))
                    .child(btn("inv-no", "can't make it", false).on_click(cx.listener(|v, _, _, cx| v.leave(Leave::Out, cx))))
                    .child(
                        div()
                            .id("inv-later")
                            .h(px(48.))
                            .px(px(14.))
                            .flex()
                            .items_center()
                            .text_size(px(15.))
                            .text_color(theme::label2())
                            .cursor_pointer()
                            .hover(|s| s.text_color(theme::text()))
                            .on_click(cx.listener(|v, _, _, cx| v.leave(Leave::Later, cx)))
                            .child("decide later"),
                    ),
            )
            .into_any_element()
    }
}

