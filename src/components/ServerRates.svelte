<script lang="ts">
  import { onDestroy } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { openPath } from '@tauri-apps/plugin-opener'
  import { appConfig, serverRunning } from '../lib/store'
  import { tip } from '../lib/tooltip'
  import PanelSidebar from './PanelSidebar.svelte'
  import {
    SERVER_RATES_FILE, KNOWN_CORE, tuningDisplayName,
    rateRows, resolveRate, formatRate, toShown, fromShown, planServerRatesWrite, reloadResult,
    type GlobalTuning, type RateRow, type RateState, type RateGroup, type TuningEntry, type ReloadResult,
  } from '../lib/tuningMeta'

  const GROUPS: RateGroup[] = ['XP', 'Loot', 'Vendors', 'Rules']

  let tuning: GlobalTuning | null = null
  let loadError = ''
  /** Rows the user has touched since the last save: setting → value. */
  let edits: Record<string, number> = {}
  let saving = false
  let saveMsg = ''
  let saveError = ''

  $: serverExe = $appConfig.server_exe
  $: if (serverExe) load(serverExe)

  async function load(exe: string) {
    loadError = ''
    try {
      tuning = await invoke<GlobalTuning>('read_global_tuning', { serverExe: exe })
    } catch (e) {
      loadError = String(e)
    }
  }

  $: rows = tuning ? rateRows(tuning.infinity_system) : []
  $: states = Object.fromEntries(rows.map(r => [r.setting, resolveRate(tuning!, r)])) as Record<string, RateState>
  $: ownedError = tuning?.files.find(f => f.file === SERVER_RATES_FILE)?.error ?? null
  $: current = (setting: string): number => edits[setting] ?? states[setting]?.value ?? 1
  $: dirty = Object.keys(edits).filter(s => edits[s] !== states[s]?.value)
  /** Base files (in load order) that set any shown row, plus the file this form saves to. */
  $: sourceFiles = (tuning?.files ?? []).flatMap((f, i) =>
    f.file === SERVER_RATES_FILE || f.values.some(v => rows.some(r => r.setting === v.setting))
      ? [{ file: f.file, order: String(i + 1).padStart(2, '0') }]
      : [])

  let folderError = ''
  async function openFolder() {
    folderError = ''
    try {
      await openPath(await invoke<string>('get_live_tuning_dir', { serverExe }))
    } catch (e) {
      folderError = String(e)
    }
  }

  function setValue(row: RateRow, value: number) {
    edits = { ...edits, [row.setting]: value }
    saveMsg = ''
  }

  function badgeText(state: RateState): string {
    switch (state.badge.kind) {
      case 'yours':        return 'Yours'
      case 'setIn':        return `Set in ${tuningDisplayName(state.badge.file)}`
      case 'overriddenBy': return `Overridden by ${tuningDisplayName(state.badge.file)}`
      default:             return 'Default'
    }
  }

  async function save() {
    if (!serverExe || !tuning) return
    saving = true
    saveError = ''
    try {
      let existing: TuningEntry[] = []
      let exists = true
      try {
        existing = await invoke<TuningEntry[]>('read_tuning_file', { serverExe, relativePath: SERVER_RATES_FILE })
      } catch (e) {
        // Anything but a missing file (a parse error, say) stops the save, so the file is never overwritten.
        if (!String(e).startsWith('File not found')) throw e
        exists = false
      }
      const changes = dirty.map(setting => ({ setting, value: edits[setting], below: states[setting].below }))
      const entries = planServerRatesWrite(existing, changes)
      if (exists) {
        await invoke('write_tuning_file', { serverExe, relativePath: SERVER_RATES_FILE, entries })
      } else if (entries.length > 0) {
        await invoke('create_tuning_file', { serverExe, relativePath: SERVER_RATES_FILE, entries })
      }
      edits = {}
      stopWatching()
      reloadMsg = ''
      reloadError = ''
      saveMsg = 'Saved'
      await load(serverExe)
    } catch (e) {
      saveError = String(e)
    } finally {
      saving = false
    }
  }

  // ── Reload ────────────────────────────────────────────────────────────────
  // send_command only proves the text reached the server console, so the result comes from
  // the log lines that follow it.

  let reloadMsg = ''
  let reloadError = ''
  let unlisten: UnlistenFn | null = null
  let timer: ReturnType<typeof setTimeout> | null = null

  function stopWatching() {
    unlisten?.()
    unlisten = null
    if (timer) clearTimeout(timer)
    timer = null
  }

  function finish(result: ReloadResult) {
    stopWatching()
    if (result.rejected === 0) reloadMsg = `Reloaded: ${result.count} settings live`
    else showReloadError(`Reloaded with ${result.rejected} rejected ${result.rejected === 1 ? 'entry' : 'entries'}. See the Server log.`)
  }

  function showReloadError(message: string) {
    reloadMsg = ''
    reloadError = message
  }

  async function reload() {
    stopWatching()
    reloadError = ''
    const lines: string[] = []
    unlisten = await listen<{ msg: string }[]>('server-log', e => {
      lines.push(...e.payload.map(l => l.msg))
      const result = reloadResult(lines)
      if (result) finish(result)
    })
    timer = setTimeout(() => {
      stopWatching()
      showReloadError('No reply from the server. See the Server log.')
    }, 5000)
    reloadMsg = 'Reload sent'
    try {
      await invoke('send_command', { cmd: '!server reloadlivetuning' })
    } catch (e) {
      stopWatching()
      showReloadError(`Reload not sent: ${e}`)
    }
  }

  onDestroy(stopWatching)

  const pct = (row: RateRow, v: number) => Math.max(0, Math.min(100, (v - row.min) / (row.max - row.min) * 100))
  const jumpTo = (group: RateGroup) => document.getElementById(`rates-${group}`)?.scrollIntoView({ block: 'start' })
