// midnite™ couch Privacy Policy, served at /privacy (public). App Store
// Connect needs this URL. Same look as /terms (its <style> is reused).
import { TERMS_HTML } from "./terms";

const STYLE = TERMS_HTML.slice(TERMS_HTML.indexOf("<style>"), TERMS_HTML.indexOf("</style>") + 8);

export const PRIVACY_HTML = `<!doctype html>
<html lang="en"><head>
<meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Privacy Policy · midnite™ couch</title>
<meta name="description" content="What midnite™ couch keeps about you, and where.">
<link rel="icon" href="/couch.png">
${STYLE}</head><body><main>
<header><img src="/couch.png" alt=""><div><div class="eyebrow">midnite™ couch</div><h1>Privacy Policy</h1></div></header>
<p class="updated">Last updated 27 November 2026</p>

<section><h2>The short version</h2>
<ul>
<li>couch keeps as little as it can. Most settings and watch history stay on your device.</li>
<li>No ads, no tracking, no analytics, and we never sell your data.</li>
<li>Nothing is collected for advertising or shared with data brokers.</li>
</ul>
</section>

<section><h2>What's kept, and where</h2>
<div class="wrap"><table>
<tr><th>What</th><th>Where</th><th>How long</th></tr>
<tr><td>Discord id, name, picture (when you sign in)</td><td>couch server</td><td>Until you sign out</td></tr>
<tr><td>What's playing on each of your devices (title, position, device name)</td><td>couch server, in memory</td><td>About 1 minute after a device goes quiet</td></tr>
<tr><td>What you're watching, shown to your crew</td><td>couch server, in memory</td><td>About 2 minutes after you stop</td></tr>
<tr><td>Watch-together rooms, asks, reactions</td><td>couch server, in memory</td><td>Minutes</td></tr>
<tr><td>Favourites, reviews, ratings, "finished" posts</td><td>couch server</td><td>Until you delete them</td></tr>
<tr><td>Settings, history, downloads, sign-in tokens</td><td>Your device only</td><td>Until you delete the app</td></tr>
<tr><td>Lists and watch history</td><td>Simkl, only if you connect it</td><td>Under Simkl's privacy policy</td></tr>
<tr><td>Smart bar questions</td><td>Sent to Google Gemini to answer</td><td>Under Google's terms</td></tr>
</table></div>
</section>

<section><h2>Services couch talks to</h2>
<ul>
<li><b>Discord</b>: sign-in (profile, server list, roles in the midnite server).</li>
<li><b>TMDB / Cinemeta</b>: posters, titles, descriptions. Only the title being looked up is sent.</li>
<li><b>Simkl</b>: optional, for lists and history.</li>
<li><b>Cloudflare</b>: hosts the couch server and website.</li>
</ul>
</section>

<section><h2>Children</h2>
<p>couch isn't meant for children under 13 and doesn't knowingly collect anything from them.</p>
</section>

<section><h2>Your choices</h2>
<ul>
<li>Sign out in Settings to remove your session from the couch server.</li>
<li>Deleting the app removes everything kept on your device.</li>
<li>To have everything the couch server holds about you deleted, message the owner in the midnite Discord server.</li>
</ul>
</section>

<section><h2>Changes</h2>
<p>If this policy changes, the date at the top changes and we post it in the midnite server.</p>
</section>

<p><a href="/terms">Terms &amp; Conditions</a> · <a href="/install">Help</a></p>
<footer>midnite™ couch · pull up a seat.</footer>
</main></body></html>`;
