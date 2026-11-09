<script lang="ts">
  import DiscordSignIn from "../ui/DiscordSignIn.svelte";
  // The Profile tab: who you are, then everything else in short groups.
  // A search field finds any setting by name.
  import { api } from "../lib/api";
  import { app, refreshSecrets } from "../lib/stores/app.svelte";
  import { go } from "../lib/router.svelte";
  import { ui } from "../lib/ui.svelte";
  import { SECTIONS, SETTINGS_INDEX, sectionById } from "../lib/sections";
  import { toast } from "../lib/stores/toast.svelte";
  import { haptic } from "../lib/native";
  import NavBar from "../ui/NavBar.svelte";
  import Item from "../ui/Item.svelte";
  import SignIn from "../ui/SignIn.svelte";
  import Sheet from "../components/Sheet.svelte";
  import Icon from "../components/Icon.svelte";

  let q = $state("");
  let signIn = $state(false);
  let addonCount = $state<number | null>(null);
  let syncing = $state(false);

  $effect(() => {
    api.addons().then((a) => (addonCount = a.filter((x) => !x.flags.protected).length)).catch(() => {});
  });

  const user = $derived(app.secrets.discordUser);
  const signed = $derived(!!user);
  const name = $derived((user?.name as string) || "");
  const initial = $derived(name.charAt(0).toUpperCase());

  const visible = (id: string) => {
    const s = sectionById(id);
    return !s?.only || s.only.includes(app.platform as any);
  };

  const results = $derived.by(() => {
    const t = q.trim().toLowerCase();
    if (!t) return [];
    const sec = SECTIONS.filter((s) => visible(s.id) && s.label.toLowerCase().includes(t)).map((s) => ({ label: s.label, hint: "", section: s.id }));
    const hits = SETTINGS_INDEX.filter((x) => visible(x.section) && (x.label.toLowerCase().includes(t) || x.hint.toLowerCase().includes(t)));
    return [...sec, ...hits].slice(0, 30);
  });

  const groups: string[][] = [
    ["playback", "subtitles", "streams"],
    ["accounts", "integrations"],
    ["network", "storage", "about"],
    ["experimental"],
  ];

  async function sync() {
    syncing = true;
    try {
      const r = await api.stremioSync(true);
      toast(`Synced · ${r.pulled} in, ${r.pushed} out`, "ok");
      haptic("success");
    } catch (e) {
      toast(String(e), "error");
    }
    syncing = false;
  }
  async function signOut() {
    await api.discordLogout();
    await refreshSecrets();
  }
</script>

<NavBar title="Profile" />

<div class="profile">
  <h1 class="large-title">Profile</h1>

  <!-- account -->
  <section class="account group">
    {#if signed}
      <button class="me press mine" onclick={() => user?.id && go({ name: "member", id: String(user.id), who: name, avatar: user?.avatar as string | undefined })}>
        <span class="avatar">{#if user?.avatar}<img src={user.avatar} alt="" />{:else}{initial || "?"}{/if}</span>
        <div class="who">
          <span class="title3">{name}</span>
          <span class="foot dim">your page on the couch · customise it</span>
        </div>
        <Icon name="chevron" size={16} />
      </button>
    {:else}
      <div class="me col">
        <div class="who">
          <span class="title3">who's on the couch?</span>
          <span class="foot dim">Sign in so the crew knows it's you</span>
        </div>
        <DiscordSignIn />
      </div>
    {/if}
  </section>

  <!-- search -->
  <label class="search">
    <Icon name="search" size={17} />
    <input bind:value={q} placeholder="Search settings" spellcheck="false" autocapitalize="off" />
    {#if q}<button class="clear" onclick={() => (q = "")} aria-label="Clear"><Icon name="close" size={12} /></button>{/if}
  </label>

  {#if q.trim()}
    {#if results.length}
      <div class="group rise">
        {#each results as r}
          {@const s = sectionById(r.section)}
          <Item label={r.label} detail={s && r.label !== s.label ? s.label : r.hint} icon={s?.icon} tint={s?.tint} chevron onclick={() => go({ name: "settings", section: r.section })} />
        {/each}
      </div>
    {:else}
      <p class="none dim">No settings match “{q}”.</p>
    {/if}
  {:else}

    {#each groups as g}
      <div class="group">
        {#each g.filter(visible) as id}
          {@const s = sectionById(id)!}
          <Item label={s.label} icon={s.icon} tint={s.tint} chevron onclick={() => go({ name: "settings", section: id })} />
        {/each}
      </div>
    {/each}

    <div class="group">
      <Item label="Run setup again" icon="wand" tint="var(--orange)" chevron onclick={() => (ui.onboarding = true)} />
      {#if signed}<Item label="Sign out" destructive onclick={signOut} />{/if}
    </div>

    <p class="foot faint ver">midnite™ couch {app.version}</p>
  {/if}
</div>

<Sheet bind:open={signIn} title="Sign in" width={520}>
  <div class="sheet-signin">
    <p class="callout dim">Use the account you have in Stremio. Apple, Facebook and email accounts all work.</p>
    <SignIn ondone={() => (signIn = false)} />
  </div>
</Sheet>

<style>
  .mine {
    width: 100%;
    text-align: left;
  }
  .mine > :global(svg) {
    margin-left: auto;
    opacity: 0.5;
  }
  .profile {
    max-width: 720px;
    margin: 0 auto;
    padding: calc(var(--safe-top) + var(--bar-h) + 18px) var(--gutter) calc(var(--safe-bottom) + 120px);
    display: flex;
    flex-direction: column;
    gap: 22px;
  }
  .large-title {
    margin-bottom: -6px;
  }
  .me {
    display: flex;
    align-items: center;
    gap: 14px;
    width: 100%;
    padding: 14px 16px;
    text-align: left;
  }
  .me.col {
    flex-direction: column;
    align-items: stretch;
    gap: 14px;
  }
  .avatar img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .avatar {
    overflow: hidden;
    width: 60px;
    height: 60px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    flex: none;
    font-family: var(--font-round);
    font-size: 26px;
    font-weight: 700;
    color: var(--accent-ink);
    background: linear-gradient(140deg, var(--ambient-1), var(--ambient-3));
    box-shadow: inset 0 1px 0 rgb(var(--hi) / 0.35);
  }
  .avatar.empty {
    background: var(--fill-3);
    color: var(--label-2);
  }
  .who {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .who span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .me :global(.chev) {
    color: var(--label-3);
  }
  .spin {
    display: inline-grid;
    animation: spin 0.9s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 40px;
    padding: 0 12px;
    border-radius: var(--r-pill);
    background: var(--fill-3);
    color: var(--label-2);
  }
  .search input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: none;
    background: none;
    color: var(--label);
    font-size: var(--fs-body);
  }
  .search input::placeholder {
    color: var(--label-2);
  }
  .clear {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--label-3);
    color: var(--bg);
  }
  .none {
    text-align: center;
    margin: 30px 0;
  }
  .ver {
    text-align: center;
    margin: 0;
  }
  .sheet-signin {
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding-top: 4px;
  }
  .sheet-signin p {
    margin: 0;
    text-align: center;
  }
</style>
