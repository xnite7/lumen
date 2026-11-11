<script lang="ts">
  // "Throw a watch party": pick when, pick who, add a line. The card on the
  // left is the ticket they'll get, filling in as you choose.
  import { api, img, type CrewPerson } from "../../lib/api";
  import { parties, throwParty } from "../../lib/stores/parties.svelte";
  import { social } from "../../lib/stores/social.svelte";
  import Icon from "../Icon.svelte";

  let people = $state<CrewPerson[]>([]);
  let everyone = $state(true);
  let picked = $state<Set<string>>(new Set());
  let note = $state("");
  let at = $state(0);
  let custom = $state("");
  let sending = $state(false);

  // quick picks: tonight, tomorrow, the coming Friday and Saturday
  function slot(daysAhead: number, h: number) {
    const d = new Date();
    d.setDate(d.getDate() + daysAhead);
    d.setHours(h, 0, 0, 0);
    return d;
  }
  function nextDow(dow: number, h: number) {
    const d = new Date();
    const add = (dow - d.getDay() + 7) % 7 || 7;
    return slot(add, h);
  }
  const picks = $derived.by(() => {
    const l: { label: string; d: Date }[] = [];
    const tonight = slot(0, 21);
    if (tonight.getTime() > Date.now() + 15 * 60_000) l.push({ label: "tonight 9pm", d: tonight });
    else l.push({ label: "in an hour", d: new Date(Math.ceil((Date.now() + 3600_000) / 900_000) * 900_000) });
    l.push({ label: "tomorrow 9pm", d: slot(1, 21) });
    l.push({ label: "friday 10pm", d: nextDow(5, 22) });
    l.push({ label: "saturday 9pm", d: nextDow(6, 21) });
    return l;
  });

  $effect(() => {
    if (!parties.plan) return;
    note = "";
    everyone = true;
    picked = new Set();
    at = picks[0].d.getTime();
    custom = "";
    api.people().then((r) => (people = (r.people ?? []).filter((p) => p.id !== social.me?.id))).catch(() => {});
  });
  $effect(() => {
    if (custom) {
      const t = new Date(custom).getTime();
      if (!isNaN(t)) at = t;
    }
  });

  function toggle(id: string) {
    const s = new Set(picked);
    s.has(id) ? s.delete(id) : s.add(id);
    picked = s;
    everyone = false;
  }
  async function send() {
    const plan = parties.plan;
    if (!plan || sending) return;
    sending = true;
    await throwParty(plan.meta, at, everyone ? "all" : [...picked], note.trim(), plan.videoId);
    sending = false;
  }
  const when = $derived(at ? new Date(at) : null);
  const valid = $derived(at > Date.now() - 60_000 && (everyone || picked.size > 0));
</script>

