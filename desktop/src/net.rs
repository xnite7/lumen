//! Images: GPUI asks for a URL, couch's image cache answers (the same disk
//! cache, DoH and headers the engine uses everywhere).
use gpui::http_client::{http, AsyncBody, HttpClient, Response, Url};
use std::sync::Arc;

pub struct CouchHttp {
    core: couch::core::CoreRef,
}

impl CouchHttp {
    pub fn new(core: couch::core::CoreRef) -> Arc<Self> {
        Arc::new(Self { core })
    }
}

impl HttpClient for CouchHttp {
    fn user_agent(&self) -> Option<&http::HeaderValue> {
        None
    }
    fn proxy(&self) -> Option<&Url> {
        None
    }
    fn send(&self, req: http::Request<AsyncBody>) -> futures::future::BoxFuture<'static, anyhow::Result<Response<AsyncBody>>> {
        let url = req.uri().to_string();
        let core = self.core.clone();
        Box::pin(async move {
            let got = couch::rt::spawn(async move { couch::images::load(&core, &url).await.map_err(|e| e.to_string()) }).await?.map_err(anyhow::Error::msg)?;
            let (bytes, ct) = got;
            Ok(Response::builder().status(200).header("content-type", ct).body(AsyncBody::from(bytes))?)
        })
    }
}
