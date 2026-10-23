<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog'
  import { invoke } from '@tauri-apps/api/core'
  import { loadConfig } from '../../lib/store'
  import IntegrationCard from './IntegrationCard.svelte'

  interface Candidate {
    name: string
    host: string
    use_https: boolean
    email: string
    has_password: boolean
    source: string
    duplicate: boolean
  }
  interface Preview { profiles: Candidate[]; rejected: string[] }

  let paths: string[] = []
  let preview: Preview | null = null
  let includeCreds = true
  let matchBy = 'host_email'
  // Per-row choices that beat the match rule, keyed by row position (stable for the same files).
  let overrides: Record<number, boolean> = {}
  let error = ''
  let notice = ''
  let busy = false

  const fileName = (p: string) => p.split(/[\\/]/).pop() ?? p

  $: withLogin = preview?.profiles.filter(p => p.has_password).length ?? 0
  $: chosen = preview ? preview.profiles.map((p, i) => overrides[i] ?? !p.duplicate) : []
  $: count = chosen.filter(Boolean).length

  function status(p: Candidate, on: boolean) {
    if (p.duplicate) return on ? 'Duplicate, included' : 'Duplicate, skipped'
    return on ? 'New' : 'Skipped'
  }

  async function load(files: string[]) {
    paths = files
    overrides = {}
    includeCreds = true
    matchBy = 'host_email'
    error = ''
    notice = ''
    await refresh()
  }

  async function browse() {
    const sel = await open({ multiple: true, filters: [{ name: 'JSON', extensions: ['json'] }] })
    if (sel && sel.length) await load(Array.isArray(sel) ? sel : [sel])
  }

  async function detect() {
    const found = await invoke<string[]>('detect_profile_sources')
    if (found.length) await load(found)
    else {
      preview = null
      notice = 'No Bifrost or Overlay launcher files found. Use Browse to pick them.'
    }
  }

  // Duplicates depend on the match rule and on whether logins are imported; manual choices are kept.
  async function refresh() {
    try {
      preview = await invoke<Preview>('preview_profile_import', { paths, includeCredentials: includeCreds, matchBy })
    } catch (e) {
      preview = null
      error = String(e)
    }
  }

  function toggle(i: number, on: boolean) {
    // Drop the override when it just restates the rule, so a later rule change still applies.
    if (on === !preview!.profiles[i].duplicate) delete overrides[i]
    else overrides[i] = on
    overrides = overrides
  }

  async function doImport() {
    busy = true
    error = ''
    try {
      const include = chosen.flatMap((on, i) => (on ? [i] : []))
      const n = await invoke<number>('import_profiles', { paths, includeCredentials: includeCreds, include })
      await loadConfig()
      notice = `Imported ${n} profile${n === 1 ? '' : 's'}.`
      preview = null
    } catch (e) {
      error = String(e)
    } finally {
      busy = false
    }
  }
</script>

<IntegrationCard
  id="profile-import"
  title="Import Server Profiles"
  subtitle="From Bifrost or the MarvelHeroesOmega Overlay launcher."
>
  <div class="opt-row">
    <div class="opt-label">
      <span class="opt-name">Source files</span>
      <span class="opt-desc">
        {#if paths.length && preview}Loaded: {paths.map(fileName).join(', ')}
        {:else}Detect finds Bifrost and the Overlay launcher, or choose the files yourself.{/if}
      </span>
    </div>
    <div class="btns">
      <button class="btn btn-sm btn-outline" on:click={detect}>Detect</button>
      <button class="btn btn-sm btn-outline" on:click={browse}>Browse</button>
    </div>
  </div>

  {#if preview}
    <div class="opt-row">
      <div class="opt-label">
        <span class="opt-name">Match existing profiles by</span>
        <span class="opt-desc">Matches are skipped; click a server below to override.</span>
      </div>
      <select class="field-select" bind:value={matchBy} on:change={refresh}>
        <option value="host_email">Host + email</option>
        <option value="host_name">Host + name</option>
        <option value="host">Host only</option>
        <option value="nothing">Don't match</option>
      </select>
    </div>
    {#if withLogin}
      <label class="opt-row">
        <div class="opt-label">
          <span class="opt-name">Import saved logins ({withLogin})</span>
          <span class="opt-desc">Stored encrypted. They are plaintext in the source files.</span>
        </div>
        <input type="checkbox" bind:checked={includeCreds} on:change={refresh} />
      </label>
    {/if}

    <div class="found">
      <div class="found-head">
        <span>Servers found</span>
        <span class="count">{count} of {preview.profiles.length} selected</span>
      </div>
      {#each preview.profiles as p, i}
        <label class="srv" class:off={!chosen[i]}>
          <input type="checkbox" checked={chosen[i]} on:change={e => toggle(i, e.currentTarget.checked)} />
          <div class="srv-main">
            <span class="srv-name">{p.name}</span>
            <span class="srv-sub">
              {p.use_https ? 'https://' : ''}{p.host} · {p.source}{p.email ? ` · ${p.email}${includeCreds ? '' : ' (not imported)'}` : ''}
            </span>
          </div>
          <span class="chip" class:dup={p.duplicate}>{status(p, chosen[i])}</span>
        </label>
      {:else}
        <div class="empty">No servers found in these files.</div>
      {/each}
      {#each preview.rejected as r}<div class="empty warn">{r}</div>{/each}
      <div class="found-foot">
        <button class="btn btn-sm btn-accent" disabled={busy || !count} on:click={doImport}>
          Import {count || ''}
        </button>
      </div>
    </div>
  {/if}

  {#if notice}<span class="opt-desc">{notice}</span>{/if}
  {#if error}<span class="opt-desc err">{error}</span>{/if}
</IntegrationCard>

<style>
  .btns { display: flex; gap: 8px; }
  .err { color: var(--text-error); }
  .warn { color: var(--amber); }

  .found {
    margin-top: 10px;
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    background: var(--bg-0);
    overflow: hidden;
  }
  .found-head {
    display: flex;
    justify-content: space-between;
    padding: 6px 10px;
    background: var(--bg-3);
    border-bottom: 1px solid var(--border-mid);
    font-family: var(--font-head);
    font-size: 9px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  .found-foot {
    display: flex;
    justify-content: flex-end;
    padding: 8px 10px;
    border-top: 1px solid var(--border-mid);
    background: var(--bg-1);
  }

  .srv {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 10px;
    border-bottom: 1px solid var(--border);
    cursor: pointer;
  }
  .srv:hover { background: var(--bg-2); }
  .srv.off .srv-main { opacity: 0.5; }
  .srv-main { flex: 1; min-width: 0; display: flex; flex-direction: column; }
  .srv-name { color: var(--text-0); font-size: 12px; }
  .srv-sub {
    color: var(--text-3);
    font-family: var(--font-mono);
    font-size: 10px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .chip {
    font-family: var(--font-head);
    font-size: 9px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--green);
    white-space: nowrap;
  }
  .chip.dup { color: var(--amber); }
  .empty { padding: 8px 10px; font-size: 11px; color: var(--text-3); }
</style>