{#if parties.plan}
  {@const m = parties.plan.meta}
  <div class="scrim" role="presentation" onclick={() => (parties.plan = null)}></div>
  <div class="sheet glass strong" role="dialog" aria-label="Throw a watch party">
    <header>
      <span class="k">throw a watch party</span>
      <button class="x press" onclick={() => (parties.plan = null)} aria-label="Close"><Icon name="close" size={16} /></button>
    </header>

    <div class="body">
      <!-- the ticket they'll get -->
      <div class="mini-ticket">
        {#if m.poster}<img src={img(m.poster, "w342")} alt="" />{/if}
        <div class="mt-txt">
          <span class="mt-k">admit one</span>
          <b>{m.name}</b>
          {#if when}<span>{when.toLocaleDateString(undefined, { weekday: "short", day: "numeric", month: "short" })} · {when.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" })}</span>{/if}
          {#if note.trim()}<i>“{note.trim()}”</i>{/if}
        </div>
      </div>

      <div class="form">
        <h3>when</h3>
        <div class="picks">
          {#each picks as p}
            <button class="pick press" class:on={!custom && at === p.d.getTime()} onclick={() => ((custom = ""), (at = p.d.getTime()))}>{p.label}</button>
          {/each}
        </div>
        <label class="custom">
          <span>or pick a time</span>
          <input type="datetime-local" bind:value={custom} />
        </label>

        <h3>who</h3>
        <div class="who">
          <button class="pp everyone press" class:on={everyone} onclick={() => ((everyone = true), (picked = new Set()))}>
            <span class="av"><Icon name="couch" size={20} /></span>
            <span class="nm">everyone</span>
          </button>
          {#each people as p (p.id)}
            <button class="pp press" class:on={!everyone && picked.has(p.id)} onclick={() => toggle(p.id)} style:--acc={p.accent || "var(--neon)"}>
              <span class="av">{#if p.avatar}<img src={p.avatar} alt="" />{:else}{p.name.charAt(0).toUpperCase()}{/if}</span>
              <span class="nm">{p.name}</span>
            </button>
          {/each}
        </div>

        <h3>a line on the ticket <span>(optional)</span></h3>
        <input class="note" bind:value={note} maxlength="140" placeholder="bring snacks. no phones." />
      </div>
    </div>

    <button class="send press" disabled={!valid || sending} onclick={send}>{sending ? "sending…" : "send the invites"}</button>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 460;
    background: rgb(0 0 0 / 0.5);
    backdrop-filter: blur(6px);
  }
  .sheet {
    position: fixed;
    z-index: 461;
    left: 50%;
    top: 50%;
    translate: -50% -50%;
    width: min(760px, calc(100vw - 28px));
    max-height: calc(100vh - 40px);
    overflow: auto;
    display: grid;
    gap: 18px;
    padding: 20px 22px 22px;
    border-radius: 26px;
    animation: pop 380ms cubic-bezier(0.16, 1, 0.3, 1) both;
  }
  @keyframes pop {
    from {
      opacity: 0;
      scale: 0.95;
    }
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .k {
    font: 800 20px var(--font-display);
    letter-spacing: -0.02em;
  }
  .x {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: rgb(255 255 255 / 0.08);
  }
  .body {
    display: grid;
    grid-template-columns: 210px 1fr;
    gap: 22px;
  }
  .mini-ticket {
    align-self: start;
    border-radius: 8px;
    overflow: hidden;
    color: #3b1029;
    background: linear-gradient(170deg, #fbf3e3, #efe1c7);
    box-shadow:
      0 20px 40px -20px #000,
      inset 0 0 0 5px #f7eedd,
      inset 0 0 0 6px rgb(196 32 111 / 0.35);
    rotate: -2deg;
  }
  .mini-ticket img {
    width: 100%;
    aspect-ratio: 16 / 10;
    object-fit: cover;
    display: block;
    border-bottom: 2px dashed rgb(59 16 41 / 0.35);
  }
  .mt-txt {
    display: grid;
    gap: 3px;
    padding: 12px 14px 14px;
  }
  .mt-k {
    font: 800 9px var(--font-text);
    letter-spacing: 0.24em;
    text-transform: uppercase;
    color: #c4206f;
  }
  .mt-txt b {
    font: 900 18px / 1 var(--font-display);
    text-transform: uppercase;
    letter-spacing: -0.02em;
  }
  .mt-txt span {
    font: 700 11px var(--font-text);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }
  .mt-txt i {
    font-family: "Segoe Print", "Bradley Hand", cursive;
    font-style: normal;
    font-size: 12px;
    color: #2a2a55;
  }
  .form {
    display: grid;
    gap: 10px;
    min-width: 0;
  }
  h3 {
    margin: 6px 0 0;
    font: 700 13px var(--font-text);
    color: var(--label-2);
  }
  h3 span {
    font-weight: 400;
    color: var(--label-3);
  }
  .picks {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .pick {
    padding: 8px 14px;
    border-radius: 999px;
    background: rgb(255 255 255 / 0.08);
    font-size: 14px;
    font-weight: 600;
  }
  .pick.on {
    background: #fff;
    color: #000;
  }
  .custom {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13px;
    color: var(--label-3);
  }
  .custom input,
  .note {
    height: 40px;
    padding: 0 12px;
    border-radius: 12px;
    border: 0;
    background: rgb(255 255 255 / 0.08);
    color: var(--label);
    font: inherit;
    color-scheme: dark;
  }
  .note {
    width: 100%;
  }
  .who {
    display: flex;
    gap: 12px;
    overflow-x: auto;
    padding: 4px 2px 6px;
  }
  .pp {
    flex: none;
    width: 64px;
    display: grid;
    justify-items: center;
    gap: 6px;
    opacity: 0.55;
    transition: opacity 150ms ease;
  }
  .pp.on {
    opacity: 1;
  }
  .pp .av {
    width: 50px;
    height: 50px;
    border-radius: 50%;
    overflow: hidden;
    display: grid;
    place-items: center;
    background: rgb(255 255 255 / 0.08);
    font: 800 18px var(--font-display);
  }
  .pp.on .av {
    box-shadow:
      0 0 0 2px var(--black, #000),
      0 0 0 4px var(--acc, #fff);
  }
  .everyone.on .av {
    box-shadow:
      0 0 0 2px var(--black, #000),
      0 0 0 4px #fff;
  }
  .pp .av img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .pp .nm {
    font-size: 12px;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .send {
    height: 50px;
    border-radius: 999px;
    font: 800 16px var(--font-display);
    color: #fff;
    background:
      radial-gradient(80% 120% at 30% 0%, rgb(255 255 255 / 0.3), transparent 60%),
      linear-gradient(180deg, #e8338f, #b3155e);
    box-shadow:
      0 12px 26px -10px rgb(224 48 154 / 0.7),
      inset 0 1px 0 rgb(255 255 255 / 0.4);
  }
  .send:disabled {
    opacity: 0.45;
  }
  @media (max-width: 640px) {
    .body {
      grid-template-columns: 1fr;
    }
    .mini-ticket {
      max-width: 240px;
      justify-self: center;
    }
  }
</style>
