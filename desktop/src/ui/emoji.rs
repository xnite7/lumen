//! Emoji rendered as text using the operating system's emoji font.
use gpui::{div, prelude::*, px, AnyElement};

/// One emoji rendered by the operating system's emoji font.
pub fn emoji(g: &str, size: f32) -> AnyElement {
    div().text_size(px(size)).child(g.to_string()).into_any_element()
}

fn pictographic(c: char) -> bool {
    let u = c as u32;
    (0x1F000..=0x1FAFF).contains(&u) || (0x2600..=0x27BF).contains(&u) || (0x1F1E6..=0x1F1FF).contains(&u) || u == 0x2764 || u == 0x2B50
}

/// Text containing emoji, rendered inline using the system emoji font.
pub fn text(s: &str, size: f32) -> AnyElement {
    if !s.chars().any(pictographic) {
        return div().child(s.to_string()).into_any_element();
    }
    let mut parts: Vec<AnyElement> = vec![];
    let mut run = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if pictographic(c) {
            let mut g = c.to_string();
            // variation selectors, skin tones, joiners
            while let Some(&n) = chars.peek() {
                let u = n as u32;
                if u == 0xFE0F || u == 0x200D || (0x1F3FB..=0x1F3FF).contains(&u) {
                    g.push(n);
                    chars.next();
                    if u == 0x200D {
                        if let Some(m) = chars.next() {
                            g.push(m);
                        }
                    }
                } else {
                    break;
                }
            }
            if !run.is_empty() {
                parts.push(div().child(std::mem::take(&mut run)).into_any_element());
            }
            parts.push(emoji(&g, size * 1.15));
        } else {
            run.push(c);
        }
    }
    if !run.is_empty() {
        parts.push(div().child(run).into_any_element());
    }
    div().flex().flex_wrap().items_center().gap(px(1.)).children(parts).into_any_element()
}