</script>

<div class="rates">
  <div class="panel-bg"></div>
  <div class="grid-overlay"></div>
  <div class="rates-layout">
    <PanelSidebar width="var(--sidebar-narrow)">
      <svelte:fragment slot="header">
        <div class="section-title">Sections</div>
        <button
          class="btn-icon"
          style="margin-left:auto;"
          on:click={openFolder}
          disabled={!serverExe}
          aria-label="Open LiveTuning folder"
          use:tip={'Open LiveTuning folder'}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
          </svg>
        </button>
      </svelte:fragment>

      <div class="rates-side">
        {#each GROUPS as group}
          <button class="side-item" on:click={() => jumpTo(group)}><span class="side-name">{group}</span></button>
        {/each}
        {#if sourceFiles.length}
          <div class="side-label">Files with these settings</div>
          {#each sourceFiles as { file, order } (file)}
            <div class="side-item" use:tip={file}>
              <span class="side-mark">{order}</span>
              <span class="side-name">{tuningDisplayName(file)}</span>
              <span class="tag">{KNOWN_CORE.has(file) ? 'Core' : 'Custom'}</span>
            </div>
          {/each}
        {/if}
      </div>
    </PanelSidebar>

    <section class="data-main">
      <header class="page-head">
        <h3 class="page-title">Server Rates</h3>
        <span class="page-sub">Saves to {SERVER_RATES_FILE}</span>
        <div class="page-acts">
          {#if reloadError}<span class="error">{reloadError}</span>
          {:else if reloadMsg}<span class="rates-status">{reloadMsg}</span>{/if}
          <button
            class="btn btn-sm btn-outline"
            on:click={reload}
            disabled={!$serverRunning}
            use:tip={$serverRunning ? 'Send !server reloadlivetuning' : 'Server not running'}
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="23 4 23 10 17 10"/>
              <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/>
            </svg>
            Reload Live Tuning
          </button>
        </div>
      </header>

      <div class="rates-body">
        {#if !serverExe}
          <p class="rates-msg">Set the server location first.</p>
        {:else if loadError}
          <p class="rates-msg error">{loadError}</p>
        {:else if tuning}
          {#if ownedError}
            <p class="rates-msg error">Can't read {SERVER_RATES_FILE}: {ownedError}</p>
          {/if}

          {#each GROUPS as group}
            <div class="subsection-title rates-group" id="rates-{group}">{group}</div>
            {#each rows.filter(r => r.group === group) as row (row.setting)}
              {@const state = states[row.setting]}
              {@const value = current(row.setting)}
              {@const locked = state.badge.kind === 'overriddenBy'}
              {@const changed = dirty.includes(row.setting)}
              <div class="rate-row" class:locked>
                <span class="rate-name">
                  {row.label}
                  {#if row.tip}<button class="info-btn" aria-label="About {row.label}" use:tip={row.tip}>?</button>{/if}
                </span>

                {#if changed}
                  <span class="rate-badge changed">Changed</span>
                {:else if 'file' in state.badge}
                  <span class="rate-badge {state.badge.kind}" use:tip={state.badge.file}>{badgeText(state)}</span>
                {:else}
                  <span class="rate-badge {state.badge.kind}">{badgeText(state)}</span>
                {/if}

                <div class="rate-control">
                  {#if row.format === 'toggle'}
                    <div
                      class="toggle-switch"
                      class:on={value !== 0}
                      role="switch"
                      aria-checked={value !== 0}
                      aria-label={row.label}
                      aria-disabled={locked}
                      tabindex={locked ? -1 : 0}
                      on:click={() => !locked && setValue(row, value !== 0 ? 0 : 1)}
                      on:keydown={(e) => !locked && e.key === 'Enter' && setValue(row, value !== 0 ? 0 : 1)}
                    ></div>
                    <span class="rate-onoff">{value !== 0 ? 'On' : 'Off'}</span>
                  {:else}
                    <span class="rate-track" style="--stock:{pct(row, state.below)}">
                      <input
                        type="range"
                        aria-label={row.label}
                        min={row.min}
                        max={row.max}
                        step={row.step}
                        value={Math.min(row.max, Math.max(row.min, value))}
                        style="--fill:{pct(row, value)}%"
                        disabled={locked}
                        on:input={(e) => setValue(row, Number(e.currentTarget.value))}
                      >
                    </span>
                    <span class="rate-box">
                      {#if row.format === 'bonus'}<span class="rate-unit">+</span>{/if}
                      <input
                        type="number"
                        aria-label="{row.label} value"
                        min={toShown(row, row.min)}
                        max={toShown(row, row.limit)}
                        step="any"
                        value={toShown(row, value)}
                        disabled={locked}
                        on:change={(e) => {
                          const box = e.currentTarget
                          if (Number.isFinite(box.valueAsNumber)) setValue(row, fromShown(row, box.valueAsNumber))
                          box.value = String(toShown(row, current(row.setting))) // show the clamped value
                        }}
                      >
                      <span class="rate-unit">{row.format === 'times' ? '×' : '%'}</span>
                    </span>
                  {/if}
                </div>

                <span class="rate-revert">
                  {#if !locked && value !== state.below}
                    <button
                      class="btn-icon"
                      on:click={() => setValue(row, state.below)}
                      aria-label="Revert {row.label}"
                      use:tip={`Revert to ${formatRate(row, state.below)}`}
                    >
                      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <path d="M3 12a9 9 0 1 0 3-6.7"/>
                        <polyline points="3 3 3 9 9 9"/>
                      </svg>
                    </button>
                  {/if}
                </span>

                {#each state.notes as note}<span class="rate-note">{note}</span>{/each}
              </div>
            {/each}
          {/each}
        {/if}
      </div>

      <footer class="panel-footer">
        <div class="rates-status">
          {#if saveError}<span class="error">{saveError}</span>
          {:else if folderError}<span class="error">{folderError}</span>
          {:else if dirty.length}<span class="dirty-badge">{dirty.length} unsaved {dirty.length === 1 ? 'change' : 'changes'}</span>
          {:else if saveMsg}<span>{saveMsg}</span>{/if}
        </div>
        <button
          class="btn btn-sm btn-accent"
          on:click={save}
          disabled={saving || dirty.length === 0 || !!ownedError}
        >{saving ? 'Saving...' : 'Save'}</button>
      </footer>
    </section>
  </div>
</div>

<style>
  .rates {
    display: flex;
    flex: 1;
    position: relative;
    overflow: hidden;
    min-height: 0;
  }
  .rates-layout {
    position: relative;
    z-index: 1;
    flex: 1;
    display: flex;
    min-height: 0;
  }
  .rates-side { padding: 6px; }
  .rates-body {
    flex: 1;
    overflow-y: auto;
    padding: 4px 20px 22px;
  }
  .rates-msg { font-size: 12px; color: var(--text-2); }
  .error { font-size: 12px; color: var(--text-error); }
  .rates-status {
    flex: 1;
    min-width: 0;
    font-size: 12px;
    color: var(--text-1);
  }
  .page-acts .rates-status { flex: none; }

  .rates-group { margin: 14px 0 0; scroll-margin-top: 4px; }

  /* Name over badge on the left, the control in the middle, revert on the right, notes below. */
  .rate-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.3fr) 28px;
    column-gap: 18px;
    row-gap: 3px;
    align-items: center;
    padding: 8px 0;
    border-bottom: 1px solid var(--border);
  }
  .rate-name {
    grid-column: 1;
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
    font-family: var(--font-head);
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.02em;
    color: var(--text-1);
  }
  .rate-badge {
    grid-column: 1;
    font-family: var(--font-head);
    font-size: var(--fs-min);
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .rate-badge.yours { color: var(--accent-bright); }
  .rate-badge.setIn,
  .rate-badge.overriddenBy { color: var(--text-2); }
  .rate-badge.changed { color: var(--amber-bright); }
  .rate-control {
    grid-column: 2;
    grid-row: 1 / span 2;
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
  }
  .rate-revert {
    grid-column: 3;
    grid-row: 1 / span 2;
    display: flex;
    justify-content: center;
  }
  .rate-note {
    grid-column: 1 / -1;
    font-size: 11.5px;
    line-height: 1.45;
    color: var(--text-2);
  }
  .rate-row.locked .rate-name,
  .rate-row.locked .rate-control { opacity: var(--dimmed-opacity); }
  .rate-onoff { font-size: 12px; color: var(--text-1); }

  /* Slider with a mark at the value it has without this form. */
  .rate-track {
    position: relative;
    flex: 1;
    min-width: 80px;
    height: 22px;
    display: flex;
    align-items: center;
  }
  .rate-track::before {
    content: '';
    position: absolute;
    left: calc(7px + (100% - 14px) * var(--stock) / 100);
    top: 4px;
    bottom: 4px;
    width: 2px;
    margin-left: -1px;
    border-radius: 1px;
    background: var(--text-3);
    pointer-events: none;
  }
  .rate-track input {
    position: relative;
    z-index: 1;
    width: 100%;
    margin: 0;
    height: 4px;
    border-radius: 2px;
    -webkit-appearance: none;
    appearance: none;
    background: linear-gradient(90deg, var(--accent-dim) var(--fill), var(--bg-4) var(--fill));
    cursor: pointer;
  }
  .rate-track input::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--accent-bright);
    border: 2px solid var(--bg-1);
    box-shadow: 0 0 0 1px var(--accent-dim);
  }
  .rate-track input:disabled { cursor: not-allowed; }
  .rate-track input:focus-visible { outline: 2px solid var(--accent); outline-offset: 4px; }

  .rate-box {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    flex-shrink: 0;
    width: 92px;
    height: 30px;
    padding: 0 8px 0 4px;
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    background: var(--bg-0);
  }
  .rate-box:focus-within { border-color: var(--accent-dim); box-shadow: 0 0 0 2px var(--accent-glow); }
  .rate-box input {
    flex: 1;
    min-width: 0;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--text-0);
    font-family: var(--font-mono);
    font-size: 12px;
    text-align: right;
    outline: none;
  }
  .rate-unit { font-size: 12px; color: var(--text-3); }
</style>
