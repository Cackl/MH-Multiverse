<script lang="ts">
  import { onDestroy } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { openPath } from '@tauri-apps/plugin-opener'
  import { activeCaps, appConfig, serverRunning, setUnsaved } from '../../../lib/store'
  import { tip } from '../../../lib/tooltip'
  import PanelSidebar from '../../PanelSidebar.svelte'
  import type { PrototypeMatch } from '../../../lib/prototypes'
  import { patchPack, STOCK_PACKS, TWEAKS_FILE, type PatchFileInfo, type PatchEntry, type PatchNames } from '../../../lib/patchesMeta'
  import AddTweakModal, { type NewTweak } from './AddTweakModal.svelte'
  import {
    AVATAR_FOLDER, COOLDOWN_MIN, COOLDOWN_MAX, COOLDOWN_STEP, COOLDOWN_DEFAULT,
    clampCooldown, readCooldowns, withCooldowns, type PowerCooldown,
    SUMMON_MIN, SUMMON_MAX, clampSummon, readSummonCaps, withSummonCaps, type SummonCap,
  } from '../../../lib/gameTweaks'

  // Patches by where they come from: the launcher's own tweaks, the server's stock packs, your files.
  type View = 'all' | 'multiverse' | 'stock' | 'yours'
  type Group = 'stock' | 'yours'
  const GROUP_LABELS: Record<Group, string> = { stock: 'Stock', yours: 'Yours' }

  let files: PatchFileInfo[] = []
  let loaded = false
  let loadError = ''
  let toggleError = ''
  let keptNote = ''
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
  // Summon caps, saved and being edited, and power names by path.
  let savedSummons: SummonCap[] = []
  let draftSummons: SummonCap[] = []
  let summonNames: Record<string, string> = {}
  let addOpen = false
  let saving = false
  let saveError = ''

  /** Rows added or changed, plus rows removed, matching rows by `key`. */
  function changes<T>(saved: T[], draft: T[], key: (x: T) => string): number {
    const json = (x: T) => JSON.stringify(x)
    return draft.filter(d => !saved.some(s => json(s) === json(d))).length
      + saved.filter(s => !draft.some(d => key(d) === key(s))).length
  }
  $: changeCount = Number(draftPct !== savedPct)
    + changes(savedPowers, draftPowers, p => p.id)
    + changes(savedSummons, draftSummons, s => s.path)
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
    savedSummons = readSummonCaps(tweakEntries)
    draftSummons = savedSummons
    if (saved.powers.length || savedSummons.length) {
      try {
        const names = await invoke<PatchNames>('name_patch_targets', {
          serverExe: exe, paths: savedSummons.map(s => s.path), ids: saved.powers.map(p => p.id),
        })
        powerNames = { ...powerNames, ...names.ids }
        summonNames = { ...summonNames, ...names.paths }
      } catch { /* rows fall back to the id or path */ }
    }
  }

  // Adding a power that is already listed updates its row.
  function addTweak(t: NewTweak) {
    if (t.kind === 'cooldown') {
      powerNames = { ...powerNames, [t.power.id]: t.name }
      draftPowers = [...draftPowers.filter(p => p.id !== t.power.id), t.power]
    } else {
      summonNames = { ...summonNames, [t.cap.path]: t.name }
      draftSummons = [...draftSummons.filter(s => s.path !== t.cap.path), t.cap]
    }
    addOpen = false
  }

  function setSummonMax(path: string, max: number) {
    draftSummons = draftSummons.map(s => s.path === path ? { ...s, max } : s)
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
        entries: withSummonCaps(withCooldowns(tweakEntries, avatars, { all: draftPct, powers: draftPowers }), draftSummons),
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

  // The tweaks file is shown as the Multiverse page's switch, not as a row.
  $: allRows = files
    .filter(f => f.file_name !== TWEAKS_FILE)
    .map(f => ({ file: f, key: keyOf(f), pack: patchPack(f.file_name, $activeCaps.patchLabels), group: (f.file_name in STOCK_PACKS ? 'stock' : 'yours') as Group }))
    .sort((a, b) => a.pack.name.localeCompare(b.pack.name))
  $: groups = (['stock', 'yours'] as Group[]).filter(g => (view === 'all' || view === g) && allRows.some(r => r.group === g))

  $: tweaksReady = !!serverExe && loaded && avatars.length > 0
  $: tweaksOn = tweaksFile?.enabled ?? true
  $: anyTweak = draftPct !== null || draftPowers.length > 0 || draftSummons.length > 0
  $: views = [
    { id: 'all', label: 'All' },
    ...(tweaksReady ? [{ id: 'multiverse', label: 'Multiverse' }] : []),
    { id: 'stock', label: 'Stock' },
    ...(allRows.some(r => r.group === 'yours') ? [{ id: 'yours', label: 'Yours' }] : []),
  ] as { id: View; label: string }[]
  $: if (loaded && !views.some(v => v.id === view)) view = 'all'
  // All lists the tweaks read-only, so they are edited in one place.
  $: showTweaks = tweaksReady && (view === 'multiverse' || (view === 'all' && anyTweak))
  $: readOnly = view !== 'multiverse'

  async function toggle(f: PatchFileInfo) {
    const key = keyOf(f)
    toggling = key
    toggleError = ''
    try {
      const { enabled, kept_as } = await invoke<{ enabled: boolean; kept_as: string | null }>('set_patch_file_enabled', {
        serverExe,
        fileName: f.file_name,
        currentlyEnabled: f.enabled,
      })
      keptNote = kept_as ? `The other copy was kept as ${kept_as}` : ''
      // With a copy in both folders, the switched one replaced the other.
      files = files
        .filter(x => x.file_name !== f.file_name || keyOf(x) === key)
        .map(x => keyOf(x) === key ? { ...x, enabled } : x)
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
        {#each views as v}
          <button class="side-item" class:active={view === v.id} on:click={() => view = v.id}><span class="side-name">{v.label}</span></button>
        {/each}
      </div>
    </PanelSidebar>

    <section class="data-main">
      <header class="page-head">
        <h3 class="page-title">{view === 'multiverse' ? 'Multiverse tweaks' : 'Patches'}</h3>
        <div class="page-acts">
          <span class="restart-note">Patches load when the server starts</span>
          {#if view === 'multiverse'}
            <button class="btn btn-sm btn-outline" on:click={() => addOpen = true} use:tip={'Add a power cooldown or summon cap'}>New Entry</button>
            {#if tweaksFile}
              <span class="file-switch">
                <span class="pct-label">All tweaks</span>
                <button
                  class="toggle-switch"
                  class:on={tweaksFile.enabled}
                  role="switch"
                  aria-checked={tweaksFile.enabled}
                  aria-label="All Multiverse tweaks"
                  use:tip={tweaksFile.enabled ? 'Turn every Multiverse tweak off and keep its settings' : 'Turn every Multiverse tweak back on'}
                  disabled={toggling === keyOf(tweaksFile)}
                  on:click={() => tweaksFile && toggle(tweaksFile)}
                ></button>
              </span>
            {/if}
          {/if}
        </div>
      </header>

      <div class="patches-body">
        <div class="content-col">
        {#if showTweaks}
          {#if view === 'all'}<div class="subsection-title patches-group">Multiverse</div>{/if}
          {#if !readOnly || draftPct !== null}
          <div class="pack-row" class:off={draftPct === null || !tweaksOn}>
            <span class="pack-name"><span class="pack-title">All heroes' cooldowns</span></span>
            <span class="pack-desc">Shortens every power cooldown a hero uses</span>
            {#if readOnly}
              <span class="pct-read">-{draftPct}%</span>
            {:else}
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
            {/if}
          </div>
          {/if}
          {#each draftPowers as p (p.id)}
            {@const name = powerNames[p.id] ?? p.id}
            <div class="pack-row" class:off={!tweaksOn}>
              <span class="pack-name"><span class="pack-title" use:tip={name}>{name}</span></span>
              <span class="pack-desc">Shortens this power's cooldown on top of All heroes' cooldowns</span>
              {#if readOnly}
                <span class="pct-read">-{p.pct}%</span>
              {:else}
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
              {/if}
            </div>
          {/each}
          {#each draftSummons as s (s.path)}
            {@const name = summonNames[s.path] ?? s.path}
            <div class="pack-row" class:off={!tweaksOn}>
              <span class="pack-name"><span class="pack-title" use:tip={name}>{name}</span></span>
              <span class="pack-desc">Sets how many this power keeps out at once</span>
              {#if readOnly}
                <span class="pct-read">Up to {s.max}</span>
              {:else}
              <label class="pct-field">
                <span class="pct-label">Max at once</span>
                <span class="pct-box">
                  <input
                    type="number"
                    min={SUMMON_MIN}
                    max={SUMMON_MAX}
                    step="1"
                    value={s.max}
                    on:change={(e) => {
                      const max = clampSummon(e.currentTarget.valueAsNumber)
                      e.currentTarget.value = String(max)
                      setSummonMax(s.path, max)
                    }}
                  >
                </span>
              </label>
              <button
                class="del-btn"
                aria-label="Remove {name}"
                use:tip={'Remove this tweak'}
                on:click={() => draftSummons = draftSummons.filter(x => x.path !== s.path)}
              >
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                  <line x1="18" y1="6" x2="6" y2="18"/>
                  <line x1="6" y1="6" x2="18" y2="18"/>
                </svg>
              </button>
              {/if}
            </div>
          {/each}
        {/if}

        {#if !serverExe}
          <p class="patches-msg">Set the server location first.</p>
        {:else if loadError}
          <p class="patches-msg error">{loadError}</p>
        {:else if view !== 'multiverse'}
          {#if loaded && !groups.length && !showTweaks}
            <p class="patches-msg">{view === 'stock' ? 'This server has no stock patches.' : 'This server has no patch files.'}</p>
          {/if}
          {#each groups as group}
          {#if view === 'all'}<div class="subsection-title patches-group">{GROUP_LABELS[group]}</div>{/if}
          {#each allRows.filter(r => r.group === group) as row (row.key)}
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
          {/each}
        {/if}
        </div>
      </div>

      <footer class="panel-footer">
        <div class="patches-status">
          {#if saveError}<span class="error">{saveError}</span>
          {:else if toggleError}<span class="error">{toggleError}</span>
          {:else if folderError}<span class="error">{folderError}</span>
          {:else if keptNote}<span>{keptNote}</span>
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
  <AddTweakModal {serverExe} onAdd={addTweak} onClose={() => addOpen = false} />
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
  .pct-read { font-size: 12px; color: var(--text-1); font-variant-numeric: tabular-nums; justify-self: end; }
  .file-switch { display: flex; align-items: center; gap: 8px; }
  .pack-row.off .pack-title,
  .pack-row.off .pack-desc { opacity: var(--dimmed-opacity); }
</style>
