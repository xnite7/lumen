// /install: a step-by-step guide for getting couch on a PC or an iPhone / iPad,
// written for people who don't do this kind of thing. Picks your device for
// you, one step at a time, with a tick to keep your place.
const EXE = "https://github.com/xnite7/lumen/releases/download/couch/couch-setup.exe";
const IPA = "https://github.com/xnite7/lumen/releases/download/couch-ios/couch.ipa";
const ITUNES = "https://www.apple.com/itunes/download/win64";
const ICLOUD = "https://updates.cdn-apple.com/2020/windows/001-39935-20200911-1A70AA56-F448-11EA-8CC0-99D41950005E/iCloudSetup.exe";
const SIDELOADLY = "https://sideloadly.io";

import { ic, ICON } from "./icons";

const path = (...p: string[]) => `<div class="path">${p.map((x) => `<span>${x}</span>`).join("<i>›</i>")}</div>`;

export const INSTALL_HTML = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>how to install couch · midnite™</title>
<meta name="description" content="get couch on your PC, iPhone or iPad, step by step.">
<meta property="og:title" content="how to install couch">
<meta property="og:description" content="step by step, no tech skills needed.">
<meta property="og:image" content="/logo.png">
<link rel="icon" href="/logo.png">
<style>
  :root { --bg:#07070b; --fg:#f4eaf0; --dim:#9a8f98; --line:#211b22; --card:#0f0c10; --card2:#17121a; --neon:#e0309a; --soft:#ff6ec4; --ok:#3ddc84; --warn:#ffb25c; }
  * { box-sizing:border-box; margin:0; }
  body { background:var(--bg); color:var(--fg); font:17px/1.55 system-ui, -apple-system, "Segoe UI", sans-serif; -webkit-font-smoothing:antialiased; }
  body::before { content:""; position:fixed; inset:-20% -10% auto; height:70vh; background:radial-gradient(closest-side, #e0309a22, transparent); pointer-events:none; }
  a { color:inherit; }
  .wrap { max-width:760px; margin:0 auto; padding:0 20px; position:relative; }
  nav { display:flex; align-items:center; gap:10px; height:64px; }
  nav a { display:flex; align-items:center; gap:10px; font-weight:700; text-decoration:none; }
  nav img { width:28px; height:28px; }
  nav .back { margin-left:auto; color:var(--dim); font-size:14px; font-weight:500; }

  header { padding:36px 0 8px; text-align:center; }
  h1 { font-size:clamp(32px, 7vw, 48px); line-height:1.1; letter-spacing:-.02em; }
  header p { color:var(--dim); margin-top:10px; }

  /* the device picker */
  .pick { display:grid; grid-template-columns:repeat(2, 1fr); gap:12px; margin:28px 0 8px; }
  .pick button { all:unset; cursor:pointer; text-align:center; padding:20px 12px; border-radius:20px; background:var(--card); border:2px solid var(--line); transition:border-color .2s, transform .15s; }
  .pick button:active { transform:scale(.97); }
  .pick button.on { border-color:var(--neon); background:var(--card2); }
  .pick .em { font-size:40px; display:block; }
  .ic { display:inline-flex; width:1.15em; height:1.15em; vertical-align:-.22em; flex:none; }
  .ic svg { width:100%; height:100%; fill:currentColor; }
  .pick .em .ic { width:44px; height:44px; }
  .finish .em .ic { width:52px; height:52px; color:var(--soft); }
  .tip .ic { color:var(--soft); } .warn .ic { color:var(--warn); }
  .check .ic { width:1.3em; height:1.3em; }
  .step.done .check .ic { color:var(--ok); }
  .pick b { display:block; margin-top:6px; font-size:18px; }
  .pick small { color:var(--dim); font-size:13px; }
  .guess { text-align:center; color:var(--dim); font-size:14px; min-height:22px; }

  .guide { display:none; }
  .guide.on { display:block; }

  .need { background:var(--card); border:1px solid var(--line); border-radius:20px; padding:18px 20px; margin:20px 0; }
  .need h3 { font-size:15px; text-transform:uppercase; letter-spacing:.08em; color:var(--dim); margin-bottom:10px; }
  .need ul { list-style:none; padding:0; display:grid; gap:8px; }
  .need li { display:flex; gap:10px; align-items:flex-start; } .need li .ic { color:var(--ok); margin-top:.15em; }

  .bar { position:sticky; top:0; z-index:5; background:rgba(7,7,11,.85); backdrop-filter:blur(14px); -webkit-backdrop-filter:blur(14px); padding:12px 0; margin:0 -20px; padding-left:20px; padding-right:20px; }
  .bar .track { height:8px; border-radius:4px; background:var(--line); overflow:hidden; }
  .bar .fill { height:100%; width:0; background:linear-gradient(90deg, var(--neon), var(--soft)); transition:width .4s; }
  .bar .txt { font-size:13px; color:var(--dim); margin-top:6px; }

  .step { display:grid; grid-template-columns:48px 1fr; gap:16px; background:var(--card); border:1px solid var(--line); border-radius:22px; padding:20px; margin:14px 0; transition:opacity .3s, border-color .3s; }
  .step .n { width:48px; height:48px; border-radius:50%; background:var(--line); display:grid; place-items:center; font-weight:800; font-size:20px; }
  .step.done { border-color:#3ddc8455; }
  .step.done .n { background:var(--ok); color:#000; }
  .step .n .ok { display:none; width:26px; height:26px; }
  .step.done .n span { display:none; }
  .step.done .n .ok { display:inline-flex; }
  .step h2 { font-size:21px; line-height:1.25; }
  .step p { color:var(--dim); margin-top:8px; }
  .step p b, .step li b { color:var(--fg); }
  .step ol { color:var(--dim); margin:8px 0 0; padding-left:20px; display:grid; gap:6px; }
  .who { display:inline-block; font-size:12px; font-weight:700; padding:3px 9px; border-radius:8px; background:var(--card2); border:1px solid var(--line); margin-bottom:8px; }

  .dl { display:inline-flex; align-items:center; gap:10px; margin-top:14px; padding:14px 22px; border-radius:999px; background:var(--fg); color:#000; font-weight:700; text-decoration:none; }
  .dl.alt { background:transparent; color:var(--fg); border:1px solid #ffffff33; }
  .dls { display:flex; flex-wrap:wrap; gap:10px; }

  .path { display:flex; flex-wrap:wrap; align-items:center; gap:6px; margin-top:12px; font-size:15px; }
  .path span { background:var(--card2); border:1px solid var(--line); padding:6px 12px; border-radius:10px; font-weight:600; }
  .path i { color:var(--dim); font-style:normal; }

  .tip, .warn { margin-top:12px; padding:12px 14px; border-radius:14px; font-size:15px; }
  .tip { background:#e0309a14; border:1px solid #e0309a33; }
  .warn { background:#ffb25c12; border:1px solid #ffb25c40; }

  /* little drawings of what you'll see */
  .see { margin-top:14px; border-radius:14px; overflow:hidden; border:1px solid var(--line); font-size:14px; max-width:420px; }
  .see .smart { background:#0067b8; color:#fff; padding:16px 18px; }
  .see .smart b { font-size:18px; display:block; margin-bottom:6px; }
  .see .smart u { cursor:default; }
  .see .smart .row { display:flex; justify-content:flex-end; gap:8px; margin-top:12px; }
  .see .smart .row span { border:1px solid #fff9; padding:5px 14px; }
  .see .smart .hl { outline:3px solid var(--warn); outline-offset:3px; border-radius:2px; }
  .see .ios { background:#1c1c1e; color:#fff; padding:18px; text-align:center; }
  .see .ios b { display:block; font-size:16px; }
  .see .ios .btns { display:grid; grid-template-columns:1fr 1fr; margin:14px -18px -18px; border-top:1px solid #38383a; }
  .see .ios .btns span { padding:12px; color:#0a84ff; }
  .see .ios .btns span + span { border-left:1px solid #38383a; }
  .see .ios .hl { font-weight:700; background:#0a84ff22; }
  .see .app { background:#f2f2f7; color:#000; padding:14px; display:grid; gap:8px; }
  .see .app .f { background:#fff; border:1px solid #ccc; border-radius:6px; padding:8px 10px; color:#666; }
  .see .app .go { justify-self:end; background:#0a84ff; color:#fff; padding:6px 18px; border-radius:6px; font-weight:700; }

  .check { all:unset; cursor:pointer; margin-top:16px; display:inline-flex; align-items:center; gap:10px; padding:10px 16px; border-radius:999px; border:1px solid #ffffff2a; font-weight:600; font-size:15px; }
  .step.done .check { border-color:#3ddc8466; color:var(--ok); }

  .finish { text-align:center; padding:28px 20px; border-radius:22px; margin:20px 0; background:linear-gradient(135deg, #e0309a26, #ff6ec414); border:1px solid #e0309a44; }
  .finish .em { font-size:44px; }
  .finish h2 { margin-top:6px; }
  .finish p { color:var(--dim); margin-top:6px; }

  h2.sec { font-size:24px; margin:40px 0 4px; }
  details { border-bottom:1px solid var(--line); padding:16px 0; }
  summary { cursor:pointer; font-weight:600; list-style:none; display:flex; justify-content:space-between; gap:12px; }
  summary::-webkit-details-marker { display:none; }
  summary::after { content:"+"; color:var(--dim); font-size:22px; line-height:1; }
  details[open] summary::after { content:"–"; }
  details p { color:var(--dim); margin-top:10px; }

  .share { text-align:center; margin:44px 0 20px; }
  .share p { color:var(--dim); font-size:14px; margin-top:8px; }
  footer { border-top:1px solid var(--line); margin-top:40px; padding:28px 0; color:var(--dim); font-size:13px; text-align:center; }

  @media (max-width:480px) {
    .step { grid-template-columns:1fr; gap:10px; }
    .step .n { width:40px; height:40px; font-size:18px; }
  }
</style>
</head>
<body>
<div class="wrap">

<nav><a href="/"><img src="/logo.png" alt="">couch</a><a class="back" href="/">${ic("back")} back</a></nav>

<header>
  <h1>let's get couch<br>on your device</h1>
  <p>no tech skills needed. just follow the steps, one at a time. tap <b>done</b> as you go so you don't lose your place.</p>
</header>

<div class="pick" role="tablist">
  <button data-g="pc" role="tab"><span class="em">${ic("windows")}</span><b>Windows PC</b><small>about 2 minutes</small></button>
  <button data-g="ios" role="tab"><span class="em">${ic("apple")}</span><b>iPhone / iPad</b><small>about 10 minutes</small></button>
</div>
<p class="guess" id="guess"></p>

<!-- ========================= WINDOWS ========================= -->
<section class="guide" id="pc">
  <div class="need">
    <h3>you'll need</h3>
    <ul><li>${ic("check")}<span>a Windows 10 or 11 computer</span></li><li>${ic("check")}<span>a Discord account that's in the midnite server</span></li></ul>
  </div>

  <div class="bar"><div class="track"><div class="fill"></div></div><div class="txt"></div></div>

  <div class="step">
    <div class="n"><span>1</span>${ic("check", "ok")}</div>
    <div>
      <h2>download couch</h2>
      <p>press the button. a file called <b>couch-setup.exe</b> will download.</p>
      <a class="dl" href="${EXE}">${ic("download")} download couch for Windows</a>
      <button class="check">${ic("circle")} done</button>
    </div>
  </div>

  <div class="step">
    <div class="n"><span>2</span>${ic("check", "ok")}</div>
    <div>
      <h2>open the file you downloaded</h2>
      <p>click it at the top-right of your browser (the little ${ic("download")} icon), or find it in your <b>Downloads</b> folder and double-click it.</p>
      <button class="check">${ic("circle")} done</button>
    </div>
  </div>

  <div class="step">
    <div class="n"><span>3</span>${ic("check", "ok")}</div>
    <div>
      <h2>if a blue box pops up, press "More info", then "Run anyway"</h2>
      <p>this is normal. Windows shows it for any app it doesn't know yet. it's safe.</p>
      <div class="see"><div class="smart"><b>Windows protected your PC</b>Microsoft Defender SmartScreen prevented an unrecognized app from starting…<br><br><u class="hl">More info</u><div class="row"><span class="hl">Run anyway</span><span>Don't run</span></div></div></div>
      <p style="font-size:14px">no blue box? great, skip this one.</p>
      <button class="check">${ic("circle")} done</button>
    </div>
  </div>

  <div class="step">
    <div class="n"><span>4</span>${ic("check", "ok")}</div>
    <div>
      <h2>sign in with Discord</h2>
      <p>couch opens by itself. press ${ic("discord")} <b>sign in with Discord</b>, then <b>Authorize</b> in the page that opens.</p>
      <button class="check">${ic("circle")} done</button>
    </div>
  </div>

  <div class="finish">
    <div class="em">${ic("party")}</div>
    <h2>you're on the couch!</h2>
    <p>couch updates itself from now on. you never have to do this again.</p>
  </div>

  <h2 class="sec">stuck?</h2>
  <details><summary>the download got blocked by my browser</summary><p>in Chrome or Edge, click the download, then <b>Keep</b> (you may need to press ⋯ first). it's flagged only because not many people have downloaded it yet.</p></details>
  <details><summary>it says I'm not in the server</summary><p>you need to be in the midnite Discord server, with the same Discord account you signed in with. ask a friend for an invite, join, then press <b>try again</b> in couch.</p></details>
</section>

<!-- ========================= iPHONE / iPAD ========================= -->
<section class="guide" id="ios">
  <div class="need">
    <h3>you'll need</h3>
    <ul>
      <li>${ic("check")}<span>a Windows PC or Mac</span></li>
      <li>${ic("check")}<span>your phone's cable</span></li>
      <li>${ic("check")}<span>your Apple ID (the one you use for the App Store)</span></li>
    </ul>
  </div>

  <div class="bar"><div class="track"><div class="fill"></div></div><div class="txt"></div></div>

  <div class="step">
    <div class="n"><span>1</span>${ic("check", "ok")}</div>
    <div>
      <span class="who">${ic("windows")} Windows only · Mac users skip this</span>
      <h2>install iTunes and iCloud</h2>
      <p>open each one and click <b>Next</b> till it's done.</p>
      <div class="dls"><a class="dl" href="${ITUNES}">${ic("itunes")} iTunes</a><a class="dl" href="${ICLOUD}">${ic("icloud")} iCloud</a></div>
      <div class="warn">${ic("warn")} already have them from the <b>Microsoft Store</b>? uninstall those first. Sideloadly can't use them.</div>
      <button class="check">${ic("circle")} done</button>
    </div>
  </div>

  <div class="step">
    <div class="n"><span>2</span>${ic("check", "ok")}</div>
    <div>
      <span class="who">${ic("computer")} on your computer</span>
      <h2>get Sideloadly and couch</h2>
      <p>install Sideloadly like any app, and download couch.</p>
      <div class="dls"><a class="dl" href="${SIDELOADLY}" target="_blank" rel="noopener">${ic("link")} Sideloadly</a><a class="dl" href="${IPA}">${ic("download")} couch.ipa</a></div>
      <button class="check">${ic("circle")} done</button>
    </div>
  </div>

  <div class="step">
    <div class="n"><span>3</span>${ic("check", "ok")}</div>
    <div>
      <span class="who">${ic("phone")} on your phone</span>
      <h2>plug your phone in and tap "Trust"</h2>
      <div class="see"><div class="ios"><b>Trust This Computer?</b>Your settings and data will be accessible from this computer when connected.<div class="btns"><span class="hl">Trust</span><span>Don't Trust</span></div></div></div>
      <button class="check">${ic("circle")} done</button>
    </div>
  </div>

  <div class="step">
    <div class="n"><span>4</span>${ic("check", "ok")}</div>
    <div>
      <span class="who">${ic("computer")} on your computer</span>
      <h2>in Sideloadly: drop in couch.ipa, type your Apple ID, press Start</h2>
      <p>if it asks for your password or a code, type them in. wait for <b>Done</b>.</p>
      <button class="check">${ic("circle")} done</button>
    </div>
  </div>

  <div class="step">
    <div class="n"><span>5</span>${ic("check", "ok")}</div>
    <div>
      <span class="who">${ic("phone")} on your phone</span>
      <h2>allow the app</h2>
      ${path("Settings", "General", "VPN &amp; Device Management", "your email", "Trust")}
      ${path("Settings", "Privacy &amp; Security", "Developer Mode", "on")}
      <p>the phone restarts. tap <b>Turn On</b> when it's back.</p>
      <button class="check">${ic("circle")} done</button>
    </div>
  </div>

  <div class="step">
    <div class="n"><span>6</span>${ic("check", "ok")}</div>
    <div>
      <span class="who">${ic("phone")} on your phone</span>
      <h2>open couch and sign in with Discord</h2>
      <button class="check">${ic("circle")} done</button>
    </div>
  </div>

  <div class="finish">
    <div class="em">${ic("party")}</div>
    <h2>you're on the couch!</h2>
    <p>${ic("calendar")} free Apple IDs make the app stop after <b>7 days</b>. plug in and press <b>Start</b> in Sideloadly again to keep it going.</p>
  </div>

  <h2 class="sec">stuck?</h2>
  <details><summary>Sideloadly doesn't see my phone</summary><p>unlock it, tap Trust, try another cable or port. on Windows, make sure iTunes and iCloud are the ones from step 1.</p></details>
  <details><summary>couch won't open</summary><p>do step 5 again. if it's been 7 days, press Start in Sideloadly again.</p></details>
</section>

<div class="share">
  <a class="dl alt" href="#" id="share">${ic("share")} send this guide to a friend</a>
  <p id="shared"></p>
</div>

<footer>couch · midnite™ · <a href="/terms">terms</a></footer>
</div>

<script>
const PHONE = ${JSON.stringify(ic("phone"))}, DONE = ${JSON.stringify(ic("checkCircle"))}, TODO = ${JSON.stringify(ic("circle"))};
(() => {
  const store = { get(k) { try { return JSON.parse(localStorage.getItem(k) || "null"); } catch { return null; } }, set(k, v) { try { localStorage.setItem(k, JSON.stringify(v)); } catch {} } };
  const btns = [...document.querySelectorAll(".pick button")];
  function show(g, why) {
    btns.forEach((b) => b.classList.toggle("on", b.dataset.g === g));
    document.querySelectorAll(".guide").forEach((s) => s.classList.toggle("on", s.id === g));
    document.getElementById("guess").innerHTML = why || "";
    store.set("couch-install-guide", g);
  }
  btns.forEach((b) => b.addEventListener("click", () => { show(b.dataset.g); }));

  // which guide to open first: the one you picked before, or your device
  const ua = navigator.userAgent;
  const apple = /iPhone|iPad|iPod/.test(ua) || (/Macintosh/.test(ua) && navigator.maxTouchPoints > 1);
  const hash = location.hash.slice(1);
  if (hash === "pc" || hash === "ios") show(hash);
  else if (store.get("couch-install-guide")) show(store.get("couch-install-guide"));
  else if (apple) show("ios", PHONE + " looks like you're on an iPhone or iPad. you'll need a computer for a few of the steps, so you might want to open this page there too.");
  else show("pc");

  // ticking steps off, remembered per guide
  document.querySelectorAll(".guide").forEach((g) => {
    const steps = [...g.querySelectorAll(".step")];
    const key = "couch-install-" + g.id;
    const done = new Set(store.get(key) || []);
    const fill = g.querySelector(".fill"), txt = g.querySelector(".bar .txt");
    function paint() {
      steps.forEach((s, i) => {
        const on = done.has(i);
        s.classList.toggle("done", on);
        s.querySelector(".check").innerHTML = (on ? DONE : TODO) + " done";
      });
      fill.style.width = (done.size / steps.length) * 100 + "%";
      txt.textContent = done.size === steps.length ? "all done!" : "step " + Math.min(done.size + 1, steps.length) + " of " + steps.length;
    }
    steps.forEach((s, i) => s.querySelector(".check").addEventListener("click", () => {
      if (done.has(i)) done.delete(i); else {
        done.add(i);
        const next = steps.find((_, j) => !done.has(j));
        if (next) setTimeout(() => next.scrollIntoView({ behavior: "smooth", block: "center" }), 250);
      }
      store.set(key, [...done]);
      paint();
    }));
    paint();
  });

  document.getElementById("share").addEventListener("click", async (e) => {
    e.preventDefault();
    const url = location.origin + "/install";
    const out = document.getElementById("shared");
    try {
      if (navigator.share) await navigator.share({ title: "how to install couch", url });
      else { await navigator.clipboard.writeText(url); out.innerHTML = DONE + " link copied, paste it to your friend"; }
    } catch { out.textContent = url; }
  });
})();
</script>
</body>
</html>`;
