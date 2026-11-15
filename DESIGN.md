# midnite™ couch design system

The full, live version (tokens, animated component previews, the midnite rewind prototype) is the "midnite™ couch" design system artifact on claude.ai. This file is the same rules, kept with the code. Tokens live in `desktop/src/theme.rs`, motion in `desktop/src/motion.rs`.

## The idea: the living room at 2 AM

couch is a room, not a website. Every screen is the same room late at night: the lights are off, the television is the brightest thing in it, a warm lamp glows low in one corner, and a small neon sign hums on the wall. Everything in the system comes from that room.

- **The dark is the material.** 90% of any screen is the room: `room-0` (OLED black, pixels off) with plum-black surfaces lifted barely off it.
- **The lamp is warmth and "now".** 8%: `lamp` amber marks progress, ratings, what's playing, the playhead, the "on air" light.
- **The neon is precious.** 2%, once per screen: `neon` / `neon-soft` for the logo, keyboard focus, and the one thing on screen that matters. If two things are neon, neither is.
- **It is drawn by a person.** The marks are hand-drawn (the midnite "m" sticker, the couch with an M on it) and the mascot's line boils. Never redraw them clean.
- **It is a broadcast.** Big moments behave like late-night television: channel changes, an on-screen display, deadpan bumps between shows, credits, a sign-off. That is couch's own voice; it is never a stack of social-media stories.

## Voice

couch talks like a friend on the other end of the couch at 2 AM: lowercase, short, dry, warm. One joke per screen at most, and it lands as an aside, never as the headline.

- Write headings and UI in **midnite lowercase**: "your channels", "hours on the couch.", "continue watching". Films, shows, people and studios keep their own case: "most binged: Radio Silence".
- Say what happened, plainly: "added to my list", "following Greta Gerwig. their new releases show up on Home". No exclamation marks, no "Oops!", no "Awesome!".
- Deadpan beats, from the app: "that's 4.5 days. solid. very solid." · "afternoons on the couch. respectfully, what's your job." · "you didn't watch it. you moved in." · "nothing on tonight."
- Talk in the room's nouns: the couch, the lamp, the remote, tonight, on air, the crew. "finding the remote…" is a loading state.
- No em dashes in what the app says (a house rule). No emoji as decoration; emoji only where people react.
- The ™ in "midnite™ couch" is a joke about taking it seriously. Keep it on the name, nowhere else.

## Colour

Use the tokens; never type a hex into a component.

