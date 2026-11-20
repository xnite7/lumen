//! A shared byte cache for remote streams, behind the local relay.
//!
//! The player, the Atmos decoder (its own ffmpeg) and the seek-bar still
//! maker all open the same film. Without this, each one downloads it again.
//! Here the film is fetched in 512 KiB pieces into one fixed-size file on
//! disk (1 GiB, oldest pieces reused first); whoever asks for a piece that's
//! already here gets it from disk, and a piece being fetched is waited for,
//! not fetched twice. Reading ahead a few pieces keeps a playing film flowing.

use futures::StreamExt;
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::Notify;

pub const CHUNK: u64 = 512 * 1024;
const SLOTS: usize = 2048; // × 512 KiB = 1 GiB
/// pieces fetched ahead in one request when one is missing
const AHEAD: u64 = 8;

/// (which film, which piece)
type Key = (u64, u64);

struct State {
    slot_of: HashMap<Key, usize>,
    /// oldest use first
    lru: VecDeque<Key>,
    free: Vec<usize>,
    inflight: HashSet<Key>,
    /// each film's size, once known
    sizes: HashMap<u64, u64>,
}

pub struct ChunkCache {
    file: Mutex<std::fs::File>,
    st: Mutex<State>,
    arrived: Notify,
}

pub struct Target<'a> {
    pub id: u64,
    pub url: &'a str,
    pub headers: &'a [(String, String)],
}

impl ChunkCache {
    pub fn open(dir: &std::path::Path) -> std::io::Result<Arc<Self>> {
        std::fs::create_dir_all(dir)?;
        let path = dir.join("stream-cache.bin");
        // old pieces from last time aren't indexed: start empty
        let file = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(true).open(&path)?;
        Ok(Arc::new(Self {
            file: Mutex::new(file),
            st: Mutex::new(State { slot_of: HashMap::new(), lru: VecDeque::new(), free: (0..SLOTS).rev().collect(), inflight: HashSet::new(), sizes: HashMap::new() }),
            arrived: Notify::new(),
        }))
    }

    fn read_slot(&self, slot: usize, len: usize) -> std::io::Result<Vec<u8>> {
        let mut f = self.file.lock();
        f.seek(SeekFrom::Start(slot as u64 * CHUNK))?;
        let mut b = vec![0u8; len];
        f.read_exact(&mut b)?;
        Ok(b)
    }

    fn store(&self, key: Key, data: &[u8]) {
        let slot = {
            let mut st = self.st.lock();
            if let Some(s) = st.slot_of.get(&key) {
                *s
            } else {
                let s = match st.free.pop() {
                    Some(s) => s,
                    None => {
                        // reuse the oldest piece's place
                        let mut found = None;
                        while let Some(old) = st.lru.pop_front() {
                            if let Some(s) = st.slot_of.remove(&old) {
                                found = Some(s);
                                break;
                            }
                        }
                        match found {
                            Some(s) => s,
                            None => return,
                        }
                    }
                };
                s
            }
        };
        let ok = {
            let mut f = self.file.lock();
            f.seek(SeekFrom::Start(slot as u64 * CHUNK)).and_then(|_| f.write_all(data)).is_ok()
        };
        let mut st = self.st.lock();
        if ok {
            st.slot_of.insert(key, slot);
            st.lru.retain(|k| *k != key);
            st.lru.push_back(key);
        } else {
            st.free.push(slot);
        }
    }

    /// a piece, from disk (and marked as just used)
    fn get(&self, key: Key, len: usize) -> Option<Vec<u8>> {
        let slot = {
            let mut st = self.st.lock();
            let s = *st.slot_of.get(&key)?;
            st.lru.retain(|k| *k != key);
            st.lru.push_back(key);
            s
        };
        self.read_slot(slot, len).ok()
    }

    pub fn size(&self, id: u64) -> Option<u64> {
        self.st.lock().sizes.get(&id).copied()
    }

