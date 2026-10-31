//! A tiny local HTTP relay for video streams.
//!
//! mpv and AVPlayer look hosts up with the system DNS, so a stream whose host
//! is DNS-blocked fails to open ("loading failed") even though Lumen itself
//! reaches it fine over encrypted DNS. Streams are handed to the player as
//! `http://127.0.0.1:<port>/s/<token>` instead; each request (including
//! the Range requests a seek makes) is forwarded through Lumen's own HTTP
//! client and the bytes streamed straight back.

use futures::StreamExt;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

struct Target {
    url: String,
    headers: Vec<(String, String)>,
}

pub struct Relay {
    port: u16,
    targets: Mutex<HashMap<String, Arc<Target>>>,
}

impl Relay {
    /// `client` hands out the HTTP client to use (Lumen's, with encrypted DNS).
    pub async fn start<F>(client: F) -> std::io::Result<Arc<Self>>
    where
        F: Fn() -> reqwest::Client + Send + Sync + 'static,
    {
        let client = Arc::new(client);
        let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
        let port = listener.local_addr()?.port();
        let relay = Arc::new(Self { port, targets: Mutex::new(HashMap::new()) });
        let r = relay.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                let Ok((sock, _)) = listener.accept().await else { continue };
                let r = r.clone();
                let http = client();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = r.serve(http, sock).await {
                        log::debug!("relay: {e}");
                    }
                });
            }
        });
        log::info!("stream relay on 127.0.0.1:{port}");
        Ok(relay)
    }

    /// Register a stream; returns the local address the player should open.
    pub fn register(&self, url: &str, headers: &[(String, String)]) -> String {
        let token = format!("{:016x}", rand_u64());
        let ext = url.split('?').next().and_then(|p| p.rsplit('/').next()).and_then(|f| f.rsplit_once('.')).map(|(_, e)| e).filter(|e| e.len() <= 4 && e.chars().all(|c| c.is_ascii_alphanumeric())).unwrap_or("bin");
        let mut t = self.targets.lock();
        if t.len() > 64 {
            t.clear();
        }
        t.insert(token.clone(), Arc::new(Target { url: url.to_string(), headers: headers.to_vec() }));
        format!("http://127.0.0.1:{}/s/{token}/video.{ext}", self.port)
    }

    async fn serve(&self, client: reqwest::Client, mut sock: TcpStream) -> anyhow::Result<()> {
        // read the request head
        let mut buf = Vec::with_capacity(2048);
        let mut chunk = [0u8; 2048];
        loop {
            let n = sock.read(&mut chunk).await?;
            if n == 0 {
                return Ok(());
            }
            buf.extend_from_slice(&chunk[..n]);
            if buf.windows(4).any(|w| w == b"\r\n\r\n") || buf.len() > 32 * 1024 {
                break;
            }
        }
        let head = String::from_utf8_lossy(&buf).to_string();
        let mut lines = head.split("\r\n");
        let first = lines.next().unwrap_or_default();
        let mut parts = first.split_whitespace();
        let method = parts.next().unwrap_or("GET").to_string();
        let path = parts.next().unwrap_or("/").to_string();
        let range = lines
            .clone()
            .find_map(|l| l.split_once(':').filter(|(k, _)| k.trim().eq_ignore_ascii_case("range")).map(|(_, v)| v.trim().to_string()));

        let token = path.trim_start_matches("/s/").split('/').next().unwrap_or_default().to_string();
        let target = self.targets.lock().get(&token).cloned();
        let Some(target) = target else {
            sock.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await?;
            return Ok(());
        };

        let mut req = if method == "HEAD" { client.head(&target.url) } else { client.get(&target.url) };
        for (k, v) in &target.headers {
            req = req.header(k.as_str(), v.as_str());
        }
        if let Some(r) = &range {
            req = req.header("Range", r.as_str());
        }
        let res = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                log::warn!("relay upstream: {e}");
                sock.write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await?;
                return Ok(());
            }
        };

        let status = res.status();
        let mut out = format!("HTTP/1.1 {} {}\r\n", status.as_u16(), status.canonical_reason().unwrap_or(""));
        for name in ["content-type", "content-length", "content-range", "accept-ranges", "last-modified", "etag"] {
            if let Some(v) = res.headers().get(name).and_then(|v| v.to_str().ok()) {
                out.push_str(&format!("{name}: {v}\r\n"));
            }
        }
        if res.headers().get("accept-ranges").is_none() {
            out.push_str("accept-ranges: bytes\r\n");
        }
        out.push_str("connection: close\r\n\r\n");
        sock.write_all(out.as_bytes()).await?;
        if method == "HEAD" {
            return Ok(());
        }
        let mut body = res.bytes_stream();
        while let Some(chunk) = body.next().await {
            // the player hanging up mid-stream (a seek) ends this request
            if sock.write_all(&chunk?).await.is_err() {
                break;
            }
        }
        Ok(())
    }
}

fn rand_u64() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn relays_with_ranges() {
        // upstream: a tiny server that honours Range
        let up = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let up_port = up.local_addr().unwrap().port();
        let body: Vec<u8> = (0..100_000u32).map(|i| (i % 251) as u8).collect();
        let b2 = body.clone();
        tokio::spawn(async move {
            loop {
                let (mut s, _) = up.accept().await.unwrap();
                let body = b2.clone();
                tokio::spawn(async move {
                    let mut buf = [0u8; 4096];
                    let n = s.read(&mut buf).await.unwrap();
                    let head = String::from_utf8_lossy(&buf[..n]).to_lowercase();
                    let (start, status) = head.lines().find_map(|l| l.strip_prefix("range: bytes=")).and_then(|r| r.split('-').next()?.parse::<usize>().ok()).map(|s| (s, 206)).unwrap_or((0, 200));
                    let part = &body[start..];
                    let hdr = format!("HTTP/1.1 {status} X\r\ncontent-length: {}\r\ncontent-range: bytes {start}-{}/{}\r\ncontent-type: video/mp4\r\n\r\n", part.len(), body.len() - 1, body.len());
                    s.write_all(hdr.as_bytes()).await.unwrap();
                    s.write_all(part).await.unwrap();
                });
            }
        });

        let relay = Relay::start(reqwest::Client::new).await.unwrap();
        let local = relay.register(&format!("http://127.0.0.1:{up_port}/movie.mkv?sig=1"), &[]);
        assert!(local.ends_with("/video.mkv"));
        let c = reqwest::Client::new();
        let full = c.get(&local).send().await.unwrap();
        assert_eq!(full.status().as_u16(), 200);
        assert_eq!(full.bytes().await.unwrap().to_vec(), body);
        let part = c.get(&local).header("Range", "bytes=60000-").send().await.unwrap();
        assert_eq!(part.status().as_u16(), 206);
        assert!(part.headers().get("content-range").unwrap().to_str().unwrap().starts_with("bytes 60000-"));
        assert_eq!(part.bytes().await.unwrap().to_vec(), body[60000..].to_vec());
        let missing = c.get(local.replace("/s/", "/s/x")).send().await.unwrap();
        assert_eq!(missing.status().as_u16(), 404);
    }
}
