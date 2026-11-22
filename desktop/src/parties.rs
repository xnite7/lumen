//! Watch parties: the list (Couch tab), new invites (the envelope), RSVPs.
use crate::api;
use gpui::{App, AppContext, Context, Entity, Global};
use serde_json::{json, Value};
use std::time::Duration;

#[derive(Default)]
pub struct Parties {
    pub list: Vec<Value>,
    /// invites waiting to be opened, oldest first
    pub invites: Vec<Value>,
    pub me: String,
}

pub struct PartiesHandle(pub Entity<Parties>);
impl Global for PartiesHandle {}

pub fn get(cx: &App) -> Entity<Parties> {
    cx.global::<PartiesHandle>().0.clone()
}

fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as f64
}

fn seen() -> Vec<String> {
    couch::state::get().core.db.get::<Vec<String>>("party_seen").unwrap_or_default()
}

pub fn mark_seen(id: &str) {
    let mut s = seen();
    if !s.iter().any(|x| x == id) {
        s.push(id.to_string());
    }
    if s.len() > 200 {
        s.drain(..s.len() - 200);
    }
    let _ = couch::state::get().core.db.set("party_seen", &s);
}

pub fn init(cx: &mut App) {
    let p = cx.new(|cx: &mut Context<Parties>| {
        cx.spawn(async move |this, cx| loop {
            let me = api::call::<Value>("discord_me", json!({})).await.ok().and_then(|v| v["id"].as_str().map(|s| s.to_string())).unwrap_or_default();
            if let Ok(r) = api::call::<Value>("crew_social", json!({ "method": "GET", "path": "/v1/parties", "body": Value::Null })).await {
                let list = r["parties"].as_array().cloned().unwrap_or_default();
                let s = seen();
                let ok = this
                    .update(cx, |p, cx| {
                        p.me = me.clone();
                        for party in &list {
                            let id = party["id"].as_str().unwrap_or("");
                            let fresh = party["host"].as_str() != Some(me.as_str())
                                && party["rsvp"][me.as_str()].is_null()
                                && !s.iter().any(|x| x == id)
                                && party["at"].as_f64().unwrap_or(0.) > now_ms() - 30. * 60_000.;
                            if fresh && !p.invites.iter().any(|x| x["id"] == party["id"]) {
                                p.invites.push(party.clone());
                            }
                        }
                        p.list = list;
                        cx.notify();
                    })
                    .is_ok();
                if !ok {
                    break;
                }
            }
            crate::timer::after(Duration::from_secs(60)).await;
        })
        .detach();
        let mut me = Parties::default();
        // dev: COUCH_FAKE_INVITE=1 shows a sample envelope
        if std::env::var("COUCH_FAKE_INVITE").is_ok() {
            me.invites.push(json!({
                "id": "dev1", "host": "1", "hostName": "nora", "at": now_ms() + 3. * 3600_000., "note": "bring snacks. no phones.",
                "title": { "id": "tt0903747", "type": "series", "name": "Breaking Bad", "year": "2008", "runtime": "47 min",
                    "poster": "https://images.metahub.space/poster/medium/tt0903747/img" },
                "rsvp": { "1": { "yes": true, "name": "nora" }, "2": { "yes": true, "name": "mika" } }, "invited": "all",
            }));
        }
        me
    });
    cx.set_global(PartiesHandle(p));
}

impl Parties {
    pub fn rsvp(&mut self, party: &Value, yes: bool, cx: &mut Context<Self>) {
        let id = party["id"].as_str().unwrap_or("").to_string();
        mark_seen(&id);
        self.invites.retain(|x| x["id"].as_str() != Some(id.as_str()));
        cx.notify();
        cx.spawn(async move |this, cx| {
            let r = api::call::<Value>("crew_social", json!({ "method": "POST", "path": format!("/v1/party/{id}/rsvp"), "body": { "yes": yes } })).await;
            if let Ok(v) = r {
                let _ = this.update(cx, |p, cx| {
                    for x in p.list.iter_mut() {
                        if x["id"] == v["party"]["id"] {
                            *x = v["party"].clone();
                        }
                    }
                    cx.notify();
                });
            }
        })
        .detach();
    }
    /// closed the envelope without answering: it waits on the Couch tab
    pub fn later(&mut self, party: &Value, cx: &mut Context<Self>) {
        let id = party["id"].as_str().unwrap_or("");
        mark_seen(id);
        self.invites.retain(|x| x["id"].as_str() != Some(id));
        cx.notify();
    }
}
