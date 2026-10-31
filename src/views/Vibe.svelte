<script lang="ts">
  // Vibe search: ask for a feeling, not a title. Picks come from your own
  // Simkl ratings. Suggestions up top, then everything you've asked before,
  // kept on this device so old picks reopen instantly (no new request).
  import { api, img } from "../lib/api";
  import { app } from "../lib/stores/app.svelte";
  import { go } from "../lib/router.svelte";
  import { ui } from "../lib/ui.svelte";
  import { openDetail } from "../lib/open";
  import { errorText } from "../lib/stores/toast.svelte";
  import { haptic } from "../lib/native";
  import NavBar from "../ui/NavBar.svelte";
  import Icon from "../components/Icon.svelte";

  let { q: initial = "" }: { q?: string } = $props();

  type Result = { read?: string; picks: any[] };
  type Past = { q: string; at: number; result: Result };
  const KEY = "lumen.vibe.history";

  const placeholders = [
    "what's the vibe today?",
    "cry all night tonight",
    "something that ruins me a little",
    "a cinematography masterpiece",
    "slow, gorgeous, nothing happens",
    "something mean and funny",
  ];
  let ph = $state(0);
  let q = $state("");
  let asked = $state("");
  let busy = $state(false);
  let result = $state<Result | null>(null);
  let error = $state("");
  let chips = $state<{ label: string; q: string }[]>([]);
  let focused = $state(false);
  let input: HTMLInputElement | undefined = $state();
  let history = $state<Past[]>(loadHistory());

  const ready = $derived(!!app.secrets.anthropic && !!app.secrets.simkl_token);

  function loadHistory(): Past[] {
    try {
      const v = JSON.parse(localStorage.getItem(KEY) ?? "[]");
      return Array.isArray(v) ? v : [];
    } catch {
      return [];
    }
  }
  function saveHistory() {
    try {
      localStorage.setItem(KEY, JSON.stringify(history.slice(0, 30)));
    } catch {}
  }

  $effect(() => {
    const t = setInterval(() => !focused && !q && (ph = (ph + 1) % placeholders.length), 4200);
    return () => clearInterval(t);
  });
  $effect(() => {
    if (ready) api.vibePrompts().then((r) => (chips = r?.prompts ?? [])).catch(() => {});
  });
  $effect(() => {
    if (initial) ask(initial);
    else if (!ui.phone) requestAnimationFrame(() => input?.focus());
  });

  async function ask(query = q) {
    const text = query.trim();
    if (!text || busy) return;
    q = text;
    asked = text;
    input?.blur();
    busy = true;
    error = "";
    result = null;
    try {
      const r = await api.vibe(text);
      result = r;
      history = [{ q: text, at: Date.now(), result: r }, ...history.filter((h) => h.q.toLowerCase() !== text.toLowerCase())];
      saveHistory();
      haptic("success");
    } catch (e) {
      error = errorText(e);
    }
    busy = false;
  }

  function reopen(h: Past) {
    q = h.q;
    asked = h.q;
    error = "";
    result = h.result;
    document.querySelector("[data-scroller]")?.scrollTo({ top: 0, behavior: "smooth" });
  }
  function forget(h: Past) {
    history = history.filter((x) => x !== h);
    saveHistory();
  }
  function clearAll() {
    history = [];
    saveHistory();
  }
  function reset() {
    q = "";
    asked = "";
    result = null;
    error = "";
    input?.focus();
  }

  const ago = (t: number) => {
    const m = Math.round((Date.now() - t) / 60000);
    if (m < 1) return "just now";
    if (m < 60) return `${m} min ago`;
    const h = Math.round(m / 60);
    if (h < 24) return `${h} h ago`;
    const d = Math.round(h / 24);
    return d === 1 ? "yesterday" : `${d} days ago`;
  };
</script>

<NavBar title="Vibe" />