- Pages sit on `room-0`. Cards and sheets on `room-1`, raised and hovered things on `room-2`, floating things (menus, toasts) on `room-3` with the `float` shadow and an `ink-4` rim.
- Broadcast moments (midnite rewind, the intro) use `broadcast` as the screen so the lamp and neon pools read as light.
- Text is `ink` (warm white, never #fff). Secondary text `ink-2`. `ink-3` fails 4.5:1: use it only for labels repeated elsewhere ("ch 1", month initials, roles in the credits). `ink-4` is for dividers and unlit things, never text.
- The lamp: `lamp` for fills and the playhead, with `on-lamp` text on it. The lamp's light is `glow-lamp`.
- The neon: `neon` for a fill or glow, `neon-soft` for neon text, `neon-core` laid over it while the sign is lit. Its light is `glow-neon`. One neon thing per screen.
- The room takes its light from the artwork in front of you: the picture, tiny and blurred, glows down from the top of the window and cross-fades over `d-room`. With no artwork, the glow is `plum`.
- Status: `good` for success and "instant", `danger` for destructive actions; always beside a word or an icon.
- Primary buttons are `ink` with `on-ink` text: in this room the brightest pill is the one to press, not a coloured one.

## Type

One family: **Satoshi**, heavy at the top and quiet underneath. No mono, no serif, no second sans.

- Reveals and counts are Black (900): `sign` for the year, `display` for a counter, `hero` for what you are. Headings `title1` (Black) and `title2` (Bold), lowercase.
- Under a reveal: `kick` before it (the set-up, `ink-2`), `aside` after it (the deadpan line, `ink-2`).
- Running text `body` (Medium 500). Metadata `foot`. Badges `caption`.
- Broadcast chrome (`osd`) is Black at 16px: "play  mar 14", "ch 03 channels".
- Numbers in counters roll on wheels and keep their leading zeros at `ink-4`, like a VCR's tape counter.

## Motion

Motion is how the room feels alive. There are four kinds, and nothing else (the full rules are in the Motion section):

1. **Lamp**: arrivals warm up. Fade up 14px over `d-lamp` on `ease-lamp`. No bounce, no blur, no swoosh.
2. **Spring**: only what you touched answers with weight (`spring-bouncy`): a pressed pill, a toast, a poster dropped on the pile.
3. **Channel**: a change of context dips through dark (`d-channel`): the picture folds to a bright line and the next opens out of it. Tabs, rewind channels, opening the player.
4. **Neon**: the sign switching on (`d-neon`): dark, a flash, dark, nearly, a dip, steady. Once per screen.

Under all of it, the room's texture never stops: film grain at 24 fps, the mascot's line boil, scanlines in broadcast moments, the night warming the later it gets. Respect reduced motion: lamp and channel become plain fades, neon is simply on, grain stops.

## Texture and light

- **Grain** over everything at about 3% (heavier, 18%, in broadcast moments), changing 24 times a second like film.
- **Pools of light**, never gradients-as-decoration: the lamp low in a corner, the neon's spill high in the opposite one, soft discs of light, not shapes.
- **Scanlines** only in broadcast moments: a dark line every 4px, drifting down.
- **The night**: after midnight the room gets warmer and dimmer (up to 16% darker, a lamp-coloured warmth from the bottom). Applied to the whole window, never per component.

## Shape and layout

- Everything you press is a pill (`r-pill`). Posters `r-sm`, wide cards `r-card`, sheets `r-xl`.
- Spacing on a 4px grid: `s1`…`s8`, rows `s8` apart, page gutter `gutter` (56px wide, 16px on a phone).
- No borders as decoration. A floating surface gets one hairline rim (`ink-4` at 7%) and the `float` shadow; everything else is separated by light and space.
- Shadows are light and weight: `lift` under text on pictures, `drop` under a lifted poster, `float` under floating surfaces, the two glows for the lamp and neon.
- Pictures are the content: posters 2:3, backdrops 16:9, the title's logo over its backdrop, melting into the room at the bottom (no hard edge).

## Iconography

- Material Symbols Rounded as SVG, filled when selected (`home` / `home-fill`), drawn in `ink` at 18–24px.
- couch's own marks (the couch, the midnite "m") are drawn by hand and are never redrawn as icons.
- A watched tick is a black disc with a white check. A failed picture is couch's own placeholder (`plum` glow and the couch mark), never a broken-image icon.

## Assets

- **Logos**: `midnite-logo.svg` (the pink "m" sticker), `midnite-logo-lit.svg` (switched on, with sparkles: launch and sign-in only), `couch-logo.svg` and `couch-mark.png` (the couch with an M), `icon-1024.png` (the app icon, black ink on white).
- **Mascot**: `couch-wobble.webp` (idle boil: loading, empty states, the station bug) and `couch-boil.webp` (a bigger boil for hero moments).
- **Party**: `postmark.png`, `seal-l.png`, `seal-r.png` from the watch-party invite envelope: the analog, tactile side of the room.

## Components

Each has a live preview and guidelines: **Neon** (the sign switching on), **ChannelChange**, **Room** (lamp, neon spill, grain, night), **TapeCounter**, **BroadcastChrome** (on-screen display and the tape), **Toast**, **Pill** (buttons and chips), **PosterCard**, and **Rewind**, the whole of midnite rewind (couch's Wrapped) running in the room.

## In the code

The GPUI app reads these from `desktop/src/theme.rs` (colours, `ty::*`, `radius::*`, `space::*`, `glow_neon`, `glow_lamp`) and `desktop/src/motion.rs` (`dur::*`, `lamp`, `out3`, `bouncy`, `channel`, `neon`, `grain`, `scanlines`, `pool`). Never write an easing curve or a hex value inline in a view.


## Motion in detail

Animation is half of couch's identity: a still screenshot of the room is just a dark page. These are the rules every moving thing follows. The values live in `motion.rs` and the `duration` and `easing` tokens.

### The four motions

| motion | what it means | when | how |
|---|---|---|---|
| **lamp** | light warming up | anything arriving: a page, a line of text, a row, a sheet | opacity 0 → 1 and 14px up → 0, `d-lamp`, `ease-lamp`. Page turns: 22px, `d-turn`, `ease-out` (14px from above when going back). |
| **spring** | weight answering a touch | only what the person touched: a pressed pill, a toast, a poster dropped, a card growing after you rest on it | `spring-bouncy` (260 / 22). A press squashes to 0.96 in `d-tap` and springs back. |
| **channel** | a change of context | switching tabs, rewind's channels, opening the player | `d-channel`: black shutters close from top and bottom over the first 40% (`ease-in`), a bright `ink` line glows across the middle, swap at the dark moment with a burst of snow, open over the last 50% (`ease-out`). |
| **neon** | the sign switching on | once per screen, for the one thing that matters | `d-neon`: off to 8%, a flash (1.15) to 11%, dark to 18%, nearly (0.85) to 22%, dark to 30%, bright to 36%, a dip (0.55) to 42%, then steady with a slow hum (±6% over 3 s, a brief buzz now and then). |

Nothing else moves. No swooshes, no slide-ins from the side, no scale-pops on arrival, no blur-ins, no confetti, no parallax for its own sake.

### Rhythm: bumps

Big moments arrive in **beats**, like a late-night bump between shows: the set-up line (`kick`), one beat (`d-beat`, 550ms), the reveal, one beat, the deadpan aside. Lines land one at a time and nothing lands together. Leave the screen alone once it has said its thing: no idle wiggles except the room's own texture.

### Counting

Numbers count on a **tape counter**: each digit is a wheel; the ones roll continuously, the tens only turn as the ones pass 9→0, leading zeros stay at `ink-4`. Counts take about 2.4 s on `ease-lamp` and the lamp in the room warms with them.

### Texture that never stops

- **Grain**: a scatter of light and dark specks, a new scatter 24 times a second. About 3% over the app, 18% in broadcast moments, up to snow during a channel change.
- **Boil**: the mascot redraws at 12 fps (couch-wobble.webp, couch-boil.webp).
- **Scanlines**: broadcast moments only; a dark line every 4px drifting down at 6 px/s.
- **The room's light**: cross-fades over `d-room` when the artwork changes; the night warms over hours.

### Leaving

Things leave faster than they arrive and accelerate away (`ease-in`): a toast sinks 14px and fades in 350ms; a menu fades in `d-quick`. Switching rewind off folds the picture to a line, the line to a dot, then dark (700ms), the way a CRT turns off.

### Reduced motion

With reduced motion: lamp and channel become a plain `d-quick` fade, springs become `ease-out`, neon is simply on, grain and scanlines stop, counters show their final number. The beats stay (they are timing, not motion).


## midnite rewind

couch's year in review. It is **not** a stack of stories (no progress bars across the top, no tap-through cards in rainbow gradients, no left-aligned stat slabs): it is the year played back as a late-night broadcast on couch's own channel, in the same dark room as the rest of the app.

### The set

- **The screen** is `broadcast`, with the room's two lights: the lamp low on the left (`lamp` pool), the neon's spill high on the right (`neon` pool). Each channel sets how bright each is; they ease between channels and never change hue. No other colours appear, except a film's own picture.
- **On-screen display**, top left: a dot and the deck's state, `rew` while the tape rewinds (blinking lamp dot), `play` after (lit neon dot), then the date on the tape ("mar 14"). Top right: `ch 03` and the channel's name.
- **The tape**, along the bottom: the whole year from jan to dec, a tick per channel, the lamp playhead riding it. It replaces story progress bars.
- **The station bug**: the boiling couch at 32% in the bottom-right corner.
- **Texture**: scanlines at 16%, grain at 18%, snow on every channel change.

### The programme

| ch | name | what airs | the moment |
|---|---|---|---|
| 01 | cold open | "midnite™ couch presents" · the year · "your year on the couch, rewound." | the tape rewinds dec 31 → jan 01 on the display; the year is a neon sign switching on |
| 02 | time | "you spent about" · tape counter · "hours on the couch." · "that's 4.5 days. solid. very solid." | the counter rolls; the lamp warms with it |
| 03 | lineup | "tonight's lineup: your year" · films / shows / episodes as a TV listing with dot leaders · "it started with … and (so far) ends with …" | rows land a beat apart, numbers count |
| 04 | channels | your genres as channels: "ch 1 drama" (hero size, an "on air" lamp light), the rest smaller | a beat per channel |
| 05 | signal | "your busiest month" · the month · a level meter of 12 columns × 12 segments | segments climb bottom-up; the busiest month is the screen's one neon thing |
| 06 | clock | "you are" · "a midnite creature" · "most watched at" an unset VCR clock blinking the peak hour · the four day-parts | for night owls the title is the neon sign; otherwise the lamp is bright |
| 07 | now playing | the most-binged show's backdrop fills the screen at 34%, slowly settling · its name · episode counter in `lamp` | |
| 08 | rewatch | "the ones you'd watch again": the 9s and 10s dropped onto a pile | each poster drops with `spring-bouncy`, a `lamp` score badge |
| 09 | credits | end credits rolling up: role on the left in `ink-3`, name on the right in `ink` ("starring · you", "hours on the couch · 107", … "filmed on location · your couch"), then the mascot and "that's the broadcast. midnite™ couch is going off the air. back on in 2027." | rolls at reading pace and stops on the sign-off; a click switches the set off |

Channels with nothing to say are skipped (no genres, no binge, no 9s). An empty year is one channel: "nothing on tonight."

### Moving through it

- Each channel stays on for its own length (5–7 s), then changes by itself; the credits stay.
- Click the right 70% (or →, space) for the next channel, the left 30% (or ←) for the one before. Esc or the close button switches the set off.
- Every change is the **channel** motion; opening is the second half of it (the picture opens out of a line); closing folds to a line, then a dot, then dark.
- Lines land on beats (`d-beat`), arriving with the **lamp** motion.

### Copy

Lowercase, deadpan, the asides from the app: "solid. very solid.", "the cushions have your shape now.", "respectfully, what's your job.", "you didn't watch it. you moved in.", "one more episode. several times."