    /// The film's size (one tiny request the first time).
    pub async fn learn_size(&self, client: &reqwest::Client, t: &Target<'_>) -> anyhow::Result<(u64, Option<String>)> {
        let mut req = client.get(t.url).header("Range", "bytes=0-0");
        for (k, v) in t.headers {
            req = req.header(k.as_str(), v.as_str());
        }
        let res = req.send().await?;
        let ctype = res.headers().get("content-type").and_then(|v| v.to_str().ok()).map(String::from);
        let total = res
            .headers()
            .get("content-range")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.rsplit('/').next())
            .and_then(|v| v.trim().parse::<u64>().ok())
            .or_else(|| if res.status().as_u16() == 200 { res.content_length() } else { None })
            .ok_or_else(|| anyhow::anyhow!("no size"))?;
        self.st.lock().sizes.insert(t.id, total);
        Ok((total, ctype))
    }

    fn piece_len(total: u64, k: u64) -> usize {
        (total.saturating_sub(k * CHUNK)).min(CHUNK) as usize
    }

    /// Piece `k`, fetching it (and a few after it) if nobody has yet.
    async fn piece(&self, client: &reqwest::Client, t: &Target<'_>, total: u64, k: u64) -> anyhow::Result<Vec<u8>> {
        let len = Self::piece_len(total, k);
        loop {
            if let Some(b) = self.get((t.id, k), len) {
                return Ok(b);
            }
            let wait = self.arrived.notified();
            let mine: Vec<u64> = {
                let mut st = self.st.lock();
                if st.inflight.contains(&(t.id, k)) {
                    vec![]
                } else {
                    let last = total.div_ceil(CHUNK);
                    let mut v = vec![];
                    let mut j = k;
                    while j < last && j < k + AHEAD && !st.slot_of.contains_key(&(t.id, j)) && !st.inflight.contains(&(t.id, j)) {
                        st.inflight.insert((t.id, j));
                        v.push(j);
                        j += 1;
                    }
                    v
                }
            };
            if mine.is_empty() {
                // someone else is fetching it: wait for it to land
                wait.await;
                continue;
            }
            let r = self.fetch(client, t, total, &mine).await;
            {
                let mut st = self.st.lock();
                for j in &mine {
                    st.inflight.remove(&(t.id, *j));
                }
            }
            self.arrived.notify_waiters();
            r?;
        }
    }

    async fn fetch(&self, client: &reqwest::Client, t: &Target<'_>, total: u64, ks: &[u64]) -> anyhow::Result<()> {
        let (a, b) = (ks[0] * CHUNK, ((ks[ks.len() - 1] + 1) * CHUNK).min(total) - 1);
        let mut req = client.get(t.url).header("Range", format!("bytes={a}-{b}"));
        for (k, v) in t.headers {
            req = req.header(k.as_str(), v.as_str());
        }
        let res = req.send().await?;
        if res.status().as_u16() != 206 {
            anyhow::bail!("upstream answered {}", res.status());
        }
        let mut body = res.bytes_stream();
        let mut buf: Vec<u8> = Vec::with_capacity(CHUNK as usize);
        let mut idx = 0usize;
        while let Some(c) = body.next().await {
            let mut c = &c?[..];
            while !c.is_empty() && idx < ks.len() {
                let need = Self::piece_len(total, ks[idx]) - buf.len();
                let take = need.min(c.len());
                buf.extend_from_slice(&c[..take]);
                c = &c[take..];
                if buf.len() == Self::piece_len(total, ks[idx]) {
                    self.store((t.id, ks[idx]), &buf);
                    {
                        self.st.lock().inflight.remove(&(t.id, ks[idx]));
                    }
                    self.arrived.notify_waiters();
                    buf.clear();
                    idx += 1;
                }
            }
        }
        if idx < ks.len() {
            anyhow::bail!("upstream ended early");
        }
        Ok(())
    }

    /// Answer one request for bytes `[start, end]` (end inclusive) from pieces.
    pub async fn serve(&self, client: &reqwest::Client, t: &Target<'_>, sock: &mut TcpStream, range: Option<&str>, head: bool) -> anyhow::Result<()> {
        let (total, ctype) = match self.size(t.id) {
            Some(n) => (n, None),
            None => self.learn_size(client, t).await?,
        };
        let (start, end) = match range.and_then(|r| r.trim().strip_prefix("bytes=")) {
            Some(r) => {
                let (a, b) = r.split_once('-').unwrap_or((r, ""));
                let a: u64 = a.trim().parse().unwrap_or(0);
                let b: u64 = b.trim().parse().unwrap_or(total.saturating_sub(1)).min(total.saturating_sub(1));
                (a, b)
            }
            None => (0, total.saturating_sub(1)),
        };
        if start >= total || start > end {
            sock.write_all(format!("HTTP/1.1 416 Range Not Satisfiable\r\ncontent-range: bytes */{total}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n").as_bytes()).await?;
            return Ok(());
        }
        let status = if range.is_some() { "206 Partial Content" } else { "200 OK" };
        let mut out = format!("HTTP/1.1 {status}\r\ncontent-length: {}\r\naccept-ranges: bytes\r\n", end - start + 1);
        if range.is_some() {
            out.push_str(&format!("content-range: bytes {start}-{end}/{total}\r\n"));
        }
        out.push_str(&format!("content-type: {}\r\nconnection: close\r\n\r\n", ctype.as_deref().unwrap_or("application/octet-stream")));
        sock.write_all(out.as_bytes()).await?;
        if head {
            return Ok(());
        }
        let mut pos = start;
        while pos <= end {
            let k = pos / CHUNK;
            let data = self.piece(client, t, total, k).await?;
            let from = (pos - k * CHUNK) as usize;
            let to = ((end + 1).min((k + 1) * CHUNK) - k * CHUNK) as usize;
            // the reader hanging up (a seek) ends this request
            if sock.write_all(&data[from..to.min(data.len())]).await.is_err() {
                return Ok(());
            }
            pos = k * CHUNK + to as u64;
        }
        Ok(())
    }
}
