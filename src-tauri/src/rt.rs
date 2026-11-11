//! The async runtime every background task runs on. Inside the Tauri app it
//! is Tauri's own tokio runtime; anywhere else (the GPUI app) couch starts
//! one of its own.
use once_cell::sync::{Lazy, OnceCell};
use std::future::Future;
use tokio::runtime::Handle;
use tokio::task::JoinHandle;

static HANDLE: OnceCell<Handle> = OnceCell::new();
static OWN: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread().enable_all().thread_name("couch-rt").build().expect("tokio runtime")
});

/// Use this runtime (call once at startup; optional).
pub fn init(h: Handle) {
    let _ = HANDLE.set(h);
}

pub fn handle() -> Handle {
    HANDLE.get_or_init(|| OWN.handle().clone()).clone()
}

pub fn spawn<F>(f: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    handle().spawn(f)
}

pub fn spawn_blocking<F, R>(f: F) -> JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    handle().spawn_blocking(f)
}

pub fn block_on<F: Future>(f: F) -> F::Output {
    handle().block_on(f)
}
