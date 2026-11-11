//! Files baked into the app: fonts, icons, the couch mark.
use gpui::{AssetSource, Result, SharedString};
use std::borrow::Cow;

pub struct Assets;

static FILES: &[(&str, &[u8])] = &include!(concat!(env!("OUT_DIR"), "/assets.rs"));

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(FILES.iter().find(|(p, _)| *p == path).map(|(_, b)| Cow::Borrowed(*b)))
    }
    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(FILES.iter().filter(|(p, _)| p.starts_with(path)).map(|(p, _)| SharedString::from(*p)).collect())
    }
}

pub fn fonts() -> Vec<Cow<'static, [u8]>> {
    FILES.iter().filter(|(p, _)| p.starts_with("fonts/")).map(|(_, b)| Cow::Borrowed(*b)).collect()
}

pub fn bytes(path: &str) -> Option<&'static [u8]> {
    FILES.iter().find(|(p, _)| *p == path).map(|(_, b)| *b)
}
