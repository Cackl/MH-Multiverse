<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { categoryForSetting, KNOWN_CORE, KNOWN_EVENTS, KNOWN_SETTINGS, NEW_TUNING_FILE_PREFIX, blueprintHintForSetting, tuningClassForSetting, tuningPickAllowed, knownSettingsFor, newTuningFileName, parseTuningValue, tuningDisplayName, settingTip, tuningEntriesChanged, type KnownSetting } from '../../../lib/tuningMeta'
  import { tip } from '../../../lib/tooltip'
  import { activeLinked, setUnsaved } from '../../../lib/store'
  import type { PatchNames } from '../../../lib/patchesMeta'
  import PrototypePicker from '../../PrototypePicker.svelte'
  import Dropdown from '../../Dropdown.svelte'

  // ── Props ──────────────────────────────────────────────────────────────────

  /** Existing file to edit. Null when creating a new file. */
  export let file: { canonical_name: string; relative_path: string } | null = null
  export let serverExe: string
  /** All canonical names currently on disk — used to validate new filenames. */
  export let existingNames: string[] = []
  /** Cancel on a new file. */
  export let onCancel: () => void = () => {}
  /** Called after a new file is successfully created on disk. */
  export let onCreated: (canonicalName: string) => void = () => {}
  /** Read-only for the parent (bind:dirty): true while there are unsaved edits. */
  export let dirty = false

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
  let addError = ''
  let saveSuccess = false

  let categoryFilter = ''
  let searchQuery = ''

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

  // Display names for prototype paths, as in Patches. `asked` holds every path already
  // sent, so each one is looked up once.
  let protoNames: Record<string, string> = {}
  const asked = new Set<string>()

  // ── Derived ────────────────────────────────────────────────────────────────

  $: categories = [...new Set(entries.map(e => categoryForSetting(e.setting)))].sort()

  $: filteredEntries = entries.filter(e => {
    const matchCat = !categoryFilter || categoryForSetting(e.setting) === categoryFilter
    const q = searchQuery.toLowerCase()
    const matchSearch = !q ||
      e.setting.toLowerCase().includes(q) ||
      e.prototype.toLowerCase().includes(q) ||
      (protoNames[e.prototype] ?? '').toLowerCase().includes(q)
    return matchCat && matchSearch
  })

  $: requestNames(entries)

  $: if (newSetting || newPrototype) addError = ''
  $: dirty = isNew
    ? entries.length > 0
    : tuningEntriesChanged(entries, savedEntries)

  // Leaving the tab asks first while there are unsaved edits.
  $: setUnsaved('tuning', dirty ? 'Live Tuning' : null)
  onDestroy(() => setUnsaved('tuning', null))

  $: known = knownSettingsFor($activeLinked?.version ?? '')
  $: filteredSettings = newSetting.trim()
    ? known.filter(s =>
        s.setting.toLowerCase().includes(newSetting.toLowerCase()) ||
        s.category.toLowerCase().includes(newSetting.toLowerCase())
      )
    : known

  // Whether the currently typed setting is Global (no prototype required)
  $: isGlobalSetting = newSetting.startsWith('eGTV_') ||
    (KNOWN_SETTINGS.find(s => s.setting === newSetting)?.requiresPrototype === false)

  $: if (isGlobalSetting) {
    newPrototype = ''
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
        relativePath: file.relative_path,
      })
      entries = loaded
      savedEntries = JSON.parse(JSON.stringify(loaded))
    } catch (e) {
      entryError = String(e)
    } finally {
      loadingEntries = false
    }
  }

  async function requestNames(list: TuningEntry[]) {
    const paths = [...new Set(list.map(e => e.prototype))].filter(p => p && !asked.has(p))
    if (!serverExe || paths.length === 0) return
    for (const p of paths) asked.add(p)
    try {
      const got = await invoke<PatchNames>('name_patch_targets', { serverExe, paths, ids: [] })
      protoNames = { ...protoNames, ...got.paths }
    } catch {
      // Names are a help, not needed: without Calligraphy.sip the table shows paths.
    }
  }

  // ── Copy from (new mode only) ──────────────────────────────────────────────

  async function applyCopyFrom(name: string) {
    if (!name) { entries = []; return }
    copyFromLoading = true
    saveError = ''
    try {
      entries = await invoke<TuningEntry[]>('read_tuning_file', { serverExe, relativePath: name })
    } catch (e) {
      saveError = `Could not load ${name}: ${String(e)}`
    } finally {
      copyFromLoading = false
    }
  }

  // ── Save / create ──────────────────────────────────────────────────────────

  function assembledName(): string {
    return newTuningFileName(newFileSuffix)
  }

  function validateNewName(): string {
    const suffix = newFileSuffix.trim()
    if (!suffix) return 'Filename cannot be empty.'
    if (/[/\\.]/.test(suffix)) return 'Filename cannot contain slashes or dots.'
    if (existingNames.includes(assembledName())) return `${assembledName()} already exists.`
    return ''
  }

  async function saveEntries() {
    if (entryError) return
    saveError = ''
    saveSuccess = false

    if (isNew) {
      newFileError = validateNewName()
      if (newFileError) return
      saving = true
      try {
        await invoke('create_tuning_file', { serverExe, relativePath: assembledName(), entries })
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
        relativePath: file!.relative_path,
        entries,
      })
      savedEntries = JSON.parse(JSON.stringify(entries))
      saveSuccess = true
      setTimeout(() => (saveSuccess = false), 3000)
    } catch (e) {
      saveError = String(e)
    } finally {
      saving = false
    }
  }

  function revert() {
    entries = JSON.parse(JSON.stringify(savedEntries))
    saveError = ''
  }

  // ── Entry management ───────────────────────────────────────────────────────

  function addEntry() {
    const trimmedSetting = newSetting.trim()
    if (!trimmedSetting || entryError) return
    const trimmedProto = newPrototype.trim()
    // Blank keeps the "1" placeholder; anything else must be a real number.
    const value = String(newValue ?? '').trim() === '' ? 1 : parseTuningValue(newValue)
    if (value === null) { addError = 'Value must be a number no bigger than 3.4e38.'; return }
    // Rows are keyed by prototype|setting, so a duplicate would crash the list.
    if (entries.some(e => e.prototype === trimmedProto && e.setting === trimmedSetting)) {
      addError = 'This setting is already in the file for that prototype.'
      return
    }
    addError = ''
    entries = [...entries, { prototype: trimmedProto, setting: trimmedSetting, value }]
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
    // Blank/invalid input keeps the old value (re-render restores the field).
    const parsed = parseTuningValue(val)
    if (parsed !== null) entry.value = parsed
    entries = [...entries]
  }

  function selectSetting(s: KnownSetting) {
    newSetting = s.setting
    settingSuggestOpen = false
    settingInputEl?.focus()
  }

  // Prototype cell expand
  let expandedProtoKey: string | null = null

  function toggleProtoExpand(entry: TuningEntry) {
    if (!entry.prototype) return
    const key = entry.prototype + '|' + entry.setting
    expandedProtoKey = expandedProtoKey === key ? null : key
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') settingSuggestOpen = false
  }

  function onWindowClick(e: MouseEvent) {
    if (settingSuggestOpen && !(e.target as Element).closest('.setting-suggest-wrap')) {
      settingSuggestOpen = false
    }
  }

  onMount(() => { if (!isNew) load() })
