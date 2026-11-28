//! The addons couch ships with, forever. Nobody installs or removes
//! anything: these plus the built-in TMDB catalogs are the whole app.

/// AIOStreams: streams and subtitles for everyone. And couch's own YouTube source.
pub const LOCKED_ADDONS: &[&str] = &[
    "https://aio.atbphosting.com/stremio/3f4f0dcc-74b6-4254-9bb5-d4fe8cd20572/eyJpIjoiNXZDNlNJU1QzZnNFdXVzZUN1NTJTZz09IiwiZSI6ImdseldHQXdkU3Q2dkxTQ3VETEZndkRNcTFBM1FyZDlZeXo2ckt3VUlWUlk9IiwidCI6ImEifQ/manifest.json",
    // couch YouTube: the real episode / film for shows that are officially on
    // YouTube (asked directly, so it never depends on AIOStreams passing it on)
    "https://midnite.club/yt/manifest.json",
];
