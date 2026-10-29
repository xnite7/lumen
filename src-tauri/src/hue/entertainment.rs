//! Hue Entertainment API: a DTLS-PSK stream to the bridge on UDP 2100,
//! one HueStream v2 packet per frame. This is what makes sync fast enough
//! to feel attached to the picture (the REST API tops out around 10 cmd/s).

use super::sampler;
use crate::core::CoreRef;
use crate::error::{AppError, AppResult};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use webrtc_dtls::cipher_suite::CipherSuiteId;
use webrtc_dtls::config::Config;
use webrtc_dtls::conn::DTLSConn;
use webrtc_util::Conn;

pub struct StreamHandle {
    stop: Arc<AtomicBool>,
}

impl StreamHandle {
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

fn packet(seq: u8, area: &str, colors: &[(u8, [f32; 3])]) -> Vec<u8> {
    let mut p = Vec::with_capacity(52 + colors.len() * 7);
    p.extend_from_slice(b"HueStream");
    p.extend_from_slice(&[0x02, 0x00, seq, 0x00, 0x00, 0x00, 0x00]);
    let mut id = area.as_bytes().to_vec();
    id.resize(36, b'0');
    p.extend_from_slice(&id);
    for (ch, c) in colors.iter().take(20) {
        p.push(*ch);
        for v in c {
            let x = (v.clamp(0.0, 1.0) * 65535.0) as u16;
            p.extend_from_slice(&x.to_be_bytes());
        }
    }
    p
}

pub async fn start(core: CoreRef, ip: String, user: String, psk_hex: String, area: String, channels: Vec<(u8, f32, f32)>) -> AppResult<StreamHandle> {
    let psk = hex::decode(psk_hex.trim()).map_err(|_| AppError::msg("Bad client key"))?;
    let sock = UdpSocket::bind("0.0.0.0:0").await?;
    sock.connect(format!("{ip}:2100")).await?;
    let config = Config {
        psk: Some(Arc::new(move |_hint: &[u8]| Ok(psk.clone()))),
        psk_identity_hint: Some(user.into_bytes()),
        cipher_suites: vec![CipherSuiteId::Tls_Psk_With_Aes_128_Gcm_Sha256],
        insecure_skip_verify: true,
        ..Default::default()
    };
    let conn = tokio::time::timeout(Duration::from_secs(6), DTLSConn::new(Arc::new(sock), config, true, None))
        .await
        .map_err(|_| AppError::msg("Bridge didn't answer the DTLS handshake"))?
        .map_err(|e| AppError::msg(format!("DTLS: {e}")))?;
    let conn = Arc::new(conn);

    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    tauri::async_runtime::spawn(async move {
        let mut seq: u8 = 0;
        let mut current: Vec<[f32; 3]> = vec![[0.0; 3]; channels.len()];
        let mut sampler = sampler::Sampler::new(core.clone());
        loop {
            if stop2.load(Ordering::SeqCst) {
                break;
            }
            let (fps, bright, sat, smooth) = {
                let h = &core.settings.read().hue;
                (h.sync_fps.clamp(5, 50), h.sync_brightness / 100.0, h.sync_saturation, h.sync_smoothing.clamp(0.0, 0.95))
            };
            let t0 = Instant::now();
            let frame = sampler.grab();
            let colors: Vec<(u8, [f32; 3])> = channels
                .iter()
                .enumerate()
                .map(|(i, (ch, x, z))| {
                    let target = frame.as_ref().map(|f| f.region_color(*x, *z, sat)).unwrap_or([0.0; 3]);
                    let c = &mut current[i];
                    for k in 0..3 {
                        c[k] = c[k] * smooth + target[k] * bright * (1.0 - smooth);
                    }
                    (*ch, *c)
                })
                .collect();
            let p = packet(seq, &area, &colors);
            seq = seq.wrapping_add(1);
            if conn.send(&p).await.is_err() {
                break;
            }
            let budget = Duration::from_millis(1000 / fps as u64);
            if let Some(rest) = budget.checked_sub(t0.elapsed()) {
                tokio::time::sleep(rest).await;
            }
        }
        let _ = conn.close().await;
    });
    Ok(StreamHandle { stop })
}
