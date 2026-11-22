//! The first-time tour: a few steps that light up parts of couch and say what
//! they're for. Once, after signing in; skip any time.
use crate::theme;
use gpui::{div, prelude::*, px, Bounds, FontWeight, Pixels};
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    /// where the things the tour points at are on screen (painted last frame)
    static SPOTS: RefCell<HashMap<&'static str, Bounds<Pixels>>> = RefCell::new(HashMap::new());
}

/// Put inside something the tour may point at.
pub fn spot(name: &'static str) -> impl IntoElement {
    gpui::canvas(
        move |b, _, _| {
            SPOTS.with(|s| {
                s.borrow_mut().insert(name, b);
            });
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
}

pub fn done() -> bool {
    couch::state::try_get().and_then(|s| s.core.db.get::<bool>("gpui_tour_done")).unwrap_or(false)
}

pub fn finish() {
    let _ = couch::state::get().core.db.set("gpui_tour_done", &true);
}

/// (what it points at, title, words)
const STEPS: [(&str, &str, &str); 7] = [
    ("", "welcome to couch", "a quick look around. thirty seconds, promise."),
    ("tabs", "everything lives up here", "Home, Shows, Movies, your lists, the calendar, and the Couch. the magnifier searches anything."),
    ("billboard", "something for you", "the big one up top is picked for you. play it, or open it for more."),
    ("rows", "rest on a poster", "it grows and plays a little preview. click for the whole page, right-click for quick things like mark watched."),
    ("couch", "watch together", "the Couch shows who's watching right now. join them, or start a watch party from any title."),
    ("me", "you", "your profile, and settings: subtitles, the progress bar, how text looks, and more."),
    ("", "that's it", "enjoy. pull up a seat."),
];

/// The tour over the window: step `i` (None when it's over).
pub fn view(i: usize, vw: f32, vh: f32, on_step: impl Fn(Option<usize>, &mut gpui::App) + 'static + Clone) -> gpui::AnyElement {
    let (name, title, words) = STEPS[i.min(STEPS.len() - 1)];
    let target: Option<Bounds<Pixels>> = match name {
        "" => None,
        "rows" => Some(Bounds::new(gpui::point(px(0.), px(vh * 0.52)), gpui::size(px(vw), px(vh * 0.4)))),
        "billboard" => Some(Bounds::new(gpui::point(px(theme::GUTTER - 12.), px(vh * 0.16)), gpui::size(px(660.), px(vh * 0.36)))),
        n => SPOTS.with(|s| s.borrow().get(n).copied()),
    };
    let dim = theme::black(0.72);
    let mut root = div().id("tour").absolute().top_0().left_0().size_full().occlude();
    // the dark everywhere but the lit part, and a soft ring round it
    match target {
        Some(t) => {
            let p = 10.;
            let (x, y) = (f32::from(t.origin.x) - p, f32::from(t.origin.y) - p);
            let (w, h) = (f32::from(t.size.width) + p * 2., f32::from(t.size.height) + p * 2.);
            root = root
                .child(div().absolute().left_0().top_0().w_full().h(px(y.max(0.))).bg(dim))
                .child(div().absolute().left_0().top(px(y + h)).w_full().bottom_0().bg(dim))
                .child(div().absolute().left_0().top(px(y)).w(px(x.max(0.))).h(px(h)).bg(dim))
                .child(div().absolute().left(px(x + w)).top(px(y)).right_0().h(px(h)).bg(dim))
                .child(div().absolute().left(px(x)).top(px(y)).w(px(w)).h(px(h)).rounded(px(16.)).border_2().border_color(theme::white(0.85)));
        }
        None => root = root.child(div().absolute().size_full().bg(dim)),
    }
    // the card: under what's lit when that's high up, above it otherwise
    let cw = 380.;
    let (cx_, cy, below) = match target {
        Some(t) => {
            let mid = f32::from(t.origin.x) + f32::from(t.size.width) / 2.;
            let x = (mid - cw / 2.).clamp(24., vw - cw - 24.);
            let low = f32::from(t.origin.y) + f32::from(t.size.height);
            if low + 230. < vh { (x, low + 24., true) } else { (x, f32::from(t.origin.y) - 24., false) }
        }
        None => ((vw - cw) / 2., vh / 2. - 110., true),
    };
    let last = i + 1 >= STEPS.len();
    let (s1, s2, s3) = (on_step.clone(), on_step.clone(), on_step);
    let btn = |id: &'static str, label: &'static str, strong: bool| {
        div()
            .id(id)
            .px(px(16.))
            .h(px(36.))
            .flex()
            .items_center()
            .rounded(px(10.))
            .text_size(px(14.))
            .font_weight(FontWeight::SEMIBOLD)
            .cursor_pointer()
            .when(strong, |d| d.bg(theme::text()).text_color(gpui::rgb(0x000000)))
            .when(!strong, |d| d.bg(theme::white(0.1)).hover(|s| s.bg(theme::white(0.16))))
            .child(label)
    };
    let card = div()
        .absolute()
        .left(px(cx_))
        .when(below, |d| d.top(px(cy)))
        .when(!below, |d| d.bottom(px(vh - cy)))
        .w(px(cw))
        .p(px(20.))
        .rounded(px(18.))
        .bg(gpui::rgba(0x17121af7))
        .border_1()
        .border_color(theme::white(0.1))
        .shadow(vec![gpui::BoxShadow { color: theme::black(0.5), offset: gpui::point(px(0.), px(14.)), blur_radius: px(36.), spread_radius: px(0.), inset: false }])
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(div().text_size(px(20.)).font_weight(FontWeight::BLACK).child(title))
        .child(div().text_size(px(14.)).line_height(px(20.)).text_color(theme::label2()).child(words))
        .child(
            div()
                .mt(px(4.))
                .flex()
                .items_center()
                .gap(px(8.))
                // where you are
                .child(div().flex_1().flex().gap(px(5.)).children((0..STEPS.len()).map(|k| div().size(px(6.)).rounded_full().bg(if k == i { theme::text().into() } else { theme::white(0.25) }))))
                .when(!last, |d| d.child(btn("tour-skip", "skip", false).on_click(move |_, _, cx| s1(None, cx))))
                .when(i > 0, |d| d.child(btn("tour-back", "back", false).on_click(move |_, _, cx| s2(Some(i - 1), cx))))
                .child(btn("tour-next", if last { "let's go" } else { "next" }, true).on_click(move |_, _, cx| s3(if last { None } else { Some(i + 1) }, cx))),
        );
    root.child(card).into_any_element()
}
