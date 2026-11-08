//! Every user-facing preference, in one serde struct.
//!
//! `#[serde(default)]` on every struct means a settings blob written by an
//! older build still loads: new fields just take their defaults. Nothing here
//! is secret -- keys and tokens live in `Db::secret`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub general: General,
    pub playback: Playback,
    pub subtitles: Subtitles,
    pub streams: Streams,
    pub network: Network,
    pub integrations: Integrations,
    pub hue: Hue,
    pub tobii: Tobii,
    pub home: Home,
}

impl Settings {
    /// Things couch just does now -- no switches for them. Applied on every
    /// load and save, so old settings files can't turn them back off.
    pub fn pin(&mut self) {
        let g = &mut self.general;
        g.sound = true;
        g.neon = "lit".into();
        g.time_aware = true;
        g.accent_from_artwork = true;
        g.ambient_glow = true;
        g.grain = true;
        g.refraction = true;
        g.motion_scale = 1.3; // floaty
        g.hover_previews = true;
        g.hover_delay_ms = 600; // a beat longer before a preview opens
        g.preview_muted = true;
        g.hero_trailers = true;
        g.hero_muted = true;
        g.show_titles_under_posters = false;
        g.top_ten_ranks = true;
        g.hide_adult = true; // adult content lives in midnite afterdark
        // Simkl: what you watch and your list always sync (no switches)
        self.integrations.simkl_scrobble = true;
        self.integrations.simkl_sync_watchlist = true;
        let p = &mut self.playback;
        p.show_skip_buttons = true;
        p.auto_skip_intro = true;
        p.auto_skip_recap = true;
        p.auto_skip_credits = true;
        p.next_card_secs = 35;
        p.binge_same_source = true;
        p.watched_threshold = 84;
        p.native_ass = true;
        let s = &mut self.subtitles;
        s.style_sdh = true;
        s.rtl_fix = true;
        let i = &mut self.integrations;
        i.builtin_catalogs = true;
        i.kinocheck_trailers = true;
        i.tidb_enabled = true;
        i.ai_enabled = true;
        i.crew_share = true;
        self.home.hidden_rows.clear();
        self.home.row_order.clear();
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            general: General::default(),
            playback: Playback::default(),
            subtitles: Subtitles::default(),
            streams: Streams::default(),
            network: Network::default(),
            integrations: Integrations::default(),
            hue: Hue::default(),
            tobii: Tobii::default(),
            home: Home::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct General {
    pub theme: String,
    /// "auto" | "dark" | "light"
    pub appearance: String,
    /// 0 = no motion, 1 = normal, up to 1.5 for extra-floaty.
    pub motion_scale: f32,
    pub accent_from_artwork: bool,
    pub grain: bool,
    pub ambient_glow: bool,
    pub refraction: bool,
    /// Hide adult (`behaviorHints.adult` / xxx) items behind a toggle.
    pub hide_adult: bool,
    pub ui_language: String,
    pub hover_previews: bool,
    pub hover_delay_ms: u32,
    pub hero_trailers: bool,
    pub hero_muted: bool,
    pub preview_muted: bool,
    pub show_titles_under_posters: bool,
    pub top_ten_ranks: bool,
    /// First-run setup finished.
    pub onboarded: bool,
    /// How hard the neon glows: "soft" | "lit" | "loud".
    pub neon: String,
    /// Tiny clicks and a CRT hum on launch.
    pub sound: bool,
    /// After midnight the room gets warmer and dimmer.
    pub time_aware: bool,
    /// Experimental features (eye tracking, Philips Hue) switched on.
    pub experimental: bool,
    /// Install new versions of couch by itself.
    pub auto_update: bool,
}

impl Default for General {
    fn default() -> Self {
        Self {
            theme: "liquid-glass".into(),
            appearance: "dark".into(),
            motion_scale: 1.0,
            accent_from_artwork: true,
            grain: true,
            ambient_glow: true,
            refraction: false,
            hide_adult: true,
            ui_language: "en".into(),
            hover_previews: true,
            hover_delay_ms: 450,
            hero_trailers: true,
            hero_muted: false,
            preview_muted: true,
            show_titles_under_posters: false,
            top_ten_ranks: true,
            onboarded: false,
            neon: "lit".into(),
            sound: false,
            time_aware: true,
            experimental: false,
            auto_update: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Playback {
    pub autoplay_next: bool,
    /// Minimal player: the progress bar sits inside the controls bar.
    pub minimal_player: bool,
    /// Spoiler-free: blur pictures and stories of episodes you haven't seen.
    pub blur_unwatched: bool,
    /// Seconds before the end at which the "Up next" card appears when no
    /// credits segment is known.
    pub next_card_secs: u32,
    pub binge_same_source: bool,
    pub auto_skip_intro: bool,
    pub auto_skip_recap: bool,
    pub auto_skip_credits: bool,
    pub show_skip_buttons: bool,
    pub audio_languages: Vec<String>,
    pub resume_prompt: bool,
    /// "auto-safe" | "auto" | "no" | "d3d11va" | "nvdec" ...
    pub hwdec: String,
    /// "gpu-next" | "gpu"
    pub vo: String,
    pub hdr_passthrough: bool,
    pub deband: bool,
    pub interpolation: bool,
    /// "fit" | "fill" | "stretch"
    pub aspect: String,
    pub seek_small: u32,
    pub seek_large: u32,
    pub volume: f64,
    pub volume_boost: bool,
    pub cache_mb: u32,
    pub readahead_secs: u32,
    /// Native mpv render for subtitles with heavy ASS typesetting.
    pub native_ass: bool,
    pub watched_threshold: u32,
    /// Draw video inside couch's window (under the UI). Off = mpv opens its
    /// own window, as a fallback for unusual GPU/driver setups.
    pub embed_video: bool,
    /// iPhone: "auto" (Apple's player for MP4/HLS, couch's for the rest) |
    /// "couch" (always couch's own mpv player).
    pub ios_player: String,
}

impl Default for Playback {
    fn default() -> Self {
        Self {
            autoplay_next: true,
            minimal_player: false,
            blur_unwatched: false,
            next_card_secs: 35,
            binge_same_source: true,
            auto_skip_intro: false,
            auto_skip_recap: false,
            auto_skip_credits: false,
            show_skip_buttons: true,
            audio_languages: vec!["eng".into(), "jpn".into()],
            resume_prompt: false,
            hwdec: "auto-safe".into(),
            vo: "gpu-next".into(),
            hdr_passthrough: true,
            deband: true,
            interpolation: false,
            aspect: "fit".into(),
            seek_small: 10,
            seek_large: 60,
            volume: 100.0,
            volume_boost: false,
            cache_mb: 400,
            readahead_secs: 120,
            native_ass: false,
            watched_threshold: 84,
            embed_video: true,
            ios_player: "auto".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Subtitles {
    pub languages: Vec<String>,
    pub enabled_by_default: bool,
    pub font: String,
    pub size: f32,
    pub weight: u32,
    pub color: String,
    /// "glass" | "box" | "outline" | "shadow"
    pub background: String,
    pub bg_opacity: f32,
    pub blur: f32,
    pub bottom_offset: f32,
    pub animate_cues: bool,
    pub style_sdh: bool,
    pub hide_sdh: bool,
    pub rtl_fix: bool,
}

impl Default for Subtitles {
    fn default() -> Self {
        Self {
            languages: vec!["eng".into(), "ara".into()],
            enabled_by_default: true,
            font: "system-ui".into(),
            size: 1.0,
            weight: 600,
            color: "#ffffff".into(),
            background: "glass".into(),
            bg_opacity: 0.35,
            blur: 14.0,
            bottom_offset: 7.0,
            animate_cues: true,
            style_sdh: true,
            hide_sdh: false,
            rtl_fix: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Streams {
    /// "smart" | "quality" | "size" | "seeders" | "addon"
    pub sort: String,
    pub hide_cam: bool,
    pub hide_3d: bool,
    pub max_size_gb: f32,
    pub prefer_hdr: bool,
    pub prefer_cached: bool,
    /// Skip the picker and play the best match immediately.
    pub autoplay_best: bool,
    pub group_by_quality: bool,
}

impl Default for Streams {
    fn default() -> Self {
        Self {
            sort: "smart".into(),
            hide_cam: true,
            hide_3d: false,
            max_size_gb: 0.0,
            prefer_hdr: true,
            prefer_cached: true,
            autoplay_best: true,
            group_by_quality: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Network {
    /// Resolve names over HTTPS. Most country/ISP blocks are DNS-level, so
    /// this alone brings back the image hosts and APIs they filter.
    pub doh: bool,
    /// "cloudflare" | "google" | "quad9"
    pub doh_provider: String,
    /// Retry failed images through this proxy (`...?url=` style prefix).
    pub image_proxy: String,
    pub proxy_failed_images: bool,
    pub addon_timeout_ms: u64,
    pub addon_concurrency: usize,
    pub torrent_cache_gb: f32,
    pub torrent_max_peers: usize,
}

impl Default for Network {
    fn default() -> Self {
        Self {
            doh: true,
            doh_provider: "cloudflare".into(),
            image_proxy: "https://stremio-image-proxy.xnite7.workers.dev/?url=".into(),
            proxy_failed_images: true,
            addon_timeout_ms: 9000,
            addon_concurrency: 6,
            torrent_cache_gb: 8.0,
            torrent_max_peers: 120,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Integrations {
    pub simkl_enabled: bool,
    pub simkl_scrobble: bool,
    pub simkl_sync_watchlist: bool,
    pub simkl_mirror_watched: bool,
    /// Log watching something again as a separate viewing (Simkl PRO/VIP).
    pub simkl_rewatches: bool,
    pub tidb_enabled: bool,
    pub ai_enabled: bool,
    pub ai_model: String,
    pub ai_fast: bool,
    pub motn_country: String,
    pub motn_service: String,
    /// Built-in couch catalogs (Simkl lists, TMDB rows, Top 10).
    pub builtin_catalogs: bool,
    pub stremio_sync: bool,
    pub kinocheck_trailers: bool,
    /// Discord "Watching ..." -- only for titles Simkl knows.
    pub discord_presence: bool,
    /// Application ID from discord.com/developers (its name is what Discord shows).
    pub discord_app_id: String,
    /// The midnite crew server (a tiny Cloudflare Worker). Empty = off.
    pub crew_url: String,
    /// Name friends see on the "On the couch" row.
    pub crew_name: String,
    /// Share what you're watching with the crew (Simkl-known titles only).
    pub crew_share: bool,
    /// Gatekeeper mode: friends see you on the couch, not what you watch.
    #[serde(default)]
    pub crew_gatekeeper: bool,
}

impl Default for Integrations {
    fn default() -> Self {
        Self {
            simkl_enabled: true,
            simkl_scrobble: true,
            simkl_sync_watchlist: true,
            simkl_mirror_watched: true,
            simkl_rewatches: false,
            tidb_enabled: true,
            ai_enabled: true,
            ai_model: "claude-opus-5".into(),
            ai_fast: true,
            motn_country: "ae".into(),
            motn_service: "netflix".into(),
            builtin_catalogs: true,
            stremio_sync: true,
            kinocheck_trailers: true,
            discord_presence: true,
            discord_app_id: "1568062298721886290".into(),
            crew_url: String::new(),
            crew_name: String::new(),
            crew_share: true,
            crew_gatekeeper: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Hue {
    pub enabled: bool,
    pub bridge_ip: String,
    pub bridge_id: String,
    /// Entertainment configuration (area) used for live sync.
    pub area_id: String,
    /// Room/zone (grouped_light) used for dimming and artwork moods.
    pub room_id: String,
    /// "off" | "dim" | "ambilight"
    pub playback_mode: String,
    pub dim_brightness: f32,
    pub sync_brightness: f32,
    pub sync_saturation: f32,
    /// 0 = instant, 1 = very smooth.
    pub sync_smoothing: f32,
    pub sync_fps: u32,
    /// Restore the lights' previous scene when playback stops.
    pub restore_after: bool,
    /// Tint the room from the artwork of whatever you're browsing.
    pub artwork_mood: bool,
    pub mood_brightness: f32,
}

impl Default for Hue {
    fn default() -> Self {
        Self {
            enabled: false,
            bridge_ip: String::new(),
            bridge_id: String::new(),
            area_id: String::new(),
            room_id: String::new(),
            playback_mode: "ambilight".into(),
            dim_brightness: 8.0,
            sync_brightness: 70.0,
            sync_saturation: 1.25,
            sync_smoothing: 0.55,
            sync_fps: 25,
            restore_after: true,
            artwork_mood: false,
            mood_brightness: 35.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Tobii {
    pub enabled: bool,
    pub dll_path: String,
    /// Pause when you look away, resume when you come back.
    pub presence_pause: bool,
    pub presence_grace_ms: u32,
    /// Look at the bottom-left corner in the player: dwell bubbles appear.
    pub gaze_controls: bool,
    /// How long to rest your eyes on a bubble.
    pub dwell_ms: u32,
    /// Hero artwork shifts with your head like a window.
    pub head_parallax: bool,
    /// 3D films: the picture follows your head (look around the scene).
    pub depth3d: bool,
    pub parallax_strength: f32,
    /// Dim the Hue lights while you're away.
    pub away_dims_lights: bool,
}

impl Default for Tobii {
    fn default() -> Self {
        Self {
            enabled: false,
            dll_path: String::new(),
            presence_pause: true,
            presence_grace_ms: 2500,
            gaze_controls: true,
            dwell_ms: 1500,
            head_parallax: true,
            depth3d: true,
            parallax_strength: 1.0,
            away_dims_lights: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct Home {
    /// Catalog keys (`addonId/type/catalogId`) the user hid from Home.
    pub hidden_rows: Vec<String>,
    /// Explicit row order; rows not listed follow in addon order.
    pub row_order: Vec<String>,
    /// Hero source: "simkl-trending" | "tmdb-trending" | catalog key
    pub hero_source: String,
}
