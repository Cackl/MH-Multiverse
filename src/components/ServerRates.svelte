<script lang="ts">
  import { onDestroy } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { appConfig, serverRunning } from '../lib/store'
  import { tip } from '../lib/tooltip'
  import {
    SERVER_RATES_FILE, rateRows, resolveRate, toShown, fromShown, planServerRatesWrite, reloadResult,
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

  function setValue(row: RateRow, value: number) {
    edits = { ...edits, [row.setting]: value }
    saveMsg = ''
  }

  function badgeText(state: RateState): string {
    switch (state.badge.kind) {
      case 'yours':        return 'Yours'
      case 'setIn':        return `Set in ${state.badge.file}`
      case 'overriddenBy': return `Overridden by ${state.badge.file}`
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
</script>

<div class="rates">
  <div class="rates-head">
    <div>
      <div class="rates-title">Server Rates</div>
      <div class="rates-sub">Saves to {SERVER_RATES_FILE}</div>
    </div>
  </div>

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
        <div class="rates-group">{group}</div>
        {#each rows.filter(r => r.group === group) as row (row.setting)}
          {@const state = states[row.setting]}
          {@const value = current(row.setting)}
          {@const locked = state.badge.kind === 'overriddenBy'}
          {@const changed = dirty.includes(row.setting)}
          <div class="rate-row" class:locked>
            <span class="rate-label">
              {row.label}
              {#if row.tip}<button class="info-btn" aria-label="About {row.label}" use:tip={row.tip}>?</button>{/if}
            </span>

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
              {:else}
                <input
                  type="range"
                  class="rate-slider"
                  aria-label={row.label}
                  min={row.min}
                  max={row.max}
                  step={row.step}
                  value={Math.min(row.max, Math.max(row.min, value))}
                  disabled={locked}
                  on:input={(e) => setValue(row, Number(e.currentTarget.value))}
                >
              {/if}
            </div>

            <span class="rate-value">
              {#if row.format !== 'toggle'}
                <input
                  type="number"
                  class="rate-box"
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
              {/if}
            </span>

            <span class="rate-side">
              {#if changed}
                <span class="rate-badge changed">Changed</span>
              {:else}
                <span class="rate-badge {state.badge.kind}">{badgeText(state)}</span>
              {/if}
              {#if !locked && value !== state.below}
                <button class="rate-revert" on:click={() => setValue(row, state.below)}>Revert</button>
              {/if}
            </span>

            {#each state.notes as note}<span class="rate-note">{note}</span>{/each}
          </div>
        {/each}
      {/each}
    {/if}
  </div>

  <div class="rates-foot">
    <div class="rates-status">
      {#if saveError}<span class="error">{saveError}</span>
      {:else if reloadError}<span class="error">{reloadError}</span>
      {:else if reloadMsg}<span>{reloadMsg}</span>
      {:else if saveMsg}<span>{saveMsg}</span>{/if}
    </div>
    <button
      class="btn btn-sm btn-outline"
      on:click={reload}
      disabled={!$serverRunning}
      use:tip={$serverRunning ? 'Send !server reloadlivetuning' : 'Server not running'}
    >Reload Live Tuning</button>
    <button
      class="btn btn-sm btn-accent"
      on:click={save}
      disabled={saving || dirty.length === 0 || !!ownedError}
    >{saving ? 'Saving...' : 'Save'}</button>
  </div>
</div>

<style>
  .rates {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-height: 0;
  }

  .rates-head {
    display: flex;
    align-items: center;
    padding: 12px 20px;
    border-bottom: 1px solid var(--border);
    min-height: 53px;
    flex-shrink: 0;
  }
  .rates-title {
    font-family: var(--font-head);
    font-size: 13px;
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-0);
  }
  .rates-sub {
    font-size: 11px;
    color: var(--text-3);
    margin-top: 2px;
  }

  .rates-body {
    flex: 1;
    overflow-y: auto;
    padding: 8px 20px 16px;
  }

  .rates-msg { font-size: 12px; color: var(--text-2); }
  .error { color: var(--text-error); }

  .rates-group {
    font-family: var(--font-head);
    font-size: var(--fs-min);
    font-weight: 600;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--text-2);
    margin: 18px 0 4px;
  }

  .rate-row {
    display: grid;
    grid-template-columns: minmax(150px, 1fr) minmax(140px, 2fr) auto minmax(120px, auto);
    align-items: center;
    column-gap: 14px;
    row-gap: 2px;
    padding: 8px 0;
    border-bottom: 1px solid var(--border);
  }
  .rate-row.locked .rate-control,
  .rate-row.locked .rate-value { opacity: var(--dimmed-opacity); }

  .rate-label {
    display: flex;
    align-items: center;
    gap: 6px;
    font-family: var(--font-head);
    font-size: var(--fs-min);
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-1);
  }

  .rate-control {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
  }
  .rate-slider {
    flex: 1;
    min-width: 0;
    accent-color: var(--accent);
  }

  .rate-value {
    display: flex;
    align-items: center;
    gap: 4px;
    white-space: nowrap;
  }
  .rate-box {
    width: 64px;
    background: var(--bg-0);
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    padding: 3px 6px;
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-0);
  }
  .rate-unit {
    font-size: 11px;
    color: var(--text-2);
  }

  .rate-side {
    display: flex;
    align-items: center;
    gap: 8px;
    justify-content: flex-end;
    min-width: 0;
  }
  .rate-badge {
    font-family: var(--font-head);
    font-size: var(--fs-min);
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .rate-badge.yours,
  .rate-badge.changed { color: var(--accent-bright); }
  .rate-badge.setIn,
  .rate-badge.overriddenBy { color: var(--text-2); }

  .rate-revert {
    background: none;
    border: none;
    padding: 0;
    font-size: 11px;
    color: var(--text-2);
    text-decoration: underline;
    cursor: pointer;
  }
  .rate-revert:hover { color: var(--text-0); }

  .rate-note {
    grid-column: 1 / -1;
    font-size: 11px;
    line-height: 1.4;
    color: var(--text-3);
  }

  .rates-foot {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 20px;
    border-top: 1px solid var(--border);
    flex-shrink: 0;
  }
  .rates-status {
    flex: 1;
    min-width: 0;
    font-size: 12px;
    color: var(--text-1);
  }
</style>