<div class="vibe">
  <h1 class="large-title">Vibe</h1>
  <p class="lede sub dim">Describe a feeling. Lumen picks from what you've loved.</p>

  <form
    class="field glass"
    class:busy
    class:focused
    onsubmit={(e) => {
      e.preventDefault();
      ask();
    }}
  >
    <span class="blobs" aria-hidden="true"></span>
    <Icon name="sparkles" size={19} />
    <input
      bind:this={input}
      bind:value={q}
      placeholder={placeholders[ph]}
      onfocus={() => (focused = true)}
      onblur={() => (focused = false)}
      aria-label="Describe a mood"
      spellcheck="false"
      enterkeyhint="search"
      disabled={!ready}
    />
    {#if q && !busy && (result || error) && q === asked}
      <button type="button" class="go clear press" onclick={reset} aria-label="Clear"><Icon name="close" size={16} /></button>
    {:else if q}
      <button type="submit" class="go press" disabled={busy} aria-label="Ask">
        <Icon name={busy ? "refresh" : "forward"} size={18} class={busy ? "spin" : ""} />
      </button>
    {/if}
  </form>

  {#if !ready}
    <section class="setup group">
      <p class="sub">Vibe search needs Simkl (your ratings) and a Claude key.</p>
      <div class="setup-row">
        <button class="btn accent small press" onclick={() => go({ name: "settings", section: "integrations" })}>Set up</button>
      </div>
    </section>
  {/if}

  {#if busy}
    <p class="read thinking">Reading “{asked}”…</p>
    <div class="picks">
      {#each Array(3) as _}<div class="pick ghost skeleton"></div>{/each}
    </div>
  {:else if error}
    <p class="foot err">{error}</p>
  {:else if result}
    <section class="answer rise">
      {#if result.read}<p class="read">{result.read}</p>{/if}
      <div class="picks">
        {#each result.picks as p, i (p.id ?? p.name)}
          <button class="pick glass press" style:--d="{i * 60}ms" onclick={(e) => openDetail(p, e.currentTarget)}>
            {#if p.poster}<img src={img(p.poster, "w342")} alt="" />{:else}<span class="noart"><Icon name="film" size={22} /></span>{/if}
            <span class="txt">
              <span class="headline">{p.name}</span>
              <span class="foot faint">{[p.year, p.type === "series" ? "Series" : "Film"].filter(Boolean).join(" · ")}</span>
              {#if p.why}<span class="why foot dim">{p.why}</span>{/if}
            </span>
          </button>
        {/each}
      </div>
    </section>
  {/if}

  {#if ready && chips.length && !busy}
    <section>
      <h3 class="group-head">Try</h3>
      <div class="chips">
        {#each chips as c}
          <button class="chip press" onclick={() => ask(c.q)}><Icon name="sparkles" size={13} /> {c.label}</button>
        {/each}
      </div>
    </section>
  {/if}

  {#if history.length}
    <section>
      <div class="sect-h">
        <h3 class="group-head">Earlier</h3>
        <button class="btn plain small" onclick={clearAll}>Clear</button>
      </div>
      <div class="group past">
        {#each history as h (h.q + h.at)}
          <div class="past-row" class:on={result === h.result}>
            <button class="past-main press" onclick={() => reopen(h)}>
              <span class="stack-art" aria-hidden="true">
                {#each h.result.picks.slice(0, 3) as p, i}
                  {#if p.poster}<img src={img(p.poster, "small")} alt="" style:--i={i} />{/if}
                {/each}
              </span>
              <span class="past-txt">
                <span class="body">{h.q}</span>
                <span class="foot faint">{h.result.picks.length} picks · {ago(h.at)}</span>
              </span>
            </button>
            <button class="x press" onclick={() => forget(h)} aria-label="Remove"><Icon name="close" size={14} /></button>
          </div>
        {/each}
      </div>
    </section>
  {/if}
</div>

<style>
  .vibe {
    max-width: 900px;
    margin: 0 auto;
    padding: calc(var(--safe-top) + var(--bar-h) + 18px) var(--gutter) calc(var(--safe-bottom) + 120px);
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  .large-title {
    margin-bottom: -12px;
  }
  .lede {
    margin: 0;
  }
  .field {
    position: sticky;
    top: calc(var(--safe-top) + 64px);
    background: color-mix(in srgb, var(--bg) 72%, transparent);
    backdrop-filter: blur(24px) saturate(170%);
    -webkit-backdrop-filter: blur(24px) saturate(170%);
    z-index: 30;
    isolation: isolate;
    display: flex;
    align-items: center;
    gap: 12px;
    height: 54px;
    padding: 0 8px 0 18px;
    border-radius: var(--r-pill);
    overflow: hidden;
    transition: box-shadow var(--t-fast);
  }
  .field.focused {
    box-shadow: inset 0 0 0 1.5px color-mix(in srgb, var(--accent) 55%, transparent);
  }
  .blobs {
    position: absolute;
    inset: -40%;
    z-index: -1;
    background:
      radial-gradient(30% 60% at 20% 50%, color-mix(in srgb, var(--ambient-1) 55%, transparent), transparent),
      radial-gradient(30% 60% at 70% 40%, color-mix(in srgb, var(--ambient-2) 50%, transparent), transparent),
      radial-gradient(25% 50% at 50% 70%, color-mix(in srgb, var(--ambient-3) 45%, transparent), transparent);
    filter: blur(24px);
    opacity: 0.5;
    animation: drift 14s ease-in-out infinite alternate;
  }
  .busy .blobs {
    animation-duration: 3s;
    opacity: 0.9;
  }
  @keyframes drift {
    to {
      transform: translate(12%, -6%) rotate(20deg);
    }
  }
  input {
    flex: 1;
    min-width: 0;
    height: 100%;
    background: none;
    border: 0;
    outline: 0;
    font-size: 17px;
    color: var(--label);
  }
  input::placeholder {
    color: var(--label-3);
  }
  .go {
    flex: none;
    width: 40px;
    height: 40px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--label);
    color: var(--bg);
  }
  .go.clear {
    width: 30px;
    height: 30px;
    margin-right: 5px;
    background: var(--fill-2);
    color: var(--label-2);
  }
  :global(.spin) {
    animation: spin 1s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .setup {
    padding: 16px;
  }
  .setup p {
    margin: 0 0 12px;
  }
  .read {
    margin: 4px 4px 12px;
    font-family: var(--font-display);
    font-size: 19px;
    font-weight: 500;
    letter-spacing: -0.01em;
    line-height: 1.35;
  }
  .thinking {
    color: var(--label-2);
    animation: pulse 1.6s ease-in-out infinite;
  }
  @keyframes pulse {
    50% {
      opacity: 0.5;
    }
  }
  .err {
    margin: 0 4px;
    color: var(--red);
  }
  .picks {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 12px;
  }
  .pick {
    display: flex;
    gap: 14px;
    padding: 12px;
    border-radius: var(--r-md);
    text-align: left;
    animation: rise 520ms var(--spring) both;
    animation-delay: var(--d, 0ms);
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(14px) scale(0.98);
    }
  }
  .pick.ghost {
    height: 170px;
    animation: none;
  }
  .pick img,
  .noart {
    width: 88px;
    aspect-ratio: 2 / 3;
    object-fit: cover;
    border-radius: 10px;
    flex: none;
  }
  .noart {
    display: grid;
    place-items: center;
    background: var(--fill-3);
    color: var(--label-3);
  }
  .txt {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }
  .why {
    margin-top: 6px;
    display: -webkit-box;
    -webkit-line-clamp: 6;
    line-clamp: 6;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .chips .chip {
    height: 34px;
    gap: 6px;
    font-size: 14px;
    font-weight: 500;
  }
  .sect-h {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
  }
  .past {
    padding: 0;
    overflow: hidden;
  }
  .past-row {
    display: flex;
    align-items: center;
    padding-right: 8px;
    transition: background var(--t-fast);
  }
  .past-row + .past-row {
    box-shadow: inset 0 0.5px 0 var(--separator);
  }
  .past-row.on {
    background: var(--fill-4, var(--fill-3));
  }
  .past-main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 10px 14px;
    text-align: left;
  }
  .stack-art {
    position: relative;
    flex: none;
    width: 58px;
    height: 52px;
  }
  .stack-art img {
    position: absolute;
    top: 0;
    left: calc(var(--i) * 12px);
    width: 34px;
    height: 51px;
    object-fit: cover;
    border-radius: 6px;
    box-shadow: 0 2px 8px rgb(0 0 0 / 0.35);
    z-index: calc(3 - var(--i));
  }
  .past-txt {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .past-txt .body {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .x {
    flex: none;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: var(--label-3);
  }
  @media (max-width: 759px) {
    .picks {
      grid-template-columns: 1fr;
    }
    .field {
      height: 50px;
      padding-left: 16px;
    }
  }
</style>