</script>

<svelte:window on:keydown={onKeydown} on:click={onWindowClick} />

<section class="file-editor">

    <!-- ── New file bar ── -->
    {#if isNew}
      <div class="new-file-bar">
        <div class="filename-row">
          <div class="filename-compose" class:has-error={!!newFileError}>
            <span class="filename-fixed">{NEW_TUNING_FILE_PREFIX}</span>
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
            <Dropdown label="Copy from" value={copyFromName} disabled={copyFromLoading} --dd-min-width="0"
              options={[{ value: '', label: 'Blank' }, ...copyFromOptions.map(n => ({ value: n, label: tuningDisplayName(n) }))]}
              onChange={v => { copyFromName = v; applyCopyFrom(v) }} />
            {#if copyFromLoading}
              <span class="copy-loading">Loading...</span>
            {/if}
          </div>
        {/if}
      </div>
    {/if}

    <!-- ── Toolbar ── -->
    <div class="entry-toolbar">
      <Dropdown label="Category" value={categoryFilter} --dd-pad="5px 8px" --dd-min-width="0" onChange={v => categoryFilter = v}
        options={[{ value: '', label: 'All categories' }, ...categories.map(c => ({ value: c, label: c }))]} />
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
            ? 'No entries yet. Add one below or copy from a bundled file.'
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
              {@const rowKey = entry.prototype + '|' + entry.setting}
              {@const isExpanded = expandedProtoKey === rowKey}
              <tr class:modified>
                <td class="col-cat"><span class="tag">{categoryForSetting(entry.setting)}</span></td>
                <td
                  class="col-proto"
                  class:proto-expandable={!!entry.prototype}
                  class:proto-expanded={isExpanded}
                  on:click={() => toggleProtoExpand(entry)}
                >
                  <span class="proto-text" class:proto-name={protoNames[entry.prototype]}>{protoNames[entry.prototype] ?? (entry.prototype || '—')}</span>
                </td>
                <td class="col-setting">
                  <span class="setting-text" use:tip={settingTip(entry.setting)}>{entry.setting}</span>
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
              {#if isExpanded}
                <tr class="proto-expand-row" class:modified>
                  <td colspan="5">
                    <div class="proto-expand-content">{entry.prototype}</div>
                  </td>
                </tr>
              {/if}
            {/each}
          </tbody>
        </table>
      {/if}
    </div>

    <!-- ── Add entry row ── -->
    <div class="add-entry-row">
      <!-- Prototype: searchable dropdown backed by Calligraphy.sip -->
      <div class="add-field proto">
        <PrototypePicker
          bind:value={newPrototype}
          {serverExe}
          compact
          disabled={isGlobalSetting}
          blueprintHint={blueprintHintForSetting(newSetting) ?? undefined}
          classFilter={tuningClassForSetting(newSetting) ?? undefined}
          allow={m => tuningPickAllowed(newSetting, m)}
          placeholder={isGlobalSetting ? 'Not required (Global setting)' : undefined}
        />
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
                <span class="setting-opt-desc">{s.description}</span>
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
      <!-- A file that failed to load can't be added to or saved: the save would replace it. -->
      <button class="btn btn-sm btn-outline" on:click={addEntry} disabled={!newSetting.trim() || !!entryError}>
        Add
      </button>
    </div>
    {#if addError}<div class="feedback-error add-error">{addError}</div>{/if}

    <!-- ── Footer ── -->
    <div class="editor-footer">
      <div class="footer-left">
        {#if dirty && !isNew}<span class="dirty-badge">Unsaved changes</span>{/if}
        {#if saveError}<span class="feedback-error">{saveError}</span>{/if}
        {#if saveSuccess}<span class="feedback-ok">Saved</span>{/if}
      </div>
      <div class="footer-actions">
        {#if isNew}
          <button class="btn btn-sm btn-outline" on:click={onCancel}>Cancel</button>
        {:else}
          <button class="btn btn-sm btn-outline" on:click={revert} disabled={saving || !dirty}>Revert</button>
        {/if}
        <button
          class="btn btn-sm btn-accent"
          class:btn-pulse={dirty}
          on:click={saveEntries}
          disabled={saving || !!entryError || (!isNew && !dirty)}
        >
          {saving ? 'Saving...' : isNew ? 'Create File' : 'Save'}
        </button>
      </div>
    </div>

</section>

<style>
  .file-editor {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }

  /* ── New file bar ── */
  .new-file-bar {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 10px 16px;
    border-bottom: 1px solid var(--border);
    background: var(--chrome-sunken-bg);
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
    font-family: var(--font-body);
    font-size: 12px;
    padding: 6px 8px;
    min-width: 160px;
    width: 200px;
  }

  .filename-error { font-size: 11px; color: var(--text-error); }

  .copy-from-wrap {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-left: auto;
  }

  .copy-from-label {
    font-family: var(--font-head);
    font-size: var(--fs-min);
    font-weight: 600;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-3);
    white-space: nowrap;
  }

  .copy-loading { font-size: 12px; color: var(--text-3); }

  /* ── Toolbar ── */
  .entry-toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 16px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    background: var(--chrome-sunken-bg);
  }

  /* Same height as the search box beside it. */

  .entry-count {
    font-size: 12px; font-variant-numeric: tabular-nums;
    color: var(--text-2); white-space: nowrap; flex-shrink: 0;
  }

  /* ── Table ── */
  .entry-table-wrap { flex: 1; overflow-y: auto; min-height: 0; }

  .entry-status { padding: 24px 20px; font-size: 12px; color: var(--text-3); }
  .entry-status.error { color: var(--text-error); }
  .add-error { padding: 4px 16px 0; font-size: 11px; }

  .entry-table { width: 100%; border-collapse: collapse; font-size: 12px; table-layout: fixed; }

  .entry-table thead th {
    font-family: var(--font-head); font-size: var(--fs-min); font-weight: 600;
    letter-spacing: 0.12em; text-transform: uppercase; color: var(--text-3);
    padding: 7px 14px; border-bottom: 1px solid var(--border);
    text-align: left; position: sticky; top: 0; background: var(--bg-1); white-space: nowrap;
    overflow: hidden;
  }

  .entry-table tbody tr { border-bottom: 1px solid var(--border); transition: background 0.08s; }
  .entry-table tbody tr:hover { background: var(--bg-2); }
  .entry-table tbody tr.modified { background: var(--accent-glow); }
  .entry-table tbody tr.modified:hover { background: var(--accent-glow-strong); }
  .entry-table td { padding: 7px 14px; vertical-align: middle; overflow: hidden; }

  /* Prototype takes what's left: paths are long, setting IDs are at most about 35 characters. */
  .col-cat     { width: 120px; }
  .col-setting { width: max(26%, 200px); }
  .col-value   { width: 110px; }
  .col-del   { width: 50px; }

  /* Clickable prototype cell */
  .col-proto.proto-expandable { cursor: pointer; }
  .col-proto.proto-expandable:hover .proto-text { color: var(--text-0); }
  .col-proto.proto-expanded .proto-text { color: var(--accent-bright); }

  /* Expanded prototype sub-row */
  .proto-expand-row { border-bottom: 1px solid var(--border) !important; }
  .proto-expand-row:hover { background: transparent !important; }
  .proto-expand-row.modified { background: var(--accent-glow) !important; }
  .proto-expand-row td { padding: 0; overflow: visible; }

  .proto-expand-content {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-1);
    line-height: 1.6;
    padding: 7px 14px 8px;
    border-top: 1px solid var(--border);
    border-left: 2px solid var(--accent-dim);
    word-break: break-all;
    user-select: text;
    background: var(--bg-0);
  }

  .proto-text {
    font-family: var(--font-mono); font-size: 11px; color: var(--text-2);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis; display: block;
  }

  .proto-text.proto-name { font-family: var(--font-body); font-size: 12px; color: var(--text-1); }

  .setting-text {
    font-family: var(--font-mono); font-size: 12px; color: var(--text-1);
    display: block; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }

  .value-input {
    width: 90px; background: var(--bg-0);
    border: 1px solid var(--border-mid); color: var(--text-0);
    font-family: var(--font-body); font-size: 12px; font-variant-numeric: tabular-nums;
    padding: 3px 7px; border-radius: var(--radius-sm);
    outline: none; transition: border-color 0.12s;
  }
  .value-input:focus { border-color: var(--accent-dim); }
  .value-input.modified { border-color: var(--accent-dim); }

  /* ── Add entry row ── */
  .add-entry-row {
    display: flex; gap: 6px; padding: 8px 14px;
    border-top: 1px solid var(--border);
    background: var(--chrome-footer-bg);
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

  .value-field { flex: 1; font-family: var(--font-body); font-variant-numeric: tabular-nums; }

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
    font-family: var(--font-head); font-size: var(--fs-min);
    letter-spacing: 0.08em; text-transform: uppercase; color: var(--text-2);
  }
  .setting-opt-desc { font-size: 11px; color: var(--text-2); line-height: 1.4; }

  /* ── Footer ── */
  .editor-footer {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 16px;
    border-top: 1px solid var(--border);
    background: var(--chrome-footer-bg);
    flex-shrink: 0;
  }

  .footer-left { display: flex; align-items: center; gap: 10px; min-width: 0; }

  .footer-actions { display: flex; align-items: center; gap: 8px; flex-shrink: 0; margin-left: auto; }
</style>