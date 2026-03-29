<script lang="ts">
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { appConfig, serverRunning, setTuningTags } from '../lib/store'

  // ── Types ──────────────────────────────────────────────────────────────────

  interface TuningFileInfo {
    canonical_name: string
    enabled: boolean
    toggleable: boolean
  }

  interface TuningEntry {
    prototype: string
    setting: string
    value: number
  }

  type Tag = 'core' | 'event' | 'custom' | ''

  const TAG_LABELS: Record<string, string> = {
    core:  'Core',
    event: 'Event',
    custom: 'Custom',
    '':    'Untagged',
  }

  const KNOWN_CORE = new Set([
    'LiveTuningData.json',
    'LiveTuningDataBugFixes.json',
    'LiveTuningDataGlobal.json',
    'LiveTuningDataPvP.json',
  ])

  const KNOWN_EVENTS = new Set([
    'LiveTuningData_CosmicChaos.json',
    'LiveTuningData_MidtownMadness.json',
    'LiveTuningData_ArmorIncursion.json',
    'LiveTuningData_OdinsBounty.json',
    'LiveTuningData_Defenders&FriendsXP.json',
    'LiveTuningData_AvengersXP.json',
    'LiveTuningData_FantasticFourXP.json',
    'LiveTuningData_Guardians&CosmicXP.json',
    'LiveTuningData_Scoundrels&VillainsXP.json',
    'LiveTuningData_XMenXP.json',
    'LiveTuningData_PandemoniumProtocol.json',
  ])

  // ── Category prefix map ────────────────────────────────────────────────────

  const CATEGORY_PREFIXES: [string, string][] = [
    ['eGTV_',   'Global'],
    ['eWETV_',  'World Entity'],
    ['ePTV_',   'Powers'],
    ['eRTV_',   'Regions'],
    ['eRT_',    'Regions'],
    ['eLTTV_',  'Loot'],
    ['eMTV_',   'Mission'],
    ['eCTV_',   'Condition'],
    ['eAETV_',  'Avatar Entity'],
    ['eATV_',   'Area'],
    ['ePOTV_',  'Population Object'],
    ['eMFTV_',  'Metrics Frequency'],
    ['ePETV_',  'Public Events'],
  ]

  function categoryForSetting(setting: string): string {
    for (const [prefix, label] of CATEGORY_PREFIXES) {
      if (setting.startsWith(prefix)) return label
    }
    return 'Other'
  }

  // ── State ──────────────────────────────────────────────────────────────────

  let files: TuningFileInfo[] = []
  let scanError = ''
  let scanning = false

  let selectedFile: TuningFileInfo | null = null
  let entries: TuningEntry[] = []
  let savedEntries: TuningEntry[] = []
  let loadingEntries = false
  let entryError = ''

  let saving = false
  let saveError = ''
  let saveSuccess = false

  let tagFilter: Tag | '' = ''
  let searchQuery = ''
  let categoryFilter = ''

  let editingTag: string | null = null

  // ── Derived ────────────────────────────────────────────────────────────────

  $: tags = $appConfig.tuning_tags

  $: filteredFiles = files.filter(f => {
    const t = effectiveTag(f.canonical_name)
    if (tagFilter !== '' && t !== tagFilter) return false
    return true
  })

  // Split filtered files into toggleable and unknown-prefix groups
  $: knownFiles = filteredFiles.filter(f => f.toggleable)
  $: unknownFiles = filteredFiles.filter(f => !f.toggleable)

  $: filteredEntries = entries.filter(e => {
    const matchCat = !categoryFilter || categoryForSetting(e.setting) === categoryFilter
    const q = searchQuery.toLowerCase()
    const matchSearch = !q ||
      e.setting.toLowerCase().includes(q) ||
      e.prototype.toLowerCase().includes(q)
    return matchCat && matchSearch
  })

  $: categories = [...new Set(entries.map(e => categoryForSetting(e.setting)))].sort()

  $: dirty = entries.some((e, i) => {
    const s = savedEntries[i]
    return !s || e.value !== s.value
  }) || entries.length !== savedEntries.length

  $: aggregates = (() => {
    const total = files.length
    const active = files.filter(f => f.enabled).length
    const events = files.filter(f => effectiveTag(f.canonical_name) === 'event')
    const activeEvents = events.filter(f => f.enabled).length
    return { total, active, events: events.length, activeEvents }
  })()

  // ── Tag helpers ────────────────────────────────────────────────────────────

  function effectiveTag(canonical: string): Tag {
    if (tags[canonical]) return tags[canonical] as Tag
    if (KNOWN_CORE.has(canonical)) return 'core'
    if (KNOWN_EVENTS.has(canonical)) return 'event'
    return ''
  }

  async function setTag(canonical: string, tag: Tag) {
    const updated = { ...tags }
    if (tag === '' && effectiveTag(canonical) === '') {
      delete updated[canonical]
    } else {
      updated[canonical] = tag
    }
    await setTuningTags(updated)
    editingTag = null
  }

  // ── Scan ───────────────────────────────────────────────────────────────────

  async function scan() {
    if (!$appConfig.server_exe) return
    scanning = true
    scanError = ''
    try {
      files = await invoke<TuningFileInfo[]>('scan_tuning_files', {
        serverExe: $appConfig.server_exe
      })
      // Re-select if previously selected file still exists
      if (selectedFile) {
        const still = files.find(f => f.canonical_name === selectedFile!.canonical_name)
        if (still) {
          selectedFile = still
        } else {
          selectedFile = null
          entries = []
          savedEntries = []
        }
      }
    } catch (e) {
      scanError = String(e)
    } finally {
      scanning = false
    }
  }

  // ── Select file ────────────────────────────────────────────────────────────

  async function selectFile(file: TuningFileInfo) {
    if (selectedFile?.canonical_name === file.canonical_name) return
    selectedFile = file
    entries = []
    savedEntries = []
    entryError = ''
    categoryFilter = ''
    searchQuery = ''
    loadingEntries = true
    try {
      const loaded = await invoke<TuningEntry[]>('read_tuning_file', {
        serverExe: $appConfig.server_exe,
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

  // ── Toggle file ────────────────────────────────────────────────────────────

  async function toggleFile(file: TuningFileInfo, e: Event) {
    e.stopPropagation()
    const newEnabled = !file.enabled
    try {
      await invoke('toggle_tuning_file', {
        serverExe: $appConfig.server_exe,
        canonicalName: file.canonical_name,
        enabled: newEnabled,
      })
      files = files.map(f =>
        f.canonical_name === file.canonical_name ? { ...f, enabled: newEnabled } : f
      )
      if (selectedFile?.canonical_name === file.canonical_name) {
        selectedFile = { ...selectedFile, enabled: newEnabled }
      }
    } catch (err) {
      scanError = String(err)
    }
  }

  // ── Save entries ───────────────────────────────────────────────────────────

  async function saveEntries() {
    if (!selectedFile) return
    saving = true
    saveError = ''
    saveSuccess = false
    try {
      await invoke('write_tuning_file', {
        serverExe: $appConfig.server_exe,
        canonicalName: selectedFile.canonical_name,
        entries,
      })
      savedEntries = JSON.parse(JSON.stringify(entries))
      saveSuccess = true
      setTimeout(() => saveSuccess = false, 3000)
    } catch (e) {
      saveError = String(e)
    } finally {
      saving = false
    }
  }

  // ── Reload server ──────────────────────────────────────────────────────────

  async function reloadLiveTuning() {
    try {
      await invoke('send_command', { cmd: '!server reloadlivetuning' })
    } catch {}
  }

  // ── Add entry ──────────────────────────────────────────────────────────────

  let newPrototype = ''
  let newSetting = ''
  let newValue = ''

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
    const filtered = filteredEntries[index]
    const globalIndex = entries.indexOf(filtered)
    if (globalIndex !== -1) {
      entries = entries.filter((_, i) => i !== globalIndex)
    }
  }

  function updateValue(entry: TuningEntry, val: string) {
    entry.value = parseFloat(val) || 0
    entries = [...entries]
  }

  onMount(() => {
    if ($appConfig.server_exe) scan()
  })
</script>

<div class="tuning-panel">

  <!-- Left: file list -->
  <div class="file-pane">

    <div class="file-pane-head">
      <div class="section-title">Live Tuning</div>
      <button class="btn-icon" on:click={scan} title="Rescan" disabled={scanning}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <polyline points="23 4 23 10 17 10"/>
          <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/>
        </svg>
      </button>
    </div>

    <!-- Aggregate stats -->
    {#if files.length > 0}
      <div class="stats-row">
        <div class="stat">
          <span class="stat-value">{aggregates.active}</span>
          <span class="stat-label">/ {aggregates.total} active</span>
        </div>
        <div class="stat-sep"></div>
        <div class="stat">
          <span class="stat-value">{aggregates.activeEvents}</span>
          <span class="stat-label">/ {aggregates.events} events</span>
        </div>
      </div>
    {/if}

    <!-- Tag filter chips -->
    {#if files.length > 0}
      <div class="tag-filters">
        <button class="tag-chip" class:active={tagFilter === ''} on:click={() => tagFilter = ''}>All</button>
        <button class="tag-chip tag-core"   class:active={tagFilter === 'core'}   on:click={() => tagFilter = tagFilter === 'core'   ? '' : 'core'  }>Core</button>
        <button class="tag-chip tag-event"  class:active={tagFilter === 'event'}  on:click={() => tagFilter = tagFilter === 'event'  ? '' : 'event' }>Event</button>
        <button class="tag-chip tag-custom" class:active={tagFilter === 'custom'} on:click={() => tagFilter = tagFilter === 'custom' ? '' : 'custom'}>Custom</button>
      </div>
    {/if}

    <!-- File list -->
    <div class="file-list">
      {#if !$appConfig.server_exe}
        <div class="file-notice">Set server exe in Settings to scan files.</div>
      {:else if scanning}
        <div class="file-notice">Scanning...</div>
      {:else if scanError}
        <div class="file-notice error">{scanError}</div>
      {:else if knownFiles.length === 0 && unknownFiles.length === 0}
        <div class="file-notice">No LiveTuning files found.</div>
      {:else}
        {#each knownFiles as file (file.canonical_name)}
          {@const tag = effectiveTag(file.canonical_name)}
          <button
            class="file-item"
            class:selected={selectedFile?.canonical_name === file.canonical_name}
            on:click={() => selectFile(file)}
          >
            <div
              class="file-toggle"
              class:on={file.enabled}
              role="switch"
              aria-checked={file.enabled}
              tabindex="-1"
              on:click={(e) => toggleFile(file, e)}
              on:keydown={(e) => e.key === 'Enter' && toggleFile(file, e)}
            ></div>
            <div class="file-info">
              <span class="file-name">{file.canonical_name.replace(/^LiveTuningData_?/, '').replace(/\.json$/, '') || 'LiveTuningData'}</span>
              {#if editingTag === file.canonical_name}
                <div class="tag-picker" on:click|stopPropagation>
                  {#each (['core', 'event', 'custom', ''] as Tag[]) as t}
                    <button class="tag-opt tag-{t || 'none'}" class:active={tag === t} on:click={() => setTag(file.canonical_name, t)}>
                      {TAG_LABELS[t]}
                    </button>
                  {/each}
                </div>
              {:else}
                <button class="file-tag tag-{tag || 'none'}" on:click|stopPropagation={() => editingTag = file.canonical_name}>
                  {TAG_LABELS[tag]}
                </button>
              {/if}
            </div>
          </button>
        {/each}

        {#if unknownFiles.length > 0}
          <div class="file-group-label">Unknown prefix</div>
          {#each unknownFiles as file (file.canonical_name)}
            {@const tag = effectiveTag(file.canonical_name)}
            <button
              class="file-item"
              class:selected={selectedFile?.canonical_name === file.canonical_name}
              on:click={() => selectFile(file)}
            >
              <div class="file-toggle locked" title="Unknown prefix — cannot toggle"></div>
              <div class="file-info">
                <span class="file-name">{file.canonical_name}</span>
                <span class="file-tag tag-{tag || 'none'}">{TAG_LABELS[tag]}</span>
              </div>
            </button>
          {/each}
        {/if}
      {/if}
    </div>

  </div>

  <!-- Right: entry editor -->
  <div class="entry-pane">

    {#if !selectedFile}
      <!-- Empty state -->
      <div class="entry-empty">
        {#if files.length > 0}
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
            <polyline points="14 2 14 8 20 8"/>
          </svg>
          <span class="entry-empty-label">Select a file</span>
          <span class="entry-empty-sub">{aggregates.total} file{aggregates.total !== 1 ? 's' : ''} found · {aggregates.active} active</span>
        {:else}
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/>
          </svg>
          <span class="entry-empty-label">No files found</span>
          <span class="entry-empty-sub">Check that server exe is set and LiveTuning directory exists</span>
        {/if}
      </div>

    {:else}

      <!-- Entry pane header -->
      <div class="entry-head">
        <div class="entry-head-left">
          <div class="section-title">{selectedFile.canonical_name.replace(/^LiveTuningData_?/, '').replace(/\.json$/, '') || 'LiveTuningData'}</div>
          <span class="entry-filename">{selectedFile.canonical_name}</span>
        </div>
        <div class="entry-head-right">
          {#if saveError}
            <span class="foot-error">{saveError}</span>
          {/if}
          {#if saveSuccess}
            <span class="foot-ok">Saved</span>
          {/if}
          {#if dirty}
            <span class="dirty-indicator">
              <span class="dirty-dot"></span>
              Unsaved
            </span>
          {/if}
          <button
            class="btn btn-sm btn-outline"
            on:click={reloadLiveTuning}
            disabled={!$serverRunning}
            title={$serverRunning ? 'Send !server reloadlivetuning' : 'Server not running'}
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:11px;height:11px;">
              <polyline points="23 4 23 10 17 10"/>
              <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/>
            </svg>
            Reload
          </button>
          <button class="btn btn-sm btn-accent" class:btn-pulse={dirty} on:click={saveEntries} disabled={saving || loadingEntries}>
            {saving ? 'Saving...' : 'Save'}
          </button>
        </div>
      </div>

      <!-- Filters -->
      <div class="entry-toolbar">
        <select class="filter-select" bind:value={categoryFilter}>
          <option value="">All categories</option>
          {#each categories as cat}
            <option value={cat}>{cat}</option>
          {/each}
        </select>
        <div class="search-wrap">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/>
          </svg>
          <input type="text" class="search-input" placeholder="Search setting or prototype..." bind:value={searchQuery}>
        </div>
        <span class="entry-count">{filteredEntries.length} entr{filteredEntries.length !== 1 ? 'ies' : 'y'}</span>
      </div>

      <!-- Entry table -->
      {#if loadingEntries}
        <div class="entry-loading">Loading...</div>
      {:else if entryError}
        <div class="entry-loading error">{entryError}</div>
      {:else}
        <div class="entry-table-wrap">
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
                  <td class="col-cat"><span class="cat-badge">{categoryForSetting(entry.setting)}</span></td>
                  <td class="col-proto" title={entry.prototype}>
                    <span class="proto-text">{entry.prototype || '—'}</span>
                  </td>
                  <td class="col-setting">{entry.setting}</td>
                  <td class="col-value">
                    <input
                      type="number"
                      class="value-input"
                      value={entry.value}
                      on:change={(e) => updateValue(entry, e.currentTarget.value)}
                    >
                  </td>
                  <td class="col-del">
                    <button class="del-btn" on:click={() => removeEntry(i)} title="Remove entry">
                      <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5">
                        <line x1="3" y1="3" x2="11" y2="11"/><line x1="11" y1="3" x2="3" y2="11"/>
                      </svg>
                    </button>
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>

          {#if filteredEntries.length === 0}
            <div class="entry-loading">No entries match the current filter.</div>
          {/if}
        </div>

        <!-- Add entry -->
        <div class="add-entry-row">
          <input type="text" class="add-input proto" placeholder="Prototype (optional)" bind:value={newPrototype}>
          <input type="text" class="add-input setting" placeholder="Setting (e.g. eGTV_XPBonus)" bind:value={newSetting}>
          <input type="number" class="add-input value" placeholder="Value" bind:value={newValue}>
          <button class="btn btn-sm btn-outline" on:click={addEntry} disabled={!newSetting.trim()}>Add</button>
        </div>
      {/if}

    {/if}
  </div>

</div>

<style>
  .tuning-panel {
    display: flex;
    flex: 1;
    overflow: hidden;
    min-height: 0;
  }

  /* ── File pane ── */
  .file-pane {
    width: 220px;
    flex-shrink: 0;
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    background: rgba(8, 9, 12, 0.3);
    overflow: hidden;
  }

  .file-pane-head {
    display: flex;
    align-items: center;
    padding: 12px 14px 10px;
    border-bottom: 1px solid var(--border);
    gap: 8px;
    flex-shrink: 0;
  }
  .file-pane-head .section-title { font-size: 10px; }
  .file-pane-head .btn-icon { margin-left: auto; width: 22px; height: 22px; }
  .file-pane-head .btn-icon svg { width: 12px; height: 12px; }

  .stats-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 14px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .stat {
    display: flex;
    align-items: baseline;
    gap: 3px;
  }

  .stat-value {
    font-family: var(--font-mono);
    font-size: 13px;
    color: var(--accent-bright);
  }

  .stat-label {
    font-family: var(--font-head);
    font-size: 9px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
  }

  .stat-sep {
    width: 1px;
    height: 12px;
    background: var(--border-mid);
  }

  /* Tag filter chips */
  .tag-filters {
    display: flex;
    gap: 4px;
    padding: 8px 10px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    flex-wrap: wrap;
  }

  .tag-chip {
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    padding: 2px 7px;
    border-radius: 2px;
    border: 1px solid var(--border-mid);
    background: transparent;
    color: var(--text-3);
    cursor: pointer;
    transition: all 0.12s;
  }
  .tag-chip:hover { color: var(--text-1); border-color: var(--border-lit); }
  .tag-chip.active { color: var(--text-0); background: var(--bg-3); border-color: var(--border-lit); }
  .tag-chip.tag-core.active   { color: #5dade2; border-color: rgba(46,134,193,0.4); background: var(--blue-dim); }
  .tag-chip.tag-event.active  { color: var(--accent-bright); border-color: var(--accent-dim); background: var(--accent-glow); }
  .tag-chip.tag-custom.active { color: #a080d0; border-color: rgba(130,100,180,0.4); background: rgba(130,100,180,0.1); }

  /* File list */
  .file-list {
    flex: 1;
    overflow-y: auto;
    padding: 6px 0;
  }

  .file-notice {
    padding: 16px 14px;
    font-family: var(--font-head);
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  .file-notice.error { color: #e74c3c; text-transform: none; font-family: var(--font-body); }

  .file-group-label {
    font-family: var(--font-head);
    font-size: 9px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-3);
    padding: 10px 14px 4px;
    border-top: 1px solid var(--border);
    margin-top: 4px;
  }

  .file-item {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 7px 10px;
    width: 100%;
    background: none;
    border: none;
    border-left: 2px solid transparent;
    cursor: pointer;
    text-align: left;
    transition: background 0.1s, border-color 0.1s;
  }
  .file-item:hover { background: var(--bg-2); }
  .file-item.selected { background: var(--accent-glow); border-left-color: var(--accent); }

  .file-toggle {
    width: 28px;
    height: 16px;
    background: var(--bg-0);
    border: 1px solid var(--border-lit);
    border-radius: 8px;
    position: relative;
    cursor: pointer;
    transition: all 0.18s;
    flex-shrink: 0;
    margin-top: 1px;
  }
  .file-toggle::after {
    content: '';
    width: 10px;
    height: 10px;
    background: var(--text-3);
    border-radius: 50%;
    position: absolute;
    top: 2px;
    left: 2px;
    transition: all 0.18s;
  }
  .file-toggle.on {
    background: var(--accent-glow-strong);
    border-color: var(--accent-dim);
  }
  .file-toggle.on::after {
    left: 14px;
    background: var(--accent-bright);
  }
  .file-toggle.locked {
    opacity: 0.3;
    cursor: not-allowed;
    pointer-events: none;
  }

  .file-info {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  .file-name {
    font-family: var(--font-head);
    font-size: 11px;
    font-weight: 600;
    color: var(--text-1);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 140px;
  }
  .file-item.selected .file-name { color: var(--text-0); }

  .file-tag {
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    padding: 1px 5px;
    border-radius: 2px;
    border: 1px solid transparent;
    cursor: pointer;
    background: none;
    transition: all 0.1s;
    align-self: flex-start;
  }
  .file-tag:hover { opacity: 0.8; }

  .tag-picker {
    display: flex;
    gap: 3px;
    flex-wrap: wrap;
  }
  .tag-opt {
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    padding: 2px 5px;
    border-radius: 2px;
    border: 1px solid var(--border-mid);
    background: var(--bg-3);
    color: var(--text-2);
    cursor: pointer;
    transition: all 0.1s;
  }
  .tag-opt.active { color: var(--text-0); }
  .tag-opt:hover { border-color: var(--border-lit); color: var(--text-0); }

  /* Tag colour classes shared by file-tag and tag-opt */
  .tag-core   { color: #5dade2; border-color: rgba(46,134,193,0.3); background: var(--blue-dim); }
  .tag-event  { color: var(--accent-bright); border-color: var(--accent-dim); background: var(--accent-glow); }
  .tag-custom { color: #a080d0; border-color: rgba(130,100,180,0.35); background: rgba(130,100,180,0.1); }
  .tag-none   { color: var(--text-3); border-color: var(--border); }

  /* ── Entry pane ── */
  .entry-pane {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    min-width: 0;
  }

  .entry-empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    color: var(--text-3);
  }
  .entry-empty svg { width: 32px; height: 32px; opacity: 0.3; }
  .entry-empty-label {
    font-family: var(--font-head);
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-2);
  }
  .entry-empty-sub { font-size: 11px; color: var(--text-3); }

  .entry-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 10px 16px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    gap: 12px;
  }

  .entry-head-left {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .entry-head-left .section-title { font-size: 11px; }

  .entry-filename {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-3);
  }

  .entry-head-right {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .dirty-indicator {
    display: flex;
    align-items: center;
    gap: 5px;
    font-family: var(--font-head);
    font-size: 10px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--accent-bright);
  }

  .dirty-dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--accent-bright);
    animation: dirty-pulse 2s ease-in-out infinite;
  }

  @keyframes dirty-pulse {
    0%, 100% { opacity: 1; }
    50%       { opacity: 0.3; }
  }

  .foot-error { font-size: 11px; color: #e74c3c; }
  .foot-ok { font-size: 11px; color: var(--green-bright); font-family: var(--font-head); letter-spacing: 0.08em; }

  .btn-pulse {
    border-color: var(--accent);
    background: rgba(62, 194, 199, 0.2);
  }

  /* Toolbar */
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

  .entry-loading {
    padding: 20px 16px;
    font-family: var(--font-head);
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  .entry-loading.error { color: #e74c3c; text-transform: none; font-family: var(--font-body); }

  /* Entry table */
  .entry-table-wrap {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
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
    padding: 6px 12px;
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
    padding: 6px 12px;
    vertical-align: middle;
  }

  .col-cat   { width: 110px; }
  .col-proto { width: 180px; }
  .col-setting { min-width: 140px; }
  .col-value { width: 100px; }
  .col-del   { width: 32px; }

  .cat-badge {
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-3);
    background: var(--bg-3);
    border: 1px solid var(--border-mid);
    padding: 1px 5px;
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
    max-width: 170px;
  }

  .col-setting {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-1);
  }

  .value-input {
    width: 80px;
    background: var(--bg-0);
    border: 1px solid var(--border-mid);
    color: var(--text-0);
    font-family: var(--font-mono);
    font-size: 12px;
    padding: 3px 6px;
    border-radius: var(--radius-sm);
    outline: none;
    transition: border-color 0.12s;
  }
  .value-input:focus { border-color: var(--accent-dim); }

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
  .del-btn:hover { border-color: rgba(192,57,43,0.4); color: #e74c3c; background: var(--red-dim); }
  .del-btn svg { width: 10px; height: 10px; }

  /* Add entry row */
  .add-entry-row {
    display: flex;
    gap: 6px;
    padding: 8px 12px;
    border-top: 1px solid var(--border);
    background: rgba(8, 9, 12, 0.3);
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
  .add-input.proto    { flex: 2; }
  .add-input.setting  { flex: 3; }
  .add-input.value    { flex: 1; }
  .add-input::placeholder { color: var(--text-3); font-family: var(--font-body); }
</style>