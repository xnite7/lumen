# Lumen

Your Stremio, rebuilt as its own app. Every Stremio addon works; everything
your 20 plugins did is now built in, natively, with nothing patched on top.

- **Tauri 2 + Rust** backend, **Svelte 5** UI (≈75 KB of JS gzipped)
- **mpv** draws video straight into the window *underneath* the UI —
  every codec, HDR passthrough, GPU scaling, no `<video>` element
- **Torrents** stream natively (librqbit) when an addon gives an info hash
- Liquid-glass interface, real spring physics, View Transitions

## Where each plugin went

| Old plugin / worker | In Lumen |
|---|---|
| Addon Manager | Addons page: drag to reorder, configure, export/import, undo remove |
| Addon Health | "Check health" on the Addons page (ok / slow / not answering) |
| AI Vibe Search | Mood bar under the hero. Calls Anthropic directly — no worker, no shared secret |
| Cast Page | Person page (TMDB; adult performers via any adult addon's `/performer` route) |
| Clip Previews | Hover/dwell trailer cards, hero trailers after 3 s, mute toggle |
| Context Menu Fix | Not needed (own UI) |
| Data Enrichment | Detail page: cast, collection, more like this, trailers & extras, About |
| Enhanced Covers | Continue Watching in 16:9 with Movie of the Night art, backdrop+logo fallback |
| Enhanced Player | Glass subtitles, SDH styling/hiding, Arabic RTL fix, cue animation, buffering art |
| Enhanced Title Bar | Floating glass bar with real window controls |
| Glass Polish | The whole design system: ambient light, grain, springs, rail arrows, Top 10 numerals, image proxy fallback, adult toggle |
| Dynamic Hero | Rotating hero from Simkl trending (TMDB if Simkl isn't connected) |
| Horizontal Navigation | Native tab bar (bottom on iPhone) |
| Initializer | Not needed |
| Picture in Picture | Native PiP: small always-on-top window, mpv keeps playing |
| Search Grid + Search Load All | Search page: every addon asked in full, one grid, suggestions, recents |
| Simkl Sync | Scrobbling, ratings (stars on detail), watched sync, library ↔ Plan to Watch |
| Stream Quality Picker | Streams sheet: grouped by resolution, Instant/HDR/codec badges, Best 4K/1080p |
| TheIntroDB | Skip intro/recap/credits buttons + optional auto-skip, segments on the seek bar |
| Simkl worker rows | Built-in "Lumen" addon: Simkl lists, Because You Watched, Surprise Me, Top 10, TMDB rows |
| Image proxy worker | Still used as the fallback for blocked images — after encrypted DNS, which fixes most blocks on its own |

## New

- **Philips Hue** — dim on play, or *match the picture* (Entertainment API,
  ~25 fps, colours sampled from the video), restore afterwards, and the room
  can follow the artwork you're browsing.
- **Tobii** — look away to pause / look back to resume, controls appear when
  you look at the bottom, dwell on a poster to preview it, and the backdrop
  moves with your head like a window.
- **Next episode, same source** — binge without the stream list.
- **Paste anything** — a magnet, a video link or an addon link, anywhere.
- **Stremio account sync** — same addons and Continue Watching as your phone.
- **Themes** — JSON + optional CSS in the themes folder; includes *Cozy Innocent
  Death* (your palette), Midnight, Broadcast, Daylight.

## Build

Push to GitHub and the workflow builds the Windows installer
(`.github/workflows/build.yml`). Locally on Windows:

```powershell
# once: Node 22, Rust (rustup), Visual Studio Build Tools (C++), 7-Zip
npm install
./scripts/fetch-mpv.ps1      # from "Developer PowerShell for VS"
npx tauri build              # -> src-tauri/target/release/bundle/nsis/*.exe
```

Dev mode with hot reload: `npx tauri dev`.

## Notes

- Lumen registers `stremio://`, so Install buttons on addon sites open here.
- If video ever doesn't show (unusual GPU drivers), turn off
  Settings → Playback → *Video inside Lumen* and it opens in its own window.
- Eye tracking loads `tobii_stream_engine.dll` from your Tobii software at
  runtime. If it isn't found, point Settings → Eye tracking at it.
- iPhone: the workflow also produces an unsigned `.ipa` for AltStore /
  Sideloadly (experimental; MKV/HEVC hands off to VLC or Infuse).
