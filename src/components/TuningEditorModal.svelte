<script lang="ts">
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'

  // ── Props ──────────────────────────────────────────────────────────────────

  export let file: { canonical_name: string; enabled: boolean; toggleable: boolean }
  export let serverExe: string
  export let serverRunning: boolean
  export let onClose: () => void

  // ── Types ──────────────────────────────────────────────────────────────────

  interface TuningEntry {
    prototype: string
    setting: string
    value: number
  }

  // ── Category prefix map ────────────────────────────────────────────────────
  // Duplicated from TuningPanel — shared module is a future refactor

  const CATEGORY_PREFIXES: [string, string][] = [
    ['eGTV_',  'Global'],
    ['eWETV_', 'World Entity'],
    ['ePTV_',  'Powers'],
    ['eRTV_',  'Regions'],
    ['eRT_',   'Regions'],
    ['eLTTV_', 'Loot'],
    ['eMTV_',  'Mission'],
    ['eCTV_',  'Condition'],
    ['eAETV_', 'Avatar Entity'],
    ['eATV_',  'Area'],
    ['ePOTV_', 'Population Object'],
    ['eMFTV_', 'Metrics Frequency'],
    ['ePETV_', 'Public Events'],
  ]

  function categoryForSetting(setting: string): string {
    for (const [prefix, label] of CATEGORY_PREFIXES) {
      if (setting.startsWith(prefix)) return label
    }
    return 'Other'
  }

  // ── State ──────────────────────────────────────────────────────────────────

  let entries: TuningEntry[] = []
  let savedEntries: TuningEntry[] = []
  let loadingEntries = false
  let entryError = ''

  let saving = false
  let saveError = ''
  let saveSuccess = false

  let categoryFilter = ''
  let searchQuery = ''

  let closeBlocked = false

  let newPrototype = ''
  let newSetting = ''
  let newValue = ''

  // ── Derived ────────────────────────────────────────────────────────────────

  $: displayName = file.canonical_name
    .replace(/^LiveTuningData_?/, '')
    .replace(/\.json$/, '') || 'LiveTuningData'

  $: categories = [...new Set(entries.map(e => categoryForSetting(e.setting)))].sort()

  $: filteredEntries = entries.filter(e => {
    const matchCat = !categoryFilter || categoryForSetting(e.setting) === categoryFilter
    const q = searchQuery.toLowerCase()
    const matchSearch = !q ||
      e.setting.toLowerCase().includes(q) ||
      e.prototype.toLowerCase().includes(q)
    return matchCat && matchSearch
  })

  $: dirty =
    entries.length !== savedEntries.length ||
    entries.some((e, i) => {
      const s = savedEntries[i]
      return !s || e.value !== s.value
    })

  // ── Load ───────────────────────────────────────────────────────────────────

  async function load() {
    loadingEntries = true
    entryError = ''
    try {
      const loaded = await invoke<TuningEntry[]>('read_tuning_file', {
        serverExe,
        canonicalName: file.canonical_name,
      })
      entries = loaded
      savedEntries = JSON.parse(JSON.stringify(loaded))
    } catch (e) {
      entryError = String(e)
    } finally {
      loadingEntries = false
    }
  }

  // ── Save ───────────────────────────────────────────────────────────────────

  async function saveEntries() {
    saving = true
    saveError = ''
    saveSuccess = false
    try {
      await invoke('write_tuning_file', {
        serverExe,
        canonicalName: file.canonical_name,
        entries,
      })
      savedEntries = JSON.parse(JSON.stringify(entries))
      saveSuccess = true
      closeBlocked = false
      setTimeout(() => (saveSuccess = false), 3000)
    } catch (e) {
      saveError = String(e)
    } finally {
      saving = false
    }
  }

  // ── Reload ─────────────────────────────────────────────────────────────────

  async function reloadLiveTuning() {
    try {
      await invoke('send_command', { cmd: '!server reloadlivetuning' })
    } catch {}
  }

  // ── Entries ────────────────────────────────────────────────────────────────

  function addEntry() {
    const trimmed = newSetting.trim()
    if (!trimmed) return
    entries = [...entries, {
      prototype: newPrototype.trim(),
      setting: trimmed,
      value: parseFloat(newValue) || 0,
    }]
    newPrototype = ''
    newSetting = ''
    newValue = ''
  }

  function removeEntry(index: number) {
    const target = filteredEntries[index]
    const globalIndex = entries.indexOf(target)
    if (globalIndex !== -1) {
      entries = entries.filter((_, i) => i !== globalIndex)
    }
  }

  function updateValue(entry: TuningEntry, val: string) {
    entry.value = parseFloat(val) || 0
    entries = [...entries]
  }

  // ── Close guard ────────────────────────────────────────────────────────────

  function tryClose() {
    if (dirty) {
      closeBlocked = true
    } else {
      onClose()
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') tryClose()
  }

  onMount(load)
</script>

<svelte:window on:keydown={onKeydown} />

<div class="modal-backdrop" role="dialog" aria-modal="true">
  <div class="editor-modal">

    <!-- Header -->
    <div class="modal-header">
      <div class="modal-title">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
          <polyline points="14 2 14 8 20 8"/>
          <line x1="16" y1="13" x2="8" y2="13"/>
          <line x1="16" y1="17" x2="8" y2="17"/>
        </svg>
        <span class="modal-display-name">{displayName}</span>
        <span class="modal-filename">{file.canonical_name}</span>
      </div>
      <div class="modal-header-actions">
        <button
          class="btn btn-sm btn-outline"
          on:click={reloadLiveTuning}
          disabled={!serverRunning}
          title={serverRunning ? 'Send !server reloadlivetuning' : 'Server not running'}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:11px;height:11px;">
            <polyline points="23 4 23 10 17 10"/>
            <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/>
          </svg>
          Reload
        </button>
        <button class="close-btn" aria-label="Close" on:click={tryClose}>
          <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5">
            <line x1="3.5" y1="3.5" x2="10.5" y2="10.5"/>
            <line x1="10.5" y1="3.5" x2="3.5" y2="10.5"/>
          </svg>
        </button>
      </div>
    </div>

    <!-- Toolbar -->
    <div class="entry-toolbar">
      <select class="filter-select" bind:value={categoryFilter}>
        <option value="">All categories</option>
        {#each categories as cat}
          <option value={cat}>{cat}</option>
        {/each}
      </select>
      <div class="search-wrap">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="11" cy="11" r="8"/>
          <line x1="21" y1="21" x2="16.65" y2="16.65"/>
        </svg>
        <input
          type="text"
          class="search-input"
          placeholder="Search setting or prototype..."
          bind:value={searchQuery}
        >
      </div>
      <span class="entry-count">
        {filteredEntries.length} entr{filteredEntries.length !== 1 ? 'ies' : 'y'}
      </span>
    </div>

    <!-- Table -->
    <div class="entry-table-wrap">
      {#if loadingEntries}
        <div class="entry-status">Loading...</div>
      {:else if entryError}
        <div class="entry-status error">{entryError}</div>
      {:else if filteredEntries.length === 0 && entries.length > 0}
        <div class="entry-status">No entries match the current filter.</div>
      {:else if entries.length === 0}
        <div class="entry-status">No entries in this file. Add one below.</div>
      {:else}
        <table class="entry-table">
          <thead>
            <tr>
              <th class="col-cat">Category</th>
              <th class="col-proto">Prototype</th>
              <th class="col-setting">Setting</th>
              <th class="col-value">Value</th>
              <th class="col-del"></th>
            </tr>
          </thead>
          <tbody>
            {#each filteredEntries as entry, i (entry.prototype + '|' + entry.setting)}
              {@const originalIndex = entries.indexOf(entry)}
              {@const saved = savedEntries[originalIndex]}
              {@const modified = saved !== undefined && entry.value !== saved.value}
              <tr class:modified>
                <td class="col-cat">
                  <span class="cat-badge">{categoryForSetting(entry.setting)}</span>
                </td>
                <td class="col-proto" title={entry.prototype}>
                  <span class="proto-text">{entry.prototype || '—'}</span>
                </td>
                <td class="col-setting">
                  <span class="setting-text">{entry.setting}</span>
                </td>
                <td class="col-value">
                  <input
                    type="number"
                    class="value-input"
                    class:modified
                    value={entry.value}
                    step="0.01"
                    on:change={e => updateValue(entry, (e.target as HTMLInputElement).value)}
                  >
                </td>
                <td class="col-del">
                  <button class="del-btn" aria-label="Remove entry" on:click={() => removeEntry(i)}>
                    <svg viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.5">
                      <line x1="2" y1="2" x2="8" y2="8"/>
                      <line x1="8" y1="2" x2="2" y2="8"/>
                    </svg>
                  </button>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </div>

    <!-- Add entry row -->
    <div class="add-entry-row">
      <input
        class="add-input proto"
        type="text"
        placeholder="Prototype (empty for Global)"
        bind:value={newPrototype}
      >
      <input
        class="add-input setting"
        type="text"
        placeholder="Setting (e.g. eGTV_XPGain)"
        bind:value={newSetting}
      >
      <input
        class="add-input value-field"
        type="number"
        placeholder="1"
        bind:value={newValue}
        step="0.01"
      >
      <button class="btn btn-sm btn-outline" on:click={addEntry} disabled={!newSetting.trim()}>
        Add
      </button>
    </div>

    <!-- Footer -->
    <div class="modal-footer">
      {#if closeBlocked}
        <span class="feedback-error">Unsaved changes — save or discard before closing.</span>
        <div class="footer-actions">
          <button class="btn btn-sm btn-red" on:click={onClose}>Discard & Close</button>
          <button
            class="btn btn-sm btn-accent btn-pulse"
            on:click={saveEntries}
            disabled={saving}
          >
            {saving ? 'Saving...' : 'Save & Close'}
          </button>
        </div>
      {:else}
        <div class="footer-left">
          {#if dirty}
            <span class="dirty-badge">Unsaved changes</span>
          {/if}
          {#if saveError}
            <span class="feedback-error">{saveError}</span>
          {/if}
          {#if saveSuccess}
            <span class="feedback-ok">Saved</span>
          {/if}
        </div>
        <div class="footer-actions">
          <button class="btn btn-sm btn-outline" on:click={tryClose}>Close</button>
          <button
            class="btn btn-sm btn-accent"
            class:btn-pulse={dirty}
            on:click={saveEntries}
            disabled={saving || !dirty}
          >
            {saving ? 'Saving...' : 'Save'}
          </button>
        </div>
      {/if}
    </div>

  </div>
</div>

<style>
  /* ── Backdrop & box ── */
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.72);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: var(--z-modal);
    backdrop-filter: blur(3px);
  }

  .editor-modal {
    background: var(--bg-1);
    border: 1px solid var(--border-lit);
    border-radius: var(--radius-md);
    width: min(1060px, 92vw);
    height: min(700px, 86vh);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    box-shadow:
      0 32px 100px rgba(0, 0, 0, 0.6),
      0 0 0 1px rgba(255, 255, 255, 0.03) inset;
  }

  /* ── Header ── */
  .modal-header {
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-shrink: 0;
    gap: 12px;
    min-height: 52px;
  }

  .modal-title {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .modal-title svg {
    width: 16px;
    height: 16px;
    color: var(--accent);
    flex-shrink: 0;
  }

  .modal-display-name {
    font-family: var(--font-head);
    font-size: 13px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-0);
    white-space: nowrap;
  }

  .modal-filename {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }

  .modal-header-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .close-btn {
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    border: 1px solid transparent;
    color: var(--text-2);
    cursor: pointer;
    border-radius: var(--radius-sm);
    transition: all 0.12s;
  }
  .close-btn:hover {
    color: var(--text-0);
    background: var(--bg-3);
    border-color: var(--border-mid);
  }
  .close-btn svg { width: 14px; height: 14px; }

  /* ── Toolbar ── */
  .entry-toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 16px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    background: rgba(8, 9, 12, 0.2);
  }

  .filter-select {
    background: var(--bg-0);
    border: 1px solid var(--border-mid);
    color: var(--text-1);
    font-family: var(--font-body);
    font-size: 12px;
    padding: 5px 8px;
    border-radius: var(--radius-sm);
    outline: none;
    cursor: pointer;
  }
  .filter-select:focus { border-color: var(--accent-dim); }

  .search-wrap {
    display: flex;
    align-items: center;
    gap: 6px;
    background: var(--bg-0);
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    padding: 0 8px;
    flex: 1;
  }
  .search-wrap svg { width: 12px; height: 12px; color: var(--text-3); flex-shrink: 0; }
  .search-wrap:focus-within { border-color: var(--accent-dim); }

  .search-input {
    flex: 1;
    background: none;
    border: none;
    outline: none;
    color: var(--text-0);
    font-family: var(--font-mono);
    font-size: 12px;
    padding: 5px 0;
  }
  .search-input::placeholder { color: var(--text-3); font-family: var(--font-body); }

  .entry-count {
    font-family: var(--font-head);
    font-size: 9px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-3);
    white-space: nowrap;
    flex-shrink: 0;
  }

  /* ── Table ── */
  .entry-table-wrap {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
  }

  .entry-status {
    padding: 24px 20px;
    font-family: var(--font-head);
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  .entry-status.error {
    color: var(--text-error);
    text-transform: none;
    font-family: var(--font-body);
  }

  .entry-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }

  .entry-table thead th {
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-3);
    padding: 7px 14px;
    border-bottom: 1px solid var(--border);
    text-align: left;
    position: sticky;
    top: 0;
    background: var(--bg-1);
    white-space: nowrap;
  }

  .entry-table tbody tr {
    border-bottom: 1px solid var(--border);
    transition: background 0.08s;
  }
  .entry-table tbody tr:hover { background: var(--bg-2); }
  .entry-table tbody tr.modified { background: var(--accent-glow); }
  .entry-table tbody tr.modified:hover { background: var(--accent-glow-strong); }

  .entry-table td {
    padding: 7px 14px;
    vertical-align: middle;
  }

  .col-cat     { width: 120px; }
  .col-proto   { width: 28%; }
  .col-setting { }
  .col-value   { width: 110px; }
  .col-del     { width: 36px; }

  .cat-badge {
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-3);
    background: var(--bg-3);
    border: 1px solid var(--border-mid);
    padding: 2px 6px;
    border-radius: 2px;
    white-space: nowrap;
  }

  .proto-text {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    display: block;
  }

  .setting-text {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-1);
  }

  .value-input {
    width: 90px;
    background: var(--bg-0);
    border: 1px solid var(--border-mid);
    color: var(--text-0);
    font-family: var(--font-mono);
    font-size: 12px;
    padding: 3px 7px;
    border-radius: var(--radius-sm);
    outline: none;
    transition: border-color 0.12s;
  }
  .value-input:focus { border-color: var(--accent-dim); }
  .value-input.modified { border-color: var(--accent-dim); }

  .del-btn {
    width: 22px;
    height: 22px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    color: var(--text-3);
    cursor: pointer;
    transition: all 0.1s;
  }
  .del-btn:hover {
    border-color: rgba(192, 57, 43, 0.4);
    color: var(--text-error);
    background: var(--red-dim);
  }
  .del-btn svg { width: 10px; height: 10px; }

  /* ── Add entry row ── */
  .add-entry-row {
    display: flex;
    gap: 6px;
    padding: 8px 14px;
    border-top: 1px solid var(--border);
    background: rgba(8, 9, 12, 0.25);
    flex-shrink: 0;
  }

  .add-input {
    background: var(--bg-0);
    border: 1px solid var(--border-mid);
    color: var(--text-0);
    font-family: var(--font-mono);
    font-size: 12px;
    padding: 5px 8px;
    border-radius: var(--radius-sm);
    outline: none;
    transition: border-color 0.12s;
  }
  .add-input:focus { border-color: var(--accent-dim); }
  .add-input::placeholder { color: var(--text-3); font-family: var(--font-body); font-size: 12px; }
  .add-input.proto      { flex: 2; }
  .add-input.setting    { flex: 3; }
  .add-input.value-field { flex: 1; }

  /* ── Footer ── */
  .modal-footer {
    padding: 10px 16px;
    border-top: 1px solid var(--border);
    background: rgba(8, 9, 12, 0.3);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    flex-shrink: 0;
    min-height: 50px;
  }

  .footer-left {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }

  .footer-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
    margin-left: auto;
  }
</style>