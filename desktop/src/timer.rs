//! Waiting a while inside UI tasks (runs on couch's own runtime).
use std::time::{Duration, Instant};

pub async fn after(d: Duration) {
    let _ = couch::rt::spawn(tokio::time::sleep(d)).await;
}

pub async fn at(t: Instant) {
    after(t.saturating_duration_since(Instant::now())).await;
}
