//! Emoji drawn as Microsoft's Fluent 3D set (MIT, github.com/microsoft/fluentui-emoji),
//! the same pictures the webview app used on PC.
use gpui::{div, img, prelude::*, px, AnyElement};
use once_cell::sync::Lazy;
use std::collections::HashMap;

const CDN: &str = "https://cdn.jsdelivr.net/gh/microsoft/fluentui-emoji@main/assets/";
static MAP: Lazy<HashMap<String, String>> = Lazy::new(|| serde_json::from_str(include_str!("../../../src/lib/fluent-map.json")).unwrap_or_default());

fn code(g: &str) -> String {
    g.chars().map(|c| format!("{:x}", c as u32)).collect::<Vec<_>>().join("-")
}

/// the Fluent 3D picture for one emoji
pub fn url(g: &str) -> Option<String> {
    let c = code(g);
    let rel = MAP.get(&c).or_else(|| MAP.get(&c.replace("-fe0f", ""))).or_else(|| MAP.get(&format!("{c}-fe0f")))?;
    let enc: Vec<String> = rel.split('/').map(|s| urlencoding::encode(s).into_owned()).collect();
    Some(format!("{CDN}{}", enc.join("/")))
}

/// one emoji as a picture (text if there's no picture for it)
pub fn emoji(g: &str, size: f32) -> AnyElement {
    match url(g) {
        Some(u) => img(u).size(px(size)).flex_none().into_any_element(),
        None => div().text_size(px(size * 0.85)).child(g.to_string()).into_any_element(),
    }
}

fn pictographic(c: char) -> bool {
    let u = c as u32;
    (0x1F000..=0x1FAFF).contains(&u) || (0x2600..=0x27BF).contains(&u) || (0x1F1E6..=0x1F1FF).contains(&u) || u == 0x2764 || u == 0x2B50
}

/// text with its emoji drawn as pictures, inline
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
