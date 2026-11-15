// The client's one page. Layout only: the look gets designed later, so the
// styles here are just structure (sections, grid, spacing) on plain colours.
const ZIP = "https://github.com/xnite7/lumen/releases/download/couch/couch-gpui.zip";

export const DOWNLOAD_HTML = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>couch · midnite™</title>
<meta name="description" content="couch is midnite™'s streaming app. watch together, on one couch.">
<meta property="og:title" content="couch · midnite™">
<meta property="og:description" content="watch together, on one couch.">
<meta property="og:image" content="/logo.png">
<link rel="icon" href="/logo.png">
<style>
  :root { --bg:#07070b; --fg:#f2f2f5; --dim:#8b8b96; --line:#1c1c24; --card:#0e0e14; --max:1120px; }
  * { box-sizing:border-box; margin:0; }
  body { background:var(--bg); color:var(--fg); font:16px/1.55 system-ui, -apple-system, "Segoe UI", sans-serif; }
  a { color:inherit; }
  .wrap { max-width:var(--max); margin:0 auto; padding:0 24px; }
  section { padding:96px 0; border-top:1px solid var(--line); }
  h2 { font-size:32px; margin-bottom:12px; }
  .lead { color:var(--dim); max-width:560px; }

  /* nav */
  nav { position:sticky; top:0; z-index:5; background:rgba(7,7,11,.8); backdrop-filter:blur(12px); border-bottom:1px solid var(--line); }
  nav .wrap { display:flex; align-items:center; gap:24px; height:64px; }
  nav .brand { display:flex; align-items:center; gap:10px; font-weight:600; text-decoration:none; }
  nav .brand img { width:28px; height:28px; }
  nav .links { display:flex; gap:20px; margin-left:auto; color:var(--dim); font-size:14px; }
  nav .links a { text-decoration:none; }

  /* hero */
  .hero { border-top:0; padding:120px 0 80px; text-align:center; }
  .hero h1 { font-size:clamp(40px, 7vw, 76px); line-height:1.05; letter-spacing:-.02em; }
  .hero p { color:var(--dim); font-size:19px; margin:18px auto 0; max-width:560px; }
  .cta { display:inline-flex; flex-direction:column; align-items:center; gap:10px; margin-top:36px; }
  .btn { display:inline-block; padding:14px 30px; border-radius:999px; background:var(--fg); color:var(--bg); font-weight:600; text-decoration:none; }
  .btn.ghost { background:transparent; color:var(--fg); border:1px solid var(--line); }
  .meta { color:var(--dim); font-size:13px; }
  .shot { margin:64px auto 0; aspect-ratio:16/9; max-width:980px; border-radius:16px; background:var(--card); border:1px solid var(--line); display:grid; place-items:center; color:var(--dim); font-size:13px; }

  /* features */
  .grid { display:grid; grid-template-columns:repeat(3, 1fr); gap:16px; margin-top:40px; }
  .feat { background:var(--card); border:1px solid var(--line); border-radius:14px; padding:24px; }
  .feat .ico { width:40px; height:40px; border-radius:10px; background:var(--line); margin-bottom:16px; }
  .feat h3 { font-size:17px; margin-bottom:6px; }
  .feat p { color:var(--dim); font-size:14px; }

  /* spotlight rows */
  .row { display:grid; grid-template-columns:1fr 1fr; gap:48px; align-items:center; padding:48px 0; }
  .row.flip .media { order:-1; }
  .row .media { aspect-ratio:4/3; border-radius:14px; background:var(--card); border:1px solid var(--line); display:grid; place-items:center; color:var(--dim); font-size:13px; }
  .row h3 { font-size:26px; margin-bottom:10px; }
  .row p { color:var(--dim); }

  /* install */
  .steps { display:grid; grid-template-columns:repeat(3, 1fr); gap:16px; margin-top:40px; counter-reset:s; }
  .step { border:1px solid var(--line); border-radius:14px; padding:24px; }
  .step::before { counter-increment:s; content:counter(s); display:grid; place-items:center; width:32px; height:32px; border-radius:50%; background:var(--line); font-weight:600; margin-bottom:14px; }
  .step p { color:var(--dim); font-size:14px; }
  .req { display:flex; flex-wrap:wrap; gap:10px; margin-top:24px; }
  .req span { border:1px solid var(--line); border-radius:999px; padding:6px 14px; font-size:13px; color:var(--dim); }

  /* faq */
  details { border-bottom:1px solid var(--line); padding:20px 0; }
  summary { cursor:pointer; font-weight:600; list-style:none; display:flex; justify-content:space-between; }
  summary::after { content:"+"; color:var(--dim); }
  details[open] summary::after { content:"–"; }
  details p { color:var(--dim); margin-top:10px; max-width:720px; }

  /* final cta + footer */
  .end { text-align:center; }
  footer { border-top:1px solid var(--line); padding:32px 0; color:var(--dim); font-size:13px; }
  footer .wrap { display:flex; gap:20px; align-items:center; }
  footer .right { margin-left:auto; display:flex; gap:20px; }

  @media (max-width:820px) {
    .grid, .steps, .row { grid-template-columns:1fr; }
    .row.flip .media { order:0; }
    nav .links { display:none; }
    section { padding:64px 0; }
  }
</style>
</head>
<body>

<nav>
  <div class="wrap">
    <a class="brand" href="#top"><img src="/logo.png" alt="">couch</a>
    <div class="links"><a href="#features">features</a><a href="#install">install</a><a href="#faq">faq</a></div>
    <a class="btn" href="${ZIP}" style="padding:8px 18px;font-size:14px">download</a>
  </div>
</nav>

<header class="hero" id="top">
  <div class="wrap">
    <h1>watch together,<br>on one couch.</h1>
    <p>couch is midnite™'s streaming app. find something, press play, and the whole crew is in the room with you.</p>
    <div class="cta">
      <a class="btn" href="${ZIP}">download for windows</a>
      <span class="meta">windows 10 / 11 · 64-bit · free for midnite members</span>
    </div>
    <div class="shot">[ app screenshot / trailer ]</div>
  </div>
</header>

<section id="features">
  <div class="wrap">
    <h2>everything in one place</h2>
    <p class="lead">films, shows and anime, with the people you actually watch with.</p>
    <div class="grid">
      <div class="feat"><div class="ico"></div><h3>watch parties</h3><p>one host, everyone in sync. see who's watching and react in real time.</p></div>
      <div class="feat"><div class="ico"></div><h3>4k &amp; hdr</h3><p>picks the best stream it can find, with audio and subtitle tracks to match.</p></div>
      <div class="feat"><div class="ico"></div><h3>immersive light</h3><p>the film's colours glow softly into the black bars.</p></div>
      <div class="feat"><div class="ico"></div><h3>simkl sync</h3><p>your history and watchlist follow you, nothing to re-mark.</p></div>
      <div class="feat"><div class="ico"></div><h3>on the couch</h3><p>see what your friends are watching right now and jump in.</p></div>
      <div class="feat"><div class="ico"></div><h3>picture-in-picture</h3><p>keep it playing in a corner while you do something else.</p></div>
    </div>
  </div>
</section>

<section>
  <div class="wrap">
    <div class="row">
      <div><h3>the room, not just the remote</h3><p>faces of everyone watching, custom emoji reactions floating up the screen, and an invite that's one click away.</p></div>
      <div class="media">[ watch party visual ]</div>
    </div>
    <div class="row flip">
      <div><h3>made to feel like a cinema</h3><p>a quiet player that gets out of the way, previews on the scrub bar, and colours pulled from every title.</p></div>
      <div class="media">[ player visual ]</div>
    </div>
    <div class="row">
      <div><h3>always finds the next one</h3><p>"because you loved", next episode cards, a calendar for what's out this week.</p></div>
      <div class="media">[ home / discovery visual ]</div>
    </div>
  </div>
</section>

<section id="install">
  <div class="wrap">
    <h2>install in a minute</h2>
    <p class="lead">couch keeps itself up to date after this.</p>
    <div class="steps">
      <div class="step"><h3>download</h3><p>grab the zip with the button below.</p></div>
      <div class="step"><h3>unzip &amp; open</h3><p>extract it anywhere and open couch.</p></div>
      <div class="step"><h3>sign in with discord</h3><p>you'll need to be in the midnite server.</p></div>
    </div>
    <div class="req"><span>windows 10 / 11</span><span>64-bit</span><span>a discord account in midnite</span><span>an internet connection</span></div>
    <p class="meta" style="margin-top:24px">on the old couch? it'll offer to move you over automatically.</p>
  </div>
</section>

<section id="faq">
  <div class="wrap">
    <h2>questions</h2>
    <details><summary>is it free?</summary><p>yes, for everyone in the midnite discord server.</p></details>
    <details><summary>why do i need discord?</summary><p>couch is for the midnite crew. signing in with discord is how it knows you're one of us.</p></details>
    <details><summary>i had the old couch. what happens to it?</summary><p>it's retired. open it once and it'll install the new one for you, or just download it here. your simkl history comes with you.</p></details>
    <details><summary>windows says it doesn't recognise the app</summary><p>click "more info" then "run anyway". it's not signed by a big publisher yet.</p></details>
    <details><summary>mac or linux?</summary><p>windows only for now.</p></details>
  </div>
</section>

<section class="end">
  <div class="wrap">
    <h2>pull up a seat.</h2>
    <div class="cta"><a class="btn" href="${ZIP}">download couch</a><span class="meta">ask someone in midnite for an invite.</span></div>
  </div>
</section>

<footer>
  <div class="wrap">
    <span>couch · midnite™</span>
    <div class="right"><a href="/terms">terms</a></div>
  </div>
</footer>

</body>
</html>`;
