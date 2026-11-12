//! Waiting a while inside UI tasks (runs on couch's own runtime).
use std::time::{Duration, Instant};

pub async fn after(d: Duration) {
    // the sleep is made on couch's runtime (tokio timers need it)
    let _ = couch::rt::spawn(async move { tokio::time::sleep(d).await }).await;
}

pub async fn at(t: Instant) {
    after(t.saturating_duration_since(Instant::now())).await;
}
