<script lang="ts">
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { categoryForSetting, KNOWN_CORE, KNOWN_EVENTS, KNOWN_SETTINGS, blueprintHintForSetting, type KnownSetting } from '../lib/tuningMeta'

  // ── Props ──────────────────────────────────────────────────────────────────

  /** Existing file to edit. Null when creating a new file. */
  export let file: { canonical_name: string; enabled: boolean; toggleable: boolean } | null = null
  export let serverExe: string
  export let serverRunning: boolean
  /** All canonical names currently on disk — used to validate new filenames. */
  export let existingNames: string[] = []
  export let onClose: () => void
  /** Called after a new file is successfully created on disk. */
  export let onCreated: (canonicalName: string) => void = () => {}

  // ── Mode ───────────────────────────────────────────────────────────────────

  $: isNew = file === null

  // ── Types ──────────────────────────────────────────────────────────────────

  interface TuningEntry {
    prototype: string
    setting: string
    value: number
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

  // New file state
  let newFileSuffix = ''
  let newFileError = ''
  let copyFromName = ''
  let copyFromLoading = false

  // Add-entry row
  let newPrototype = ''
  let newSetting = ''
  let newValue = ''
  let settingSuggestOpen = false
  let settingInputEl: HTMLInputElement

  // Prototype search
  let protoSuggestOpen = false
  let protoSearchResults: { path: string; blueprint: string }[] = []
  let protoSearching = false
  let protoSearchError = ''
  let protoDebounceTimer: ReturnType<typeof setTimeout> | null = null
  let protoInputEl: HTMLInputElement

  // ── Derived ────────────────────────────────────────────────────────────────

  $: displayName = file
    ? file.canonical_name.replace(/^LiveTuningData_?/, '').replace(/\.json$/, '') || 'LiveTuningData'
    : (newFileSuffix.trim() || 'New File')

  $: categories = [...new Set(entries.map(e => categoryForSetting(e.setting)))].sort()

  $: filteredEntries = entries.filter(e => {
    const matchCat = !categoryFilter || categoryForSetting(e.setting) === categoryFilter
    const q = searchQuery.toLowerCase()
    const matchSearch = !q ||
      e.setting.toLowerCase().includes(q) ||
      e.prototype.toLowerCase().includes(q)
    return matchCat && matchSearch
  })

  $: dirty = isNew
    ? entries.length > 0
    : entries.length !== savedEntries.length ||
      entries.some((e, i) => {
        const s = savedEntries[i]
        return !s || e.value !== s.value
      })

  $: filteredSettings = newSetting.trim()
    ? KNOWN_SETTINGS.filter(s =>
        s.setting.toLowerCase().includes(newSetting.toLowerCase()) ||
        s.category.toLowerCase().includes(newSetting.toLowerCase())
      )
    : KNOWN_SETTINGS

  // Whether the currently typed setting is Global (no prototype required)
  $: isGlobalSetting = newSetting.startsWith('eGTV_') ||
    (KNOWN_SETTINGS.find(s => s.setting === newSetting)?.requiresPrototype === false)

  $: if (isGlobalSetting) {
    newPrototype = ''
    protoSuggestOpen = false
  }

  $: copyFromOptions = [
    ...[...KNOWN_CORE].filter(n => existingNames.includes(n)),
    ...[...KNOWN_EVENTS].filter(n => existingNames.includes(n)),
  ]

  // ── Load (edit mode only) ──────────────────────────────────────────────────

  async function load() {
    if (!file) return
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

  // ── Copy from (new mode only) ──────────────────────────────────────────────

  async function applyCopyFrom(name: string) {
    if (!name) { entries = []; return }
    copyFromLoading = true
    saveError = ''
    try {
      entries = await invoke<TuningEntry[]>('read_tuning_file', { serverExe, canonicalName: name })
    } catch (e) {
      saveError = `Could not load ${name}: ${String(e)}`
    } finally {
      copyFromLoading = false
    }
  }

  // ── Save / create ──────────────────────────────────────────────────────────

  function assembledName(): string {
    return `LiveTuningData_${newFileSuffix.trim()}.json`
  }

  function validateNewName(): string {
    const suffix = newFileSuffix.trim()
    if (!suffix) return 'Filename cannot be empty.'
    if (/[/\\.]/.test(suffix)) return 'Filename cannot contain slashes or dots.'
    if (existingNames.includes(assembledName())) return `${assembledName()} already exists.`
    return ''
  }

  async function saveEntries() {
    saveError = ''
    saveSuccess = false

    if (isNew) {
      newFileError = validateNewName()
      if (newFileError) return
      saving = true
      try {
        await invoke('create_tuning_file', { serverExe, canonicalName: assembledName(), entries })
        onCreated(assembledName())
      } catch (e) {
        saveError = String(e)
      } finally {
        saving = false
      }
      return
    }

    saving = true
    try {
      await invoke('write_tuning_file', {
        serverExe,
        canonicalName: file!.canonical_name,
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
    try { await invoke('send_command', { cmd: '!server reloadlivetuning' }) } catch {}
  }

  async function searchPrototypes(query: string) {
    if (isGlobalSetting || query.length < 2) {
      protoSuggestOpen = false
      protoSearchResults = []
      return
    }
    protoSearching = true
    protoSearchError = ''
    try {
      const hint = blueprintHintForSetting(newSetting) ?? undefined
      protoSearchResults = await invoke<{ path: string; blueprint: string }[]>(
        'search_prototypes',
        { serverExe: serverExe, query, blueprintHint: hint }
      )
      protoSuggestOpen = protoSearchResults.length > 0
    } catch (e) {
      protoSearchError = String(e)
      protoSuggestOpen = false
    } finally {
      protoSearching = false
    }
  }

  function onProtoInput() {
    if (protoDebounceTimer) clearTimeout(protoDebounceTimer)
    if (newPrototype.length < 2) {
      protoSuggestOpen = false
      return
    }
    protoDebounceTimer = setTimeout(() => searchPrototypes(newPrototype), 200)
  }

  function selectPrototype(path: string) {
    newPrototype = path
    protoSuggestOpen = false
    protoInputEl?.focus()
  }

  // ── Entry management ───────────────────────────────────────────────────────

  function addEntry() {
    const trimmedSetting = newSetting.trim()
    if (!trimmedSetting) return
    entries = [...entries, {
      prototype: newPrototype.trim(),
      setting: trimmedSetting,
      value: parseFloat(newValue) || 1,
    }]
    newPrototype = ''
    newSetting = ''
    newValue = ''
    settingSuggestOpen = false
  }

  function removeEntry(index: number) {
    const target = filteredEntries[index]
    const globalIndex = entries.indexOf(target)
    if (globalIndex !== -1) entries = entries.filter((_, i) => i !== globalIndex)
  }

  function updateValue(entry: TuningEntry, val: string) {
    entry.value = parseFloat(val) || 0
    entries = [...entries]
  }

  function selectSetting(s: KnownSetting) {
    newSetting = s.setting
    settingSuggestOpen = false
    settingInputEl?.focus()
  }

  // ── Close guard ────────────────────────────────────────────────────────────

  function tryClose() {
    if (dirty && !isNew) { closeBlocked = true } else { onClose() }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      if (protoSuggestOpen) { protoSuggestOpen = false; return }
      if (settingSuggestOpen) { settingSuggestOpen = false; return }
      tryClose()
    }
  }

  function onWindowClick(e: MouseEvent) {
    if (settingSuggestOpen && !(e.target as Element).closest('.setting-suggest-wrap')) {
      settingSuggestOpen = false
    }
    if (protoSuggestOpen && !(e.target as Element).closest('.proto-suggest-wrap')) {
      protoSuggestOpen = false
    }
  }

  onMount(() => { if (!isNew) load() })
</script>

<svelte:window on:keydown={onKeydown} on:click={onWindowClick} />

<div class="modal-backdrop"
  role="presentation"
  on:click={e => { if (e.target === e.currentTarget) tryClose() }}
  on:keydown={onKeydown}
>
  <div
    class="editor-modal"
    role="dialog"
    aria-modal="true"
    tabindex="-1"
  >

    <!-- ── Header ── -->
    <div class="modal-header">
      <div class="modal-title">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
          <polyline points="14 2 14 8 20 8"/>
          <line x1="16" y1="13" x2="8" y2="13"/>
          <line x1="16" y1="17" x2="8" y2="17"/>
        </svg>
        {#if isNew}
          <span class="modal-display-name">New Tuning File</span>
        {:else}
          <span class="modal-display-name">{displayName}</span>
          <span class="modal-filename">{file?.canonical_name}</span>
        {/if}
      </div>
      <div class="modal-header-actions">
        {#if !isNew}
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
        {/if}
        <button class="close-btn" aria-label="Close" on:click={tryClose}>
          <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5">
            <line x1="3.5" y1="3.5" x2="10.5" y2="10.5"/>
            <line x1="10.5" y1="3.5" x2="3.5" y2="10.5"/>
          </svg>
        </button>
      </div>
    </div>

    <!-- ── New file bar ── -->
    {#if isNew}
      <div class="new-file-bar">
        <div class="filename-row">
          <div class="filename-compose" class:has-error={!!newFileError}>
            <span class="filename-fixed">LiveTuningData_</span>
            <input
              type="text"
              class="filename-input"
              placeholder="MyCustomFile"
              bind:value={newFileSuffix}
              on:input={() => newFileError = ''}
              spellcheck="false"
              autocomplete="off"
            >
            <span class="filename-fixed">.json</span>
          </div>
          {#if newFileError}
            <span class="filename-error">{newFileError}</span>
          {/if}
        </div>
        {#if copyFromOptions.length > 0}
          <div class="copy-from-wrap">
            <span class="copy-from-label">Copy from</span>
            <select
              class="filter-select"
              bind:value={copyFromName}
              on:change={() => applyCopyFrom(copyFromName)}
              disabled={copyFromLoading}
            >
              <option value="">— blank —</option>
              {#each copyFromOptions as name}
                <option value={name}>
                  {name.replace(/^LiveTuningData_?/, '').replace(/\.json$/, '') || 'LiveTuningData'}
                </option>
              {/each}
            </select>
            {#if copyFromLoading}
              <span class="copy-loading">Loading...</span>
            {/if}
          </div>
        {/if}
      </div>
    {/if}

    <!-- ── Toolbar ── -->
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
        <input type="text" class="search-input" placeholder="Search setting or prototype..."
          bind:value={searchQuery}>
      </div>
      <span class="entry-count">
        {filteredEntries.length} entr{filteredEntries.length !== 1 ? 'ies' : 'y'}
      </span>
    </div>

    <!-- ── Table ── -->
    <div class="entry-table-wrap">
      {#if loadingEntries}
        <div class="entry-status">Loading...</div>
      {:else if entryError}
        <div class="entry-status error">{entryError}</div>
      {:else if entries.length === 0}
        <div class="entry-status">
          {isNew
            ? 'No entries yet — add one below or copy from a bundled file.'
            : 'No entries in this file. Add one below.'}
        </div>
      {:else if filteredEntries.length === 0}
        <div class="entry-status">No entries match the current filter.</div>
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
              {@const modified = !isNew && saved !== undefined && entry.value !== saved.value}
              <tr class:modified>
                <td class="col-cat"><span class="cat-badge">{categoryForSetting(entry.setting)}</span></td>
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

    <!-- ── Add entry row ── -->
    <div class="add-entry-row">
      <!-- Prototype: searchable dropdown backed by Calligraphy.sip -->
      <div class="add-field proto proto-suggest-wrap">
        <input
          class="add-input"
          class:input-dimmed={isGlobalSetting}
          type="text"
          bind:this={protoInputEl}
          bind:value={newPrototype}
          on:input={onProtoInput}
          on:focus={() => { if (newPrototype.length >= 2 && !isGlobalSetting) searchPrototypes(newPrototype) }}
          placeholder={isGlobalSetting ? 'Not required (Global setting)' : 'Prototype path — type to search'}
          disabled={isGlobalSetting}
          autocomplete="off"
          spellcheck="false"
        >
        {#if protoSearching}
          <div class="proto-hint">Searching...</div>
        {:else if protoSearchError}
          <div class="proto-hint error">{protoSearchError}</div>
        {/if}
        {#if protoSuggestOpen}
          <div class="proto-dropdown">
            {#each protoSearchResults as result}
              <button
                class="proto-option"
                type="button"
                on:click={() => selectPrototype(result.path)}
              >
                <span class="proto-opt-path">{result.path}</span>
                <span class="proto-opt-bp">{result.blueprint}</span>
              </button>
            {/each}
          </div>
        {/if}
      </div>

      <!-- Setting: combobox with filterable known-settings dropdown -->
      <div class="add-field setting setting-suggest-wrap">
        <input
          bind:this={settingInputEl}
          class="add-input"
          type="text"
          placeholder="Setting (e.g. eGTV_XPGain)"
          bind:value={newSetting}
          on:focus={() => settingSuggestOpen = true}
          on:input={() => settingSuggestOpen = true}
          autocomplete="off"
          spellcheck="false"
        >
        {#if settingSuggestOpen && filteredSettings.length > 0}
          <div class="setting-dropdown">
            {#each filteredSettings as s}
              <button
                class="setting-option"
                tabindex="-1"
                on:mousedown|preventDefault={() => selectSetting(s)}
              >
                <span class="setting-opt-name">{s.setting}</span>
                <span class="setting-opt-cat">{s.category}</span>
                {#if s.description}
                  <span class="setting-opt-desc">{s.description}</span>
                {/if}
              </button>
            {/each}
          </div>
        {/if}
      </div>

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

    <!-- ── Footer ── -->
    <div class="modal-footer">
      {#if closeBlocked}
        <span class="feedback-error">Unsaved changes — save or discard before closing.</span>
        <div class="footer-actions">
          <button class="btn btn-sm btn-red" on:click={onClose}>Discard & Close</button>
          <button class="btn btn-sm btn-accent btn-pulse" on:click={saveEntries} disabled={saving}>
            {saving ? 'Saving...' : 'Save & Close'}
          </button>
        </div>
      {:else}
        <div class="footer-left">
          {#if dirty && !isNew}<span class="dirty-badge">Unsaved changes</span>{/if}
          {#if saveError}<span class="feedback-error">{saveError}</span>{/if}
          {#if saveSuccess}<span class="feedback-ok">Saved</span>{/if}
        </div>
        <div class="footer-actions">
          <button class="btn btn-sm btn-outline" on:click={tryClose}>
            {isNew ? 'Cancel' : 'Close'}
          </button>
          <button
            class="btn btn-sm btn-accent"
            class:btn-pulse={dirty}
            on:click={saveEntries}
            disabled={saving || (!isNew && !dirty)}
          >
            {saving ? 'Saving...' : isNew ? 'Create File' : 'Save'}
          </button>
        </div>
      {/if}
    </div>

  </div>
</div>

<style>
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
  .modal-title svg { width: 16px; height: 16px; color: var(--accent); flex-shrink: 0; }

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

  .modal-header-actions { display: flex; align-items: center; gap: 8px; flex-shrink: 0; }

  .close-btn {
    width: 28px; height: 28px;
    display: flex; align-items: center; justify-content: center;
    background: transparent;
    border: 1px solid transparent;
    color: var(--text-2);
    cursor: pointer;
    border-radius: var(--radius-sm);
    transition: all 0.12s;
  }
  .close-btn:hover { color: var(--text-0); background: var(--bg-3); border-color: var(--border-mid); }
  .close-btn svg { width: 14px; height: 14px; }

  /* ── New file bar ── */
  .new-file-bar {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 10px 16px;
    border-bottom: 1px solid var(--border);
    background: rgba(8, 9, 12, 0.2);
    flex-shrink: 0;
    flex-wrap: wrap;
  }

  .filename-row {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }

  .filename-compose {
    display: flex;
    align-items: center;
    background: var(--bg-0);
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    overflow: hidden;
    transition: border-color 0.15s;
  }
  .filename-compose:focus-within { border-color: var(--accent-dim); }
  .filename-compose.has-error { border-color: var(--red); }

  .filename-fixed {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-3);
    padding: 6px 8px;
    white-space: nowrap;
    background: var(--bg-2);
    user-select: none;
    flex-shrink: 0;
  }

  .filename-input {
    background: transparent;
    border: none;
    outline: none;
    color: var(--text-0);
    font-family: var(--font-mono);
    font-size: 12px;
    padding: 6px 8px;
    min-width: 160px;
    width: 200px;
  }
  .filename-input::placeholder { color: var(--text-3); font-family: var(--font-body); }

  .filename-error { font-size: 11px; color: var(--text-error); }

  .copy-from-wrap {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-left: auto;
  }

  .copy-from-label {
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-3);
    white-space: nowrap;
  }

  .copy-loading {
    font-family: var(--font-head);
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
  }

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
    display: flex; align-items: center; gap: 6px;
    background: var(--bg-0);
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    padding: 0 8px; flex: 1;
  }
  .search-wrap svg { width: 12px; height: 12px; color: var(--text-3); flex-shrink: 0; }
  .search-wrap:focus-within { border-color: var(--accent-dim); }

  .search-input {
    flex: 1; background: none; border: none; outline: none;
    color: var(--text-0); font-family: var(--font-mono); font-size: 12px; padding: 5px 0;
  }
  .search-input::placeholder { color: var(--text-3); font-family: var(--font-body); }

  .entry-count {
    font-family: var(--font-head); font-size: 9px;
    letter-spacing: 0.1em; text-transform: uppercase;
    color: var(--text-3); white-space: nowrap; flex-shrink: 0;
  }

  /* ── Table ── */
  .entry-table-wrap { flex: 1; overflow-y: auto; min-height: 0; }

  .entry-status {
    padding: 24px 20px;
    font-family: var(--font-head); font-size: 11px;
    letter-spacing: 0.08em; text-transform: uppercase; color: var(--text-3);
  }
  .entry-status.error { color: var(--text-error); text-transform: none; font-family: var(--font-body); letter-spacing: 0; }

  .entry-table { width: 100%; border-collapse: collapse; font-size: 12px; }

  .entry-table thead th {
    font-family: var(--font-head); font-size: 9px; font-weight: 600;
    letter-spacing: 0.12em; text-transform: uppercase; color: var(--text-3);
    padding: 7px 14px; border-bottom: 1px solid var(--border);
    text-align: left; position: sticky; top: 0; background: var(--bg-1); white-space: nowrap;
  }

  .entry-table tbody tr { border-bottom: 1px solid var(--border); transition: background 0.08s; }
  .entry-table tbody tr:hover { background: var(--bg-2); }
  .entry-table tbody tr.modified { background: var(--accent-glow); }
  .entry-table tbody tr.modified:hover { background: var(--accent-glow-strong); }
  .entry-table td { padding: 7px 14px; vertical-align: middle; }

  .col-cat   { width: 120px; }
  .col-proto { width: 28%; }
  .col-value { width: 110px; }
  .col-del   { width: 36px; }

  .cat-badge {
    font-family: var(--font-head); font-size: 9px; font-weight: 600;
    letter-spacing: 0.06em; text-transform: uppercase;
    color: var(--text-3); background: var(--bg-3);
    border: 1px solid var(--border-mid); padding: 2px 6px; border-radius: 2px; white-space: nowrap;
  }

  .proto-text {
    font-family: var(--font-mono); font-size: 11px; color: var(--text-2);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis; display: block;
  }

  .setting-text { font-family: var(--font-mono); font-size: 12px; color: var(--text-1); }

  .value-input {
    width: 90px; background: var(--bg-0);
    border: 1px solid var(--border-mid); color: var(--text-0);
    font-family: var(--font-mono); font-size: 12px;
    padding: 3px 7px; border-radius: var(--radius-sm);
    outline: none; transition: border-color 0.12s;
  }
  .value-input:focus { border-color: var(--accent-dim); }
  .value-input.modified { border-color: var(--accent-dim); }

  .del-btn {
    width: 22px; height: 22px;
    display: flex; align-items: center; justify-content: center;
    background: none; border: 1px solid transparent;
    border-radius: var(--radius-sm); color: var(--text-3); cursor: pointer; transition: all 0.1s;
  }
  .del-btn:hover { border-color: rgba(192,57,43,0.4); color: var(--text-error); background: var(--red-dim); }
  .del-btn svg { width: 10px; height: 10px; }

  /* ── Add entry row ── */
  .add-entry-row {
    display: flex; gap: 6px; padding: 8px 14px;
    border-top: 1px solid var(--border);
    background: rgba(8, 9, 12, 0.25);
    flex-shrink: 0; align-items: flex-start;
  }

  .add-field { display: flex; flex-direction: column; }
  .add-field.proto   { flex: 2; }
  .add-field.setting { flex: 3; }

  .add-input {
    background: var(--bg-0); border: 1px solid var(--border-mid);
    color: var(--text-0); font-family: var(--font-mono); font-size: 12px;
    padding: 5px 8px; border-radius: var(--radius-sm);
    outline: none; transition: border-color 0.12s; width: 100%;
  }
  .add-input:focus { border-color: var(--accent-dim); }
  .add-input::placeholder { color: var(--text-3); font-family: var(--font-body); font-size: 12px; }

  .value-field { flex: 1; }

  /* ── Prototype autocomplete ── */
  .proto-suggest-wrap { position: relative; flex: 2; display: flex; flex-direction: column; }

  .input-dimmed { opacity: 0.45; cursor: not-allowed; }

  .proto-hint {
    font-size: 10px;
    color: var(--text-3);
    padding: 2px 2px 0;
  }
  .proto-hint.error { color: var(--text-error); }

  .proto-dropdown {
    position: absolute;
    bottom: calc(100% + 3px);
    left: 0;
    min-width: max(100%, 460px);
    background: var(--bg-2);
    border: 1px solid var(--border-lit);
    border-radius: var(--radius-sm);
    z-index: var(--z-dropdown);
    max-height: 280px;
    overflow-y: auto;
    box-shadow: 0 -8px 32px rgba(0,0,0,0.45);
  }

  .proto-option {
    display: flex;
    flex-direction: column;
    gap: 1px;
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    border-bottom: 1px solid var(--border);
    padding: 6px 10px;
    cursor: pointer;
    transition: background 0.08s;
  }
  .proto-option:last-child { border-bottom: none; }
  .proto-option:hover { background: var(--bg-3); }

  .proto-opt-path {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-0);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .proto-opt-bp {
    font-family: var(--font-head);
    font-size: 9px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--accent-dim);
  }

  /* ── Setting autocomplete ── */
  .setting-suggest-wrap { position: relative; flex: 3; display: flex; flex-direction: column; }

  .setting-dropdown {
    position: absolute;
    bottom: calc(100% + 3px);
    right: 0;
    min-width: max(100%, 460px);
    background: var(--bg-2);
    border: 1px solid var(--border-lit);
    border-radius: var(--radius-sm);
    z-index: var(--z-dropdown);
    max-height: 320px;
    overflow-y: auto;
    box-shadow: 0 -8px 32px rgba(0,0,0,0.45);
  }

  .setting-option {
    display: flex; flex-direction: column; gap: 2px;
    width: 100%; text-align: left;
    background: none; border: none;
    border-bottom: 1px solid var(--border);
    padding: 7px 10px; cursor: pointer; transition: background 0.08s;
  }
  .setting-option:last-child { border-bottom: none; }
  .setting-option:hover { background: var(--bg-3); }

  .setting-opt-name { font-family: var(--font-mono); font-size: 12px; color: var(--text-0); }
  .setting-opt-cat {
    font-family: var(--font-head); font-size: 9px;
    letter-spacing: 0.08em; text-transform: uppercase; color: var(--accent-dim);
  }
  .setting-opt-desc { font-size: 11px; color: var(--text-2); line-height: 1.4; }

  /* ── Footer ── */
  .modal-footer {
    padding: 10px 16px; border-top: 1px solid var(--border);
    background: rgba(8, 9, 12, 0.3);
    display: flex; align-items: center; justify-content: space-between;
    gap: 10px; flex-shrink: 0; min-height: 50px;
  }

  .footer-left { display: flex; align-items: center; gap: 10px; min-width: 0; }

  .footer-actions { display: flex; align-items: center; gap: 8px; flex-shrink: 0; margin-left: auto; }
</style>