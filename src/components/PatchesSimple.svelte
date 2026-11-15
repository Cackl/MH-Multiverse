<script lang="ts">
  import { onDestroy } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { openPath } from '@tauri-apps/plugin-opener'
  import { appConfig, serverRunning, setUnsaved } from '../lib/store'
  import { tip } from '../lib/tooltip'
  import PanelSidebar from './PanelSidebar.svelte'
  import type { PrototypeMatch } from '../lib/prototypes'
  import { patchPack, STOCK_PACKS, TWEAKS_FILE, type PatchFileInfo, type PatchEntry, type PatchNames } from '../lib/patchesMeta'
  import AddTweakModal from './AddTweakModal.svelte'
  import {
    AVATAR_FOLDER, COOLDOWN_MIN, COOLDOWN_MAX, COOLDOWN_STEP, COOLDOWN_DEFAULT,
    clampCooldown, readCooldowns, withCooldowns, type PowerCooldown,
  } from '../lib/gameTweaks'

  type View = 'all' | 'on' | 'off'
  const VIEWS: { id: View; label: string }[] = [
    { id: 'all', label: 'All' },
    { id: 'on',  label: 'On' },
    { id: 'off', label: 'Off' },
  ]
  // Your own files come first, the stock ones after.
  const GROUPS = ['Yours', 'Stock'] as const

  let files: PatchFileInfo[] = []
  let loaded = false
  let loadError = ''
  let toggleError = ''
  let folderError = ''
  let toggling: string | null = null
  let view: View = 'all'
  /** Set by a toggle; cleared once the server has (re)started, which is when patches load. */
  let pendingRestart = false
  let sawStopped = false

  // Tweaks: the shipping heroes the cooldown recipe writes to, the launcher file's entries,
  // and the reduction saved there and being edited (null = off). The box keeps its number while off.
  let avatars: string[] = []
  let tweakEntries: PatchEntry[] = []
  let savedPct: number | null = null
  let draftPct: number | null = null
  let boxPct = COOLDOWN_DEFAULT
  // Per-power reductions, saved and being edited, and power names by id.
  let savedPowers: PowerCooldown[] = []
  let draftPowers: PowerCooldown[] = []
  let powerNames: Record<string, string> = {}
  let addOpen = false
  let saving = false
  let saveError = ''

  const same = (a: PowerCooldown, b: PowerCooldown) => a.id === b.id && a.pct === b.pct
  $: changeCount = Number(draftPct !== savedPct)
    + draftPowers.filter(p => !savedPowers.some(s => same(s, p))).length
    + savedPowers.filter(s => !draftPowers.some(p => p.id === s.id)).length
  $: dirty = changeCount > 0
  $: setUnsaved('patches', dirty ? 'Patches' : null)
  onDestroy(() => setUnsaved('patches', null))
  /** The launcher file, preferring the copy the server loads. */
  $: tweaksFile = files.find(f => f.file_name === TWEAKS_FILE && f.enabled) ?? files.find(f => f.file_name === TWEAKS_FILE)

  $: serverExe = $appConfig.server_exe
  $: if (serverExe) load(serverExe)

  $: if (!$serverRunning) sawStopped = true
  $: if ($serverRunning && sawStopped && pendingRestart) pendingRestart = false

  async function load(exe: string) {
    loadError = ''
    try {
      files = await invoke<PatchFileInfo[]>('scan_patch_files', { serverExe: exe })
      await loadTweaks(exe)
      loaded = true
    } catch (e) {
      loadError = String(e)
    }
  }

  async function loadTweaks(exe: string) {
    saveError = ''
    try {
      const heroes = await invoke<PrototypeMatch[]>('search_prototypes', {
        serverExe: exe, query: '', categoryPath: AVATAR_FOLDER, classFilter: 'AvatarPrototype',
      })
      avatars = heroes.map(h => h.path)
    } catch {
      avatars = [] // no Calligraphy.sip: the Tweaks group stays hidden
    }
    const owned = files.find(f => f.file_name === TWEAKS_FILE && f.enabled) ?? files.find(f => f.file_name === TWEAKS_FILE)
    try {
      tweakEntries = owned
        ? await invoke<PatchEntry[]>('load_patch_file', { serverExe: exe, fileName: TWEAKS_FILE, enabled: owned.enabled })
        : []
    } catch (e) {
      // Saving would overwrite a file we couldn't read, so hide the Tweaks group instead.
      avatars = []
      saveError = String(e)
      return
    }
    const saved = readCooldowns(tweakEntries)
    savedPct = saved.all
    draftPct = savedPct
    boxPct = savedPct ?? COOLDOWN_DEFAULT
    savedPowers = saved.powers
    draftPowers = saved.powers
    if (saved.powers.length) {
      try {
        const names = await invoke<PatchNames>('name_patch_targets', { serverExe: exe, paths: [], ids: saved.powers.map(p => p.id) })
        powerNames = { ...powerNames, ...names.ids }
      } catch { /* rows fall back to the id */ }
    }
  }

  function addPower(power: PowerCooldown, name: string) {
    powerNames = { ...powerNames, [power.id]: name }
    draftPowers = [...draftPowers.filter(p => p.id !== power.id), power]
    addOpen = false
  }

  function setPowerPct(id: string, pct: number) {
    draftPowers = draftPowers.map(p => p.id === id ? { ...p, pct } : p)
  }

  async function saveTweaks() {
    if (!serverExe) return
    saving = true
    saveError = ''
    try {
      await invoke('save_patch_file', {
        serverExe,
        fileName: TWEAKS_FILE,
        enabled: tweaksFile?.enabled ?? true,
        entries: withCooldowns(tweakEntries, avatars, { all: draftPct, powers: draftPowers }),
      })
      pendingRestart = true
      sawStopped = !$serverRunning
      await load(serverExe)
    } catch (e) {
      saveError = String(e)
    } finally {
      saving = false
    }
  }

  // A file can sit in both Patches and Patches/Off after an update, so the key includes the folder.
  const keyOf = (f: PatchFileInfo) => `${f.enabled ? 'on' : 'off'}/${f.file_name}`

  $: rows = files
    .filter(f => view === 'all' || f.enabled === (view === 'on'))
    .map(f => ({ file: f, key: keyOf(f), pack: patchPack(f.file_name), stock: f.file_name in STOCK_PACKS }))
    .sort((a, b) => a.pack.name.localeCompare(b.pack.name))
  // Filtered on the saved state so switching it doesn't hide the row before Save.
  // Power rows have no switch, so they count as on.
  $: tweaksReady = !!serverExe && loaded && avatars.length > 0
  $: showGlobal = tweaksReady && (view === 'all' || (savedPct !== null) === (view === 'on'))
  $: showPowers = tweaksReady && view !== 'off' && draftPowers.length > 0
  $: showTweaks = showGlobal || showPowers

  async function toggle(f: PatchFileInfo) {
    const key = keyOf(f)
    toggling = key
    toggleError = ''
    try {
      const enabled = await invoke<boolean>('set_patch_file_enabled', {
        serverExe,
        fileName: f.file_name,
        currentlyEnabled: f.enabled,
      })
      files = files.map(x => keyOf(x) === key ? { ...x, enabled } : x)
      pendingRestart = true
      sawStopped = !$serverRunning
    } catch (e) {
      toggleError = String(e)
    } finally {
      toggling = null
    }
  }

  async function openFolder() {
    folderError = ''
    try {
      await openPath(await invoke<string>('get_patches_dir', { serverExe }))
    } catch (e) {
      folderError = String(e)
    }
  }
</script>

<div class="patches">
  <div class="panel-bg"></div>
  <div class="grid-overlay"></div>
  <div class="patches-layout">
    <PanelSidebar width="var(--sidebar-narrow)">
      <svelte:fragment slot="header">
        <div class="section-title">Show</div>
        <button
          class="btn-icon"
          style="margin-left:auto;"
          on:click={openFolder}
          disabled={!serverExe}
          aria-label="Open Patches folder"
          use:tip={'Open Patches folder'}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
          </svg>
        </button>
      </svelte:fragment>

      <div class="patches-side">
        {#each VIEWS as v}
          <button class="side-item" class:active={view === v.id} on:click={() => view = v.id}><span class="side-name">{v.label}</span></button>
        {/each}
      </div>
    </PanelSidebar>

    <section class="data-main">
      <header class="page-head">
        <h3 class="page-title">Patches</h3>
        <div class="page-acts">
          <span class="restart-note">Patches load when the server starts</span>
          {#if tweaksReady}
            <button class="btn btn-sm btn-outline" on:click={() => addOpen = true} use:tip={'Add a power cooldown'}>Add tweak</button>
          {/if}
        </div>
      </header>

      <div class="patches-body">
        {#if showTweaks}
          <div class="subsection-title patches-group">Tweaks</div>
          {#if showGlobal}
          <div class="pack-row" class:off={draftPct === null}>
            <span class="pack-name"><span class="pack-title">All heroes' cooldowns</span></span>
            <span class="pack-desc">Shortens every power cooldown a hero uses</span>
            <label class="pct-field">
              <span class="pct-label">Reduce by</span>
              <span class="pct-box">
                <input
                  type="number"
                  min={COOLDOWN_MIN}
                  max={COOLDOWN_MAX}
                  step={COOLDOWN_STEP}
                  value={boxPct}
                  disabled={draftPct === null}
                  on:change={(e) => {
                    boxPct = clampCooldown(e.currentTarget.valueAsNumber)
                    e.currentTarget.value = String(boxPct) // show the clamped value
                    draftPct = boxPct
                  }}
                >
                <span class="pct-unit">%</span>
              </span>
            </label>
            <button
              class="toggle-switch"
              class:on={draftPct !== null}
              role="switch"
              aria-checked={draftPct !== null}
              aria-label="All heroes' cooldowns"
              on:click={() => draftPct = draftPct === null ? boxPct : null}
            ></button>
          </div>
          {/if}
          {#if showPowers}
          {#each draftPowers as p (p.id)}
            {@const name = powerNames[p.id] ?? p.id}
            <div class="pack-row">
              <span class="pack-name"><span class="pack-title" use:tip={name}>{name}</span></span>
              <span class="pack-desc">Shortens this power's cooldown on top of All heroes' cooldowns</span>
              <label class="pct-field">
                <span class="pct-label">Reduce by</span>
                <span class="pct-box">
                  <input
                    type="number"
                    min={COOLDOWN_MIN}
                    max={COOLDOWN_MAX}
                    step={COOLDOWN_STEP}
                    value={p.pct}
                    on:change={(e) => {
                      const pct = clampCooldown(e.currentTarget.valueAsNumber)
                      e.currentTarget.value = String(pct)
                      setPowerPct(p.id, pct)
                    }}
                  >
                  <span class="pct-unit">%</span>
                </span>
              </label>
              <button
                class="del-btn"
                aria-label="Remove {name}"
                use:tip={'Remove this tweak'}
                on:click={() => draftPowers = draftPowers.filter(x => x.id !== p.id)}
              >
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                  <line x1="18" y1="6" x2="6" y2="18"/>
                  <line x1="6" y1="6" x2="18" y2="18"/>
                </svg>
              </button>
            </div>
          {/each}
          {/if}
        {/if}

        {#if !serverExe}
          <p class="patches-msg">Set the server location first.</p>
        {:else if loadError}
          <p class="patches-msg error">{loadError}</p>
        {:else if loaded && files.length === 0}
          <p class="patches-msg">This server has no patch files.</p>
        {:else if loaded && rows.length === 0 && !showTweaks}
          <p class="patches-msg">{view === 'on' ? 'No patches are on.' : 'No patches are off.'}</p>
        {:else}
          {#each GROUPS as group}
          {@const groupRows = rows.filter(r => r.stock === (group === 'Stock'))}
          {#if groupRows.length}
          <div class="subsection-title patches-group">{group}</div>
          {#each groupRows as row (row.key)}
            <div class="pack-row" class:off={!row.file.enabled}>
              <span class="pack-name">
                <span class="pack-title" use:tip={row.file.file_name}>{row.pack.name}</span>
              </span>
              <span class="pack-desc">{row.pack.description}</span>
              <button
                class="toggle-switch"
                class:on={row.file.enabled}
                role="switch"
                aria-checked={row.file.enabled}
                aria-label={row.pack.name}
                disabled={toggling === row.key}
                on:click={() => toggle(row.file)}
              ></button>
            </div>
          {/each}
          {/if}
          {/each}
        {/if}
      </div>

      <footer class="panel-footer">
        <div class="patches-status">
          {#if saveError}<span class="error">{saveError}</span>
          {:else if toggleError}<span class="error">{toggleError}</span>
          {:else if folderError}<span class="error">{folderError}</span>
          {:else if dirty}<span class="dirty-badge">{changeCount} unsaved change{changeCount === 1 ? '' : 's'}</span>
          {:else if pendingRestart}<span>{$serverRunning ? 'Takes effect after a server restart.' : 'Applies next time the server starts.'}</span>{/if}
        </div>
        {#if avatars.length}
          <button
            class="btn btn-sm btn-accent"
            on:click={saveTweaks}
            use:tip={`Saves to ${TWEAKS_FILE}`}
            disabled={saving || !dirty}
          >{saving ? 'Saving...' : 'Save'}</button>
        {/if}
      </footer>
    </section>
  </div>
</div>

{#if addOpen && serverExe}
  <AddTweakModal {serverExe} onAdd={addPower} onClose={() => addOpen = false} />
{/if}

<style>
  .patches {
    display: flex;
    flex: 1;
    position: relative;
    overflow: hidden;
    min-height: 0;
  }
  .patches-layout {
    position: relative;
    z-index: 1;
    flex: 1;
    display: flex;
    min-height: 0;
  }
  .patches-side { padding: 6px; }
  .patches-body {
    flex: 1;
    overflow-y: auto;
    padding: 4px 20px 22px;
  }
  .patches-msg { font-size: 12px; color: var(--text-2); }
  .error { font-size: 12px; color: var(--text-error); }
  .patches-status {
    flex: 1;
    min-width: 0;
    min-height: 26px;
    display: flex;
    align-items: center;
    font-size: 12px;
    color: var(--text-1);
  }
  .patches-group { margin: 14px 0 0; }
  .restart-note { font-size: 12px; color: var(--text-2); white-space: nowrap; }

  /* Name, what it does, switch. */
  .pack-row {
    display: grid;
    grid-template-columns: minmax(0, 0.7fr) minmax(0, 1.6fr) auto 36px;
    column-gap: 16px;
    align-items: center;
    padding: 10px 0;
    border-bottom: 1px solid var(--border);
  }
  .pack-name {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
  }
  .pack-title {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-head);
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.02em;
    color: var(--text-1);
  }
  .pack-desc { font-size: 12px; color: var(--text-2); }
  .pack-row > .toggle-switch,
  .pack-row > .del-btn { grid-column: 4; justify-self: center; }

  .pct-field { display: flex; align-items: center; gap: 8px; }
  .pct-label { font-size: 12px; color: var(--text-2); white-space: nowrap; }
  .pct-box {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    width: 72px;
    height: 30px;
    padding: 0 8px 0 4px;
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    background: var(--bg-0);
  }
  .pct-box:focus-within { border-color: var(--accent-dim); box-shadow: 0 0 0 2px var(--accent-glow); }
  .pct-box input {
    flex: 1;
    min-width: 0;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--text-0);
    font-family: var(--font-body);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    text-align: right;
    outline: none;
  }
  .pct-box input:disabled { opacity: var(--dimmed-opacity); }
  .pct-unit { font-size: 12px; color: var(--text-3); }
  .pack-row.off .pack-title,
  .pack-row.off .pack-desc { opacity: var(--dimmed-opacity); }
</style>
