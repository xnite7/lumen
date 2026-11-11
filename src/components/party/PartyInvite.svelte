<script lang="ts">
  // A watch party invite, the paper way: an envelope slides onto the screen,
  // the wax seal cracks, the flap lifts and a ticket rises out with the film,
  // the time and who's hosting. "I'm in" stamps it ADMITTED; "can't make it"
  // tucks it back in and the envelope slides off.
  import { img } from "../../lib/api";
  import { parties, rsvp, later } from "../../lib/stores/parties.svelte";
  import { social } from "../../lib/stores/social.svelte";
  import { haptic } from "../../lib/native";
  import mark from "../../assets/brand/couch-mark.png";

  const p = $derived(parties.invites[0] ?? null);
  // "in" -> stamped then gone; "out" -> back into the envelope then gone
  let leaving = $state<"" | "in" | "out" | "later">("");
  let shownFor = "";
  $effect(() => {
    if (p && p.id !== shownFor) {
      shownFor = p.id;
      leaving = "";
      haptic("success");
    }
  });

  function answer(yes: boolean) {
    if (!p || leaving) return;
    const party = p;
    leaving = yes ? "in" : "out";
    haptic(yes ? "success" : "light");
    setTimeout(() => rsvp(party, yes), yes ? 1700 : 1300);
  }
  function close() {
    if (!p || leaving) return;
    const party = p;
    leaving = "later";
    setTimeout(() => later(party), 700);
  }

  const when = $derived(p ? new Date(p.at) : null);
  const day = $derived(when ? when.toLocaleDateString(undefined, { weekday: "short" }).toUpperCase() : "");
  const date = $derived(when ? when.toLocaleDateString(undefined, { day: "numeric", month: "short" }).toUpperCase() : "");
  const hour = $derived(when ? when.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" }).toUpperCase() : "");
  // seat and ticket number, the same every time for this party
  const hash = (s: string) => [...s].reduce((h, c) => (h * 31 + c.charCodeAt(0)) >>> 0, 7);
  const seat = $derived(p ? `${String.fromCharCode(65 + (hash(p.id) % 13))}${(hash(p.id + parties.me) % 18) + 1}` : "");
  const number = $derived(p ? String(hash(p.id + "n") % 1_000_000).padStart(6, "0") : "");
  const bars = $derived(p ? [...String(hash(p.id + "bars")).padEnd(24, "3")].map((d) => 1 + (Number(d) % 3)) : []);
  const me = $derived(social.me?.name ?? "you");
  const going = $derived(p ? Object.values(p.rsvp).filter((r) => r.yes) : []);
</script>

{#if p}
  {#key p.id}
    <div class="invite" class:leaving={!!leaving} class:l-in={leaving === "in"} class:l-out={leaving === "out"} class:l-later={leaving === "later"} role="dialog" aria-label="Watch party invite from {p.hostName}">
      <div class="dim" role="presentation" onclick={close}></div>

      <div class="stage">
        <!-- the envelope: back, ticket inside, front pocket, flap, seal -->
        <div class="env">
          <div class="env-back"></div>

          <!-- the ticket -->
          <div class="ticket" aria-live="polite">
            <div class="t-main">
              <div class="t-art">
                {#if p.title.poster}<img src={img(p.title.poster, "w342")} alt="" />{/if}
              </div>
              <div class="t-info">
                <div class="t-kicker"><span>midnite™ couch</span><span>presents</span></div>
                <h2 class="t-title">{p.title.name}</h2>
                <div class="t-meta">{[p.title.year, p.title.type === "series" ? "series" : "feature", p.title.runtime].filter(Boolean).join(" · ")}</div>
                <div class="t-grid">
                  <div><span>date</span><b>{day} {date}</b></div>
                  <div><span>doors</span><b>{hour}</b></div>
                  <div><span>seat</span><b>COUCH {seat}</b></div>
                </div>
                <div class="t-host">
                  <span class="t-av">{#if p.hostAvatar}<img src={p.hostAvatar} alt="" />{:else}{p.hostName.charAt(0)}{/if}</span>
                  <span>hosted by <b>{p.hostName}</b>{#if going.length > 1} · {going.length - 1} more going{/if}</span>
                </div>
                {#if p.note}<p class="t-note">“{p.note}”</p>{/if}
              </div>
            </div>
            <div class="t-stub">
              <span class="admit">ADMIT ONE</span>
              <span class="bars" aria-hidden="true">{#each bars as w}<i style:width="{w}px"></i>{/each}</span>
              <span class="no">№ {number}</span>
            </div>
            <div class="stamp" aria-hidden="true">ADMITTED</div>
          </div>

          <div class="env-front">
            <span class="to">to {me}</span>
            <span class="from">from {p.hostName}</span>
            <div class="postage" aria-hidden="true">
              <div class="postage-in">
                {#if p.title.poster}<img src={img(p.title.poster, "w342")} alt="" />{/if}
                <span>couch post</span>
              </div>
            </div>
            <svg class="postmark" viewBox="0 0 120 70" aria-hidden="true">
              <circle cx="35" cy="35" r="27" />
              <circle cx="35" cy="35" r="21" />
              <text x="35" y="31" text-anchor="middle">{day}</text>
              <text x="35" y="44" text-anchor="middle">{date}</text>
              <path d="M66 22 q 9 -6 18 0 t 18 0 t 18 0 M66 35 q 9 -6 18 0 t 18 0 t 18 0 M66 48 q 9 -6 18 0 t 18 0 t 18 0" />
            </svg>
          </div>
          <div class="flap"><div class="flap-paper"></div></div>
          <div class="seal" aria-hidden="true">
            <div class="seal-l"><img src={mark} alt="" /></div>
            <div class="seal-r"><img src={mark} alt="" /></div>
          </div>
        </div>

        <div class="answers">
          <button class="yes press" onclick={() => answer(true)}>I'm in</button>
          <button class="no press" onclick={() => answer(false)}>can't make it</button>
          <button class="later press" onclick={close}>decide later</button>
        </div>
      </div>
    </div>
  {/key}
{/if}

<style>
  .invite {
    position: fixed;
    inset: 0;
    z-index: 700;
    display: grid;
    place-items: center;
    --paper: #efe4cf;
    --paper-2: #e3d4b8;
    --ink: #3b1029;
    --wax: #c4206f;
    --cream: #f7eedd;
    --motion: 1;
  }
  .dim {
    position: absolute;
    inset: 0;
    background:
      radial-gradient(60% 60% at 50% 45%, rgb(255 178 92 / 0.12), transparent 70%),
      rgb(6 3 8 / 0.72);
    backdrop-filter: blur(8px);
    animation: fade 500ms ease both;
  }
  .stage {
    position: relative;
    display: grid;
    justify-items: center;
    gap: 26px;
    width: min(620px, calc(100vw - 32px));
  }

  /* ------------------------------------------------------------ envelope */
  .env {
    position: relative;
    width: 100%;
    aspect-ratio: 1.6;
    perspective: 1400px;
    animation: env-in 1000ms cubic-bezier(0.2, 1.2, 0.3, 1) both;
  }
  @keyframes env-in {
    0% {
      transform: translate(120vw, -10vh) rotate(24deg);
    }
    70% {
      transform: translate(-2%, 0) rotate(-4deg);
    }
    100% {
      transform: translate(0, 0) rotate(-2deg);
    }
  }
  /* paper: warm stock, a little fibre, a little grain */
  .env-back,
  .env-front,
  .flap-paper {
    background:
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='220' height='220'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='.75' numOctaves='3' stitchTiles='stitch'/><feColorMatrix values='0 0 0 0 .35  0 0 0 0 .25  0 0 0 0 .15  0 0 0 .22 0'/></filter><rect width='100%' height='100%' filter='url(%23n)'/></svg>"),
      repeating-linear-gradient(97deg, rgb(120 90 60 / 0.035) 0 1px, transparent 1px 7px),
      linear-gradient(160deg, var(--paper), var(--paper-2));
  }
  .env-back {
    position: absolute;
    inset: 0;
    border-radius: 10px;
    box-shadow:
      0 40px 80px -30px rgb(0 0 0 / 0.85),
      0 12px 24px -12px rgb(0 0 0 / 0.6),
      inset 0 0 0 1px rgb(80 50 20 / 0.12);
  }
  /* the front pocket: two side folds and the bottom fold, with soft creases */
  .env-front {
    position: absolute;
    inset: 0;
    z-index: 3;
    border-radius: 10px;
    clip-path: polygon(0 0, 50% 52%, 100% 0, 100% 100%, 0 100%);
    filter: drop-shadow(0 -3px 4px rgb(60 30 10 / 0.18));
  }
  .env-front::before {
    content: "";
    position: absolute;
    inset: 0;
    background:
      linear-gradient(32deg, transparent 49.6%, rgb(90 60 30 / 0.16) 50%, transparent 50.8%),
      linear-gradient(-32deg, transparent 49.6%, rgb(90 60 30 / 0.16) 50%, transparent 50.8%),
      linear-gradient(180deg, transparent 40%, rgb(80 50 20 / 0.08));
  }
  .to,
  .from {
    position: absolute;
    font-family: "Segoe Print", "Bradley Hand", "Chalkboard SE", "Comic Sans MS", cursive;
    color: #2a2a55;
    opacity: 0.85;
  }
  .to {
    left: 9%;
    bottom: 22%;
    font-size: clamp(18px, 3.4vw, 26px);
    rotate: -3deg;
  }
  .from {
    left: 9%;
    bottom: 11%;
    font-size: clamp(12px, 2.2vw, 15px);
    rotate: -2deg;
    opacity: 0.6;
  }
  /* a postage stamp with the film on it: scalloped edges, a little crooked */
  .postage {
    position: absolute;
    right: 6%;
    bottom: 10%;
    width: 15%;
    aspect-ratio: 0.8;
    padding: 5px;
    rotate: 4deg;
    background: #fbf6ec;
    -webkit-mask: radial-gradient(circle at 4px 4px, transparent 2.6px, #000 3px) -4px -4px / 8px 8px;
    mask: radial-gradient(circle at 4px 4px, transparent 2.6px, #000 3px) -4px -4px / 8px 8px;
    filter: drop-shadow(0 1px 1px rgb(0 0 0 / 0.25));
  }
  .postage-in {
    position: relative;
    width: 100%;
    height: 100%;
    overflow: hidden;
    background: var(--wax);
  }
  .postage-in img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    filter: sepia(0.35) saturate(1.2) contrast(1.05);
  }
  .postage-in span {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 2px;
    text-align: center;
    font: 800 7px var(--font-text);
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: #fff;
    text-shadow: 0 1px 2px rgb(0 0 0 / 0.8);
  }
  .postmark {
    position: absolute;
    right: 15%;
    bottom: 22%;
    width: 22%;
    rotate: -12deg;
    fill: none;
    stroke: rgb(40 30 70 / 0.5);
    stroke-width: 1.4;
    mix-blend-mode: multiply;
  }
  .postmark text {
    fill: rgb(40 30 70 / 0.55);
    stroke: none;
    font: 800 8px var(--font-text);
    letter-spacing: 0.08em;
  }

  /* the flap: lifts open after the seal cracks */
  .flap {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    height: 56%;
    z-index: 4;
    transform-origin: top center;
    transform-style: preserve-3d;
    animation: flap-open 650ms 1550ms cubic-bezier(0.45, 0, 0.25, 1) both;
  }
  .flap-paper {
    position: absolute;
    inset: 0;
    clip-path: polygon(0 0, 100% 0, 50% 100%);
    border-radius: 10px 10px 0 0;
    filter: brightness(0.97);
    box-shadow: inset 0 -20px 30px -20px rgb(80 50 20 / 0.25);
  }
  .flap::after {
    /* the shadow the flap throws on the pocket */
    content: "";
    position: absolute;
    inset: 0;
    clip-path: polygon(0 0, 100% 0, 50% 100%);
    background: linear-gradient(180deg, transparent 60%, rgb(60 30 10 / 0.18));
    pointer-events: none;
  }
  @keyframes flap-open {
    0% {
      transform: rotateX(0deg);
      z-index: 4;
    }
    49% {
      z-index: 4;
    }
    50% {
      z-index: 0;
    }
    100% {
      transform: rotateX(180deg);
      z-index: 0;
      filter: brightness(0.85);
    }
  }

  /* wax seal: raised, glossy, with the couch mark pressed in; it cracks */
  .seal {
    position: absolute;
    z-index: 5;
    left: 50%;
    top: 56%;
    width: 17%;
    aspect-ratio: 1;
    translate: -50% -50%;
    animation: seal-pop 400ms 1100ms cubic-bezier(0.3, 1.6, 0.5, 1) both;
  }
  .seal-l,
  .seal-r {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    border-radius: 47% 53% 50% 50% / 52% 46% 54% 48%;
    background:
      radial-gradient(60% 50% at 35% 28%, rgb(255 255 255 / 0.5), transparent 60%),
      radial-gradient(circle at 50% 55%, #e0308a 0 46%, #a9155c 64%, #6d0b3a 100%);
    box-shadow:
      0 6px 10px -2px rgb(60 0 30 / 0.55),
      inset 0 -4px 8px rgb(60 0 30 / 0.5),
      inset 0 3px 5px rgb(255 160 210 / 0.4);
  }
  .seal-l {
    clip-path: polygon(0 0, 54% 0, 44% 34%, 58% 52%, 46% 70%, 52% 100%, 0 100%);
    animation: crack-l 700ms 1500ms cubic-bezier(0.5, 0, 0.6, 1) both;
  }
  .seal-r {
    clip-path: polygon(54% 0, 100% 0, 100% 100%, 52% 100%, 46% 70%, 58% 52%, 44% 34%);
    animation: crack-r 700ms 1500ms cubic-bezier(0.5, 0, 0.6, 1) both;
  }
  .seal img {
    width: 58%;
    opacity: 0.9;
    filter: brightness(0) invert(0.15) sepia(1) saturate(4) hue-rotate(290deg) drop-shadow(0 1px 0 rgb(255 170 215 / 0.55)) drop-shadow(0 -1px 0 rgb(60 0 30 / 0.6));
  }
  @keyframes seal-pop {
    from {
      transform: scale(1.8);
      opacity: 0;
    }
  }
  @keyframes crack-l {
    30% {
      transform: translate(-3px, 0) rotate(-6deg);
    }
    100% {
      transform: translate(-60px, 90px) rotate(-40deg);
      opacity: 0;
    }
  }
  @keyframes crack-r {
    30% {
      transform: translate(3px, 0) rotate(6deg);
    }
    100% {
      transform: translate(60px, 90px) rotate(40deg);
      opacity: 0;
    }
  }

  /* ------------------------------------------------------------ the ticket */
  .ticket {
    position: absolute;
    z-index: 1;
    left: 5%;
    right: 5%;
    top: 8%;
    height: 82%;
    display: flex;
    color: var(--ink);
    border-radius: 6px;
    background:
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='200' height='200'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='.9' numOctaves='2' stitchTiles='stitch'/><feColorMatrix values='0 0 0 0 .4  0 0 0 0 .25  0 0 0 0 .2  0 0 0 .16 0'/></filter><rect width='100%' height='100%' filter='url(%23n)'/></svg>"),
      linear-gradient(170deg, #fbf3e3, #efe1c7);
    box-shadow:
      0 30px 50px -24px rgb(0 0 0 / 0.75),
      inset 0 0 0 1px rgb(120 70 40 / 0.18),
      inset 0 0 0 6px var(--cream),
      inset 0 0 0 7px rgb(196 32 111 / 0.35);
    /* the notches at both ends of the tear line */
    -webkit-mask:
      radial-gradient(circle at 76.5% 0, transparent 11px, #000 11.5px) top / 100% 51% no-repeat,
      radial-gradient(circle at 76.5% 100%, transparent 11px, #000 11.5px) bottom / 100% 51% no-repeat;
    mask:
      radial-gradient(circle at 76.5% 0, transparent 11px, #000 11.5px) top / 100% 51% no-repeat,
      radial-gradient(circle at 76.5% 100%, transparent 11px, #000 11.5px) bottom / 100% 51% no-repeat;
    animation: ticket-up 900ms 2050ms cubic-bezier(0.2, 1, 0.3, 1) both;
  }
  @keyframes ticket-up {
    0% {
      transform: translateY(0);
    }
    55% {
      transform: translateY(-86%) rotate(-1deg);
      z-index: 1;
    }
    56% {
      z-index: 6;
    }
    100% {
      transform: translateY(-50%) rotate(1.5deg) scale(1.04);
      z-index: 6;
    }
  }
  .t-main {
    flex: 0 0 76.5%;
    display: flex;
    gap: 14px;
    padding: 18px 16px 16px 18px;
    min-width: 0;
  }
  .t-art {
    flex: 0 0 28%;
    border-radius: 3px;
    overflow: hidden;
    background: var(--ink);
    box-shadow:
      0 0 0 2px var(--cream),
      0 0 0 3px rgb(59 16 41 / 0.4);
  }
  .t-art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    filter: saturate(1.05) contrast(1.05);
  }
  .t-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .t-kicker {
    display: flex;
    justify-content: space-between;
    font: 800 9px var(--font-text);
    letter-spacing: 0.24em;
    text-transform: uppercase;
    color: var(--wax);
    border-bottom: 1px dashed rgb(59 16 41 / 0.35);
    padding-bottom: 5px;
  }
  /* letterpress: the ink sits a little into the paper */
  .t-title {
    margin: 2px 0 0;
    font: 900 clamp(20px, 4.4vw, 34px) / 0.95 var(--font-display);
    letter-spacing: -0.03em;
    text-transform: uppercase;
    text-shadow:
      0 1px 0 rgb(255 255 255 / 0.55),
      0 -1px 0 rgb(59 16 41 / 0.25);
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }
  .t-meta {
    font: 600 10px var(--font-text);
    letter-spacing: 0.14em;
    text-transform: uppercase;
    opacity: 0.6;
  }
  .t-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    margin-top: auto;
    border-top: 1px solid rgb(59 16 41 / 0.25);
    border-bottom: 1px solid rgb(59 16 41 / 0.25);
  }
  .t-grid div {
    display: grid;
    padding: 6px 0;
  }
  .t-grid div + div {
    padding-left: 8px;
    border-left: 1px solid rgb(59 16 41 / 0.2);
  }
  .t-grid span {
    font: 700 8px var(--font-text);
    letter-spacing: 0.2em;
    text-transform: uppercase;
    opacity: 0.55;
  }
  .t-grid b {
    font: 900 clamp(11px, 2vw, 14px) var(--font-display);
    letter-spacing: 0.02em;
    white-space: nowrap;
  }
  .t-host {
    display: flex;
    align-items: center;
    gap: 7px;
    font: 500 11px var(--font-text);
  }
  .t-av {
    width: 20px;
    height: 20px;
    flex: none;
    border-radius: 50%;
    overflow: hidden;
    display: grid;
    place-items: center;
    background: var(--ink);
    color: var(--cream);
    font-size: 10px;
    font-weight: 800;
    filter: grayscale(0.2) sepia(0.2);
  }
  .t-av img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .t-note {
    margin: 0;
    font-family: "Segoe Print", "Bradley Hand", "Chalkboard SE", cursive;
    font-size: 12px;
    color: #2a2a55;
    rotate: -1deg;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  /* the stub: torn off at the door */
  .t-stub {
    flex: 1;
    position: relative;
    display: grid;
    align-content: center;
    justify-items: center;
    gap: 10px;
    padding: 14px 6px;
    background-image: radial-gradient(circle, rgb(59 16 41 / 0.45) 1.2px, transparent 1.6px);
    background-size: 2px 9px;
    background-repeat: repeat-y;
    background-position: left 0 top 0;
  }
  .admit {
    writing-mode: vertical-rl;
    rotate: 180deg;
    font: 900 15px var(--font-display);
    letter-spacing: 0.28em;
    color: var(--wax);
  }
  .bars {
    display: flex;
    gap: 2px;
    height: 34px;
  }
  .bars i {
    display: block;
    height: 100%;
    background: var(--ink);
  }
  .no {
    font: 700 9px ui-monospace, monospace;
    letter-spacing: 0.1em;
    opacity: 0.6;
  }
  /* the rubber stamp, for "I'm in" */
  .stamp {
    position: absolute;
    left: 36%;
    top: 30%;
    padding: 6px 16px;
    border: 3px solid #c0182c;
    border-radius: 8px;
    color: #c0182c;
    font: 900 26px var(--font-display);
    letter-spacing: 0.12em;
    rotate: -14deg;
    opacity: 0;
    mix-blend-mode: multiply;
    -webkit-mask: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='120' height='60'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='1.2' numOctaves='2'/><feColorMatrix values='0 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0 0 0 -2.2 1.6'/></filter><rect width='100%' height='100%' filter='url(%23n)'/></svg>");
    mask: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='120' height='60'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='1.2' numOctaves='2'/><feColorMatrix values='0 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0 0 0 -2.2 1.6'/></filter><rect width='100%' height='100%' filter='url(%23n)'/></svg>");
  }

  /* --------------------------------------------------------------- answers */
  .answers {
    position: relative;
    z-index: 7;
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
    justify-content: center;
    animation: up 500ms 2850ms cubic-bezier(0.2, 1, 0.3, 1) both;
  }
  .answers button {
    height: 48px;
    padding: 0 26px;
    border-radius: 999px;
    font: 800 16px var(--font-display);
    letter-spacing: -0.01em;
  }
  .yes {
    background:
      radial-gradient(80% 120% at 30% 0%, rgb(255 255 255 / 0.35), transparent 60%),
      linear-gradient(180deg, #e8338f, #b3155e);
    color: #fff;
    box-shadow:
      0 10px 24px -8px rgb(224 48 154 / 0.7),
      inset 0 1px 0 rgb(255 255 255 / 0.4),
      inset 0 -2px 0 rgb(80 0 40 / 0.4);
  }
  .no {
    background: linear-gradient(180deg, var(--cream), #e6d6b8);
    color: var(--ink);
    box-shadow:
      0 10px 24px -10px rgb(0 0 0 / 0.6),
      inset 0 1px 0 rgb(255 255 255 / 0.7),
      inset 0 -2px 0 rgb(120 80 40 / 0.25);
  }
  .later {
    height: 36px !important;
    padding: 0 12px !important;
    font: 600 13px var(--font-text) !important;
    color: rgb(255 255 255 / 0.6);
  }

  /* ---------------------------------------------------------------- leaving */
  .l-in .stamp {
    animation: stamp 420ms cubic-bezier(0.3, 1.5, 0.5, 1) both;
  }
  @keyframes stamp {
    from {
      opacity: 0;
      transform: scale(2.6);
    }
    to {
      opacity: 0.9;
      transform: scale(1);
    }
  }
  .l-in .env {
    animation:
      env-in 1000ms cubic-bezier(0.2, 1.2, 0.3, 1) both,
      fly-up 700ms 950ms cubic-bezier(0.6, 0, 0.8, 0.4) forwards;
  }
  @keyframes fly-up {
    to {
      transform: translate(0, -120vh) rotate(-12deg);
    }
  }
  .l-out .ticket {
    animation: ticket-down 500ms cubic-bezier(0.6, 0, 0.4, 1) both;
  }
  @keyframes ticket-down {
    from {
      transform: translateY(-50%) rotate(1.5deg) scale(1.04);
      z-index: 1;
    }
    to {
      transform: translateY(0);
      z-index: 1;
    }
  }
  .l-out .env {
    animation:
      env-in 1000ms both,
      slide-off 650ms 550ms cubic-bezier(0.6, 0, 0.8, 0.4) forwards;
  }
  .l-later .env {
    animation:
      env-in 1000ms both,
      slide-off 600ms cubic-bezier(0.6, 0, 0.8, 0.4) forwards;
  }
  @keyframes slide-off {
    to {
      transform: translate(-130vw, 6vh) rotate(-18deg);
    }
  }
  .leaving .answers {
    opacity: 0;
    transition: opacity 200ms ease;
    pointer-events: none;
  }
  .leaving .dim {
    animation: fade 500ms ease reverse both;
    animation-delay: 1100ms;
  }
  .l-later .dim {
    animation-delay: 300ms;
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }
  @keyframes up {
    from {
      opacity: 0;
      transform: translateY(16px);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .env,
    .flap,
    .seal,
    .seal-l,
    .seal-r,
    .ticket,
    .answers {
      animation-duration: 1ms !important;
      animation-delay: 0ms !important;
    }
  }
  @media (max-width: 600px) {
    .t-art {
      display: none;
    }
    .env {
      aspect-ratio: 1.25;
    }
  }
</style>
