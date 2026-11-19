<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { openPath } from '@tauri-apps/plugin-opener'
  import { activeCaps, appConfig, setUnsaved, askConfirm } from '../../../lib/store'
  import { patchPack, idsOf, namedValue, type PatchFileInfo, type PatchEntry, type PatchNames } from '../../../lib/patchesMeta'
  import PanelSidebar from '../../PanelSidebar.svelte'
  import PatchEditorModal from './PatchEditorModal.svelte'
  import NewFileModal from '../NewFileModal.svelte'
  import { tip } from '../../../lib/tooltip'

  // ── State ──────────────────────────────────────────────────────────────────

  let files: PatchFileInfo[] = []
  let scanning = false
  let scanError = ''
  let openDirError = ''

  let selectedFile: PatchFileInfo | null = null
  let entries: PatchEntry[] = []
  let dirtyEntries: PatchEntry[] = []
  let loadingEntries = false
  let loadError = ''

  let saving = false
  let saveError = ''
  let saveSuccess = ''

  // New file modal open
  let creatingFile = false

  // Modal state — null entry = new, non-null = edit at modalIndex
  let modalOpen  = false
  let modalEntry: PatchEntry | null = null
  let modalIndex: number | null = null

  // File-level enable toggle in flight
  let togglingFile: string | null = null

  // Names for prototype paths and id values, kept across files. `asked` holds every path
  // and id already sent, so ones the catalogue doesn't know aren't sent again.
  let names: PatchNames = { paths: {}, ids: {} }
  const asked = new Set<string>()

  // ── Derived ────────────────────────────────────────────────────────────────

  $: isDirty       = JSON.stringify(entries) !== JSON.stringify(dirtyEntries)
  $: setUnsaved('patches', isDirty ? 'Patches' : null)
  onDestroy(() => setUnsaved('patches', null))
  $: enabledCount  = dirtyEntries.filter(e => e.Enabled).length
  $: disabledCount = dirtyEntries.length - enabledCount
  $: enabledFiles  = files.filter(f => f.enabled)
  $: disabledFiles = files.filter(f => !f.enabled)
  $: requestNames(dirtyEntries)

  // ── Search / sort ──────────────────────────────────────────────────────────

  let searchQuery = ''
  let sortCol: 'desc' | 'proto' | 'path' | 'type' | 'value' | null = null
  let sortDir: 'asc' | 'desc' = 'asc'

  function toggleSort(col: 'desc' | 'proto' | 'path' | 'type' | 'value') {
    if (sortCol === col) {
      if (sortDir === 'asc') { sortDir = 'desc' }
      else { sortCol = null }
    } else {
      sortCol = col
      sortDir = 'asc'
    }
  }

  $: searchLower = searchQuery.toLowerCase()

  // Tag each entry with its stable dirtyEntries index before filtering/sorting
  // so edit operations always target the right element regardless of display order.
  $: indexedEntries = dirtyEntries.map((entry, dirtyIndex) => ({ entry, dirtyIndex }))

  $: filteredIndexed = searchLower
    ? indexedEntries.filter(({ entry: e }) =>
        e.Description.toLowerCase().includes(searchLower) ||
        e.Prototype.toLowerCase().includes(searchLower)   ||
        e.Path.toLowerCase().includes(searchLower)        ||
        (names.paths[e.Prototype] ?? '').toLowerCase().includes(searchLower) ||
        (namedValue(e, names.ids) ?? '').toLowerCase().includes(searchLower)
      )
    : indexedEntries

  $: displayEntries = sortCol === null
    ? filteredIndexed
    : [...filteredIndexed].sort((a, b) => {
        const ea = a.entry, eb = b.entry
        let cmp = 0
        switch (sortCol) {
          case 'desc':  cmp = ea.Description.localeCompare(eb.Description); break
          case 'proto': cmp = ea.Prototype.localeCompare(eb.Prototype);     break
          case 'path':  cmp = ea.Path.localeCompare(eb.Path);               break
          case 'type':  cmp = ea.ValueType.localeCompare(eb.ValueType);     break
          case 'value': cmp = formatValue(ea.Value).localeCompare(formatValue(eb.Value)); break
        }
        return sortDir === 'asc' ? cmp : -cmp
      })

  // ── Helpers ────────────────────────────────────────────────────────────────

  $: displayName = (f: PatchFileInfo) => patchPack(f.file_name, $activeCaps.patchLabels).name

  function formatValue(v: unknown): string {
    if (v === null || v === undefined) return 'null'
    if (typeof v === 'string') return v
    if (typeof v === 'number' || typeof v === 'boolean') return String(v)
    return JSON.stringify(v)
  }

  // ── Backend calls ──────────────────────────────────────────────────────────

  async function requestNames(list: PatchEntry[]) {
    const paths = [...new Set(list.map(e => e.Prototype))].filter(p => p && !asked.has(p))
    const ids   = [...new Set(list.flatMap(idsOf))].filter(id => !asked.has(id))
    if (!$appConfig.server_exe || (paths.length === 0 && ids.length === 0)) return
    for (const k of [...paths, ...ids]) asked.add(k)
    try {
      const got = await invoke<PatchNames>('name_patch_targets', { serverExe: $appConfig.server_exe, paths, ids })
      names = { paths: { ...names.paths, ...got.paths }, ids: { ...names.ids, ...got.ids } }
    } catch {
      // Names are a help, not needed: without Calligraphy.sip the table shows paths and ids.
    }
  }

  async function scan() {
    if (!$appConfig.server_exe) return
    scanning  = true
    scanError = ''
    try {
      files = await invoke<PatchFileInfo[]>('scan_patch_files', { serverExe: $appConfig.server_exe })
    } catch (e) {
      scanError = String(e)
    } finally {
      scanning = false
    }
  }

  async function selectFile(f: PatchFileInfo) {
    if (isDirty && selectedFile && !await askConfirm({
      title: 'Discard changes?',
      message: `${displayName(selectedFile)} has unsaved changes. Discard them?`,
      okLabel: 'Discard',
      cancelLabel: 'Keep editing',
    })) return
    await _doSelectFile(f)
  }

  async function _doSelectFile(f: PatchFileInfo) {
    selectedFile   = f
    modalOpen      = false
    saveError      = ''
    saveSuccess    = ''
    loadingEntries = true
    loadError      = ''
    try {
      entries      = await invoke<PatchEntry[]>('load_patch_file', {
        serverExe: $appConfig.server_exe,
        fileName:  f.file_name,
        enabled:   f.enabled,
      })
      dirtyEntries = entries.map(e => ({ ...e }))
    } catch (e) {
      loadError    = String(e)
      entries      = []
      dirtyEntries = []
    } finally {
      loadingEntries = false
    }
  }

  async function save() {
    if (!selectedFile) return
    saving     = true
    saveError  = ''
    saveSuccess = ''
    try {
      await invoke('save_patch_file', {
        serverExe: $appConfig.server_exe,
        fileName:  selectedFile.file_name,
        enabled:   selectedFile.enabled,
        entries:   dirtyEntries,
      })
      entries     = dirtyEntries.map(e => ({ ...e }))
      saveSuccess = 'Saved.'
      setTimeout(() => saveSuccess = '', 3000)
    } catch (e) {
      saveError = String(e)
    } finally {
      saving = false
    }
  }

  async function toggleFileEnabled(f: PatchFileInfo) {
    togglingFile = f.file_name
    try {
      const newEnabled = await invoke<boolean>('set_patch_file_enabled', {
        serverExe:        $appConfig.server_exe,
        fileName:         f.file_name,
        currentlyEnabled: f.enabled,
      })
      // With a copy in both folders, the switched one replaced the other (kept as .json.bak).
      files = files
        .filter(x => x.file_name !== f.file_name || x.enabled === f.enabled)
        .map(x => x.file_name === f.file_name ? { ...x, enabled: newEnabled } : x)
      if (selectedFile?.file_name === f.file_name) {
        if (selectedFile.enabled === f.enabled) selectedFile = { ...selectedFile, enabled: newEnabled }
        else await _doSelectFile({ ...f, enabled: newEnabled })
      }
    } catch (e) {
      scanError = String(e)
    } finally {
      togglingFile = null
    }
  }

  /** Shows a file the New patch file modal just made. */
  async function handleFileCreated(name: string) {
    creatingFile = false
    await scan()
    const created = files.find(f => f.file_name === name)
    if (created) selectFile(created)
  }

  async function openPatchesDir() {
    openDirError = ''
    try {
      const dir = await invoke<string>('get_patches_dir', { serverExe: $appConfig.server_exe })
      await openPath(dir)
    } catch (e) {
      openDirError = String(e)
    }
  }

  // ── Modal handlers ─────────────────────────────────────────────────────────

  function openAddModal() {
    modalEntry = null
    modalIndex = null
    modalOpen  = true
  }

  function openEditModal(dirtyIndex: number) {
    modalEntry = { ...dirtyEntries[dirtyIndex] }
    modalIndex = dirtyIndex
    modalOpen  = true
  }

  function handleModalSave(saved: PatchEntry) {
    if (modalIndex === null) {
      // New entry
      dirtyEntries = [...dirtyEntries, saved]
    } else {
      // Edit existing
      dirtyEntries = dirtyEntries.map((e, i) => i === modalIndex ? saved : e)
    }
    modalOpen = false
  }

  function handleModalDelete() {
    if (modalIndex === null) return
    dirtyEntries = dirtyEntries.filter((_, i) => i !== modalIndex)
    modalOpen    = false
  }

  // ── Entry reorder ──────────────────────────────────────────────────────────

  function moveEntry(i: number, dir: -1 | 1) {
    const j = i + dir
    if (j < 0 || j >= dirtyEntries.length) return
    const copy = [...dirtyEntries]
    ;[copy[i], copy[j]] = [copy[j], copy[i]]
    dirtyEntries = copy
  }

  onMount(() => { if ($appConfig.server_exe) scan() })
</script>

<div class="patches-panel">
  <div class="panel-bg"></div>
  <div class="grid-overlay"></div>
  <div class="patches-layout">

    <!-- ── Sidebar ── -->
    <PanelSidebar width="var(--sidebar-wide)">
      <svelte:fragment slot="header">
        <div class="section-title">Patch Files</div>
        <button
          class="btn-icon"
          style="margin-left:auto;"
          aria-label="New patch file"
          use:tip={'New patch file'}
          disabled={!$appConfig.server_exe}
          on:click={() => creatingFile = true}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="12" y1="5" x2="12" y2="19"/>
            <line x1="5" y1="12" x2="19" y2="12"/>
          </svg>
        </button>
        <button class="btn-icon" aria-label="Rescan Patches directory" use:tip={'Rescan Patches directory'} disabled={scanning} on:click={scan}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="23 4 23 10 17 10"/>
            <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/>
          </svg>
        </button>
        <button class="btn-icon" aria-label="Open Patches folder" use:tip={'Open Patches folder'} disabled={!$appConfig.server_exe} on:click={openPatchesDir}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
          </svg>
        </button>
      </svelte:fragment>

      <!-- File list -->
      <div class="file-list">
        {#if !$appConfig.server_exe}
          <div class="file-notice">Set server exe in Settings to load patch files.</div>
        {:else if scanning}
          <div class="file-notice">Scanning...</div>
        {:else if scanError}
          <div class="file-notice error">{scanError}</div>
        {:else if openDirError}
          <div class="file-notice error">{openDirError}</div>
        {:else if files.length === 0}
          <div class="file-notice">No PatchData*.json files found.</div>
        {:else}
          {#if enabledFiles.length > 0}
            <div class="side-label">Active</div>
            {#each enabledFiles as f (f.file_name)}
              <div
                class="file-item"
                class:active={selectedFile?.file_name === f.file_name}
                role="button" tabindex="0"
                on:click={() => selectFile(f)}
                on:keydown={e => e.key === 'Enter' && selectFile(f)}
              >
                <span class="file-info"><span class="file-name">{displayName(f)}</span><span class="file-sub">{f.file_name}</span></span>
                <button
                  class="toggle-switch"
                  class:on={f.enabled}
                  role="switch"
                  aria-checked={f.enabled}
                  aria-label="Move to Off/ (disable)"
                  use:tip={'Move to Off/ (disable)'}
                  disabled={togglingFile === f.file_name}
                  on:click|stopPropagation={() => toggleFileEnabled(f)}
                ></button>
              </div>
            {/each}
          {/if}

          {#if disabledFiles.length > 0}
            <div class="side-label">Disabled</div>
            {#each disabledFiles as f (f.file_name)}
              <div
                class="file-item file-item-off"
                class:active={selectedFile?.file_name === f.file_name}
                role="button" tabindex="0"
                on:click={() => selectFile(f)}
                on:keydown={e => e.key === 'Enter' && selectFile(f)}
              >
                <span class="file-info"><span class="file-name">{displayName(f)}</span><span class="file-sub">{f.file_name}</span></span>
                <button
                  class="toggle-switch"
                  class:on={f.enabled}
                  role="switch"
                  aria-checked={f.enabled}
                  aria-label="Move to Patches/ (enable)"
                  use:tip={'Move to Patches/ (enable)'}
                  disabled={togglingFile === f.file_name}
                  on:click|stopPropagation={() => toggleFileEnabled(f)}
                ></button>
              </div>
            {/each}
          {/if}
        {/if}
      </div>
    </PanelSidebar>

    <!-- ── Content pane ── -->
    <section class="data-main">

      {#if !selectedFile}
        <header class="page-head"><h3 class="page-title">Patches</h3></header>
        <div class="empty-state">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
            <polyline points="14 2 14 8 20 8"/>
          </svg>
          <span class="empty-state-label">No file selected</span>
          <span class="empty-state-sub">Select a patch file from the sidebar.</span>
        </div>

      {:else}

        <header class="page-head">
          <h3 class="page-title" use:tip={selectedFile.file_name}>{displayName(selectedFile)}</h3>
          {#if dirtyEntries.length > 0}
            <span class="entry-count">{enabledCount} on · {disabledCount} off</span>
          {/if}
          <div class="page-acts">
          <div class="search-wrap">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="11" cy="11" r="8"/>
              <line x1="21" y1="21" x2="16.65" y2="16.65"/>
            </svg>
            <input
              type="text"
              class="search-input"
              placeholder="Filter entries..."
              bind:value={searchQuery}
            />
          </div>
          <button class="btn btn-sm btn-outline" disabled={!selectedFile} on:click={openAddModal}>New Entry</button>
          </div>
        </header>

        <!-- Entry list -->
        {#if loadingEntries}
          <div class="empty-state"><span class="empty-state-label">Loading...</span></div>
        {:else if loadError}
          <div class="empty-state">
            <span class="empty-state-label error">Load error</span>
            <span class="empty-state-sub">{loadError}</span>
          </div>
        {:else if dirtyEntries.length === 0}
          <div class="empty-state">
            <span class="empty-state-label">No entries</span>
            <span class="empty-state-sub">Add the first patch entry with the button above.</span>
          </div>
        {:else if displayEntries.length === 0}
          <div class="empty-state">
            <span class="empty-state-label">No results</span>
            <span class="empty-state-sub">No entries match "{searchQuery}".</span>
          </div>
        {:else}
          <div class="list-wrap">
            <table class="entry-list">
              <thead>
                <tr>
                  <th class="col-on">On</th>
                  <th class="col-desc sortable" on:click={() => toggleSort('desc')}>
                    Description
                    <span class="sort-ind" class:active={sortCol === 'desc'}>
                      {sortCol === 'desc' ? (sortDir === 'asc' ? '↑' : '↓') : '↕'}
                    </span>
                  </th>
                  <th class="col-proto sortable" on:click={() => toggleSort('proto')}>
                    Prototype
                    <span class="sort-ind" class:active={sortCol === 'proto'}>
                      {sortCol === 'proto' ? (sortDir === 'asc' ? '↑' : '↓') : '↕'}
                    </span>
                  </th>
                  <th class="col-path sortable" on:click={() => toggleSort('path')}>
                    Path
                    <span class="sort-ind" class:active={sortCol === 'path'}>
                      {sortCol === 'path' ? (sortDir === 'asc' ? '↑' : '↓') : '↕'}
                    </span>
                  </th>
                  <th class="col-type sortable" on:click={() => toggleSort('type')}>
                    Type
                    <span class="sort-ind" class:active={sortCol === 'type'}>
                      {sortCol === 'type' ? (sortDir === 'asc' ? '↑' : '↓') : '↕'}
                    </span>
                  </th>
                  <th class="col-value sortable" on:click={() => toggleSort('value')}>
                    Value
                    <span class="sort-ind" class:active={sortCol === 'value'}>
                      {sortCol === 'value' ? (sortDir === 'asc' ? '↑' : '↓') : '↕'}
                    </span>
                  </th>
                  <th class="col-actions"></th>
                </tr>
              </thead>
              <tbody>
                {#each displayEntries as { entry, dirtyIndex } (dirtyIndex)}
                  <tr
                    class="list-row"
                    class:row-disabled={!entry.Enabled}
                    role="button"
                    tabindex="0"
                    on:click={() => openEditModal(dirtyIndex)}
                    on:keydown={e => e.key === 'Enter' && openEditModal(dirtyIndex)}
                  >
                    <td>
                      <button
                        class="toggle-switch row-toggle"
                        class:on={entry.Enabled}
                        role="switch"
                        aria-checked={entry.Enabled}
                        aria-label={entry.Enabled ? 'Disable entry' : 'Enable entry'}
                        use:tip={entry.Enabled ? 'Disable entry' : 'Enable entry'}
                        on:click|stopPropagation={() => {
                          dirtyEntries[dirtyIndex].Enabled = !dirtyEntries[dirtyIndex].Enabled
                          dirtyEntries = dirtyEntries
                        }}
                      ></button>
                    </td>
                    <td class="cell-desc cell-truncate">{entry.Description || '—'}</td>
                    {#if names.paths[entry.Prototype]}
                      <td class="cell-name cell-truncate" use:tip={entry.Prototype}>{names.paths[entry.Prototype]}</td>
                    {:else}
                      <td class="cell-mono cell-truncate">{entry.Prototype || '—'}</td>
                    {/if}
                    <td class="cell-mono cell-truncate">{entry.Path || '—'}</td>
                    <td class="cell-type cell-truncate">{entry.ValueType}</td>
                    {#if namedValue(entry, names.ids)}
                      <td class="cell-name cell-truncate" use:tip={formatValue(entry.Value)}>{namedValue(entry, names.ids)}</td>
                    {:else}
                      <td class="cell-mono cell-truncate">{formatValue(entry.Value)}</td>
                    {/if}
                    <td class="col-actions-cell">
                      <button
                        class="move-btn"
                        use:tip={'Move up'}
                        disabled={sortCol !== null || !!searchLower || dirtyIndex === 0}
                        on:click|stopPropagation={() => moveEntry(dirtyIndex, -1)}
                      >↑</button>
                      <button
                        class="move-btn"
                        use:tip={'Move down'}
                        disabled={sortCol !== null || !!searchLower || dirtyIndex === dirtyEntries.length - 1}
                        on:click|stopPropagation={() => moveEntry(dirtyIndex, 1)}
                      >↓</button>
                    </td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>
        {/if}

        <!-- Footer -->
        <div class="panel-footer">
          {#if isDirty}
            <span class="dirty-badge">Unsaved changes</span>
          {/if}
          {#if saveSuccess}
            <span class="feedback-ok">{saveSuccess}</span>
          {/if}
          {#if saveError}
            <span class="feedback-error">{saveError}</span>
          {/if}
          <button
            class="btn btn-sm btn-accent"
            class:btn-pulse={isDirty}
            style="margin-left:auto;"
            disabled={!isDirty || saving}
            on:click={save}
          >
            {saving ? 'Saving...' : 'Save File'}
          </button>
        </div>

      {/if}
    </section>

  </div><!-- patches-layout -->
</div>

{#if creatingFile}
  <NewFileModal
    title="New patch file"
    prefix="PatchData"
    placeholder="MyPatches"
    hint="Letters, numbers, _ or -. Add entries to it with New Entry."
    create={suffix => invoke<string>('create_patch_file', { serverExe: $appConfig.server_exe, suffix })}
    onCreated={handleFileCreated}
    onClose={() => creatingFile = false}
  />
{/if}

<!-- Modal -->
{#if modalOpen && selectedFile}
  <PatchEditorModal
    entry={modalEntry}
    serverExe={$appConfig.server_exe}
    fileName={selectedFile.file_name}
    onClose={() => modalOpen = false}
    onSave={handleModalSave}
    onDelete={handleModalDelete}
  />
{/if}


<style>
  /* ── Shell ── */
  .patches-panel {
    display: flex;
    flex: 1;
    flex-direction: column;
    position: relative;
    overflow: hidden;
    min-height: 0;
  }

  .patches-layout {
    position: relative;
    z-index: 1;
    flex: 1;
    display: flex;
    overflow: hidden;
    min-height: 0;
  }


  .file-list {
    flex: 1;
    overflow-y: auto;
    padding: 6px;
  }

  .file-notice {
    padding: 16px 14px;
    font-family: var(--font-head);
    font-size: var(--fs-min);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-2);
  }
  .file-notice.error { color: var(--text-error); text-transform: none; font-family: var(--font-body); letter-spacing: 0; }

  .file-list > .side-label:first-child { margin-top: 0; border-top: none; }

  /* Same rows as the Store sidebar. */
  .file-item {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 7px 10px;
    min-height: 42px;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: all 0.1s;
    margin-bottom: 2px;
  }
  .file-item:hover  { background: var(--bg-3); border-color: var(--border-mid); }
  .file-item.active { background: var(--accent-glow); border-color: var(--accent-dim); }
  .file-item-off    { opacity: var(--dimmed-opacity); }

  .file-info { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  .file-name,
  .file-sub {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .file-sub { font-size: var(--fs-min); color: var(--text-3); }
  .file-name {
    font-family: var(--font-head);
    font-size: 11px;
    font-weight: 600;
    color: var(--text-1);
  }
  .file-item.active .file-name { color: var(--accent-bright); }

  .page-title { min-width: 0; overflow: hidden; text-overflow: ellipsis; }

  .entry-count {
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    color: var(--text-2);
    white-space: nowrap;
  }

  .page-acts { flex-shrink: 1; min-width: 0; }
  .search-wrap { min-width: 120px; flex: 0 1 220px; }

  /* ── List view ── */
  .list-wrap { flex: 1; overflow: auto; }

  .entry-list {
    width: 100%;
    border-collapse: collapse;
    font-size: 11px;
  }

  .entry-list thead th {
    font-family: var(--font-head);
    font-size: var(--fs-min);
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-3);
    padding: 6px 10px;
    border-bottom: 1px solid var(--border);
    text-align: left;
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--bg-1);
    white-space: nowrap;
  }

  .sortable {
    cursor: pointer;
    user-select: none;
  }
  .sortable:hover { color: var(--text-1); }

  .sort-ind {
    font-family: var(--font-body);
    font-size: var(--fs-min);
    color: var(--text-3);
    margin-left: 3px;
  }
  .sort-ind.active { color: var(--accent-bright); }

  .col-on      { width: 40px; }
  .col-desc    { width: 18%; }
  .col-proto   { width: 25%; }
  .col-path    { width: 18%; }
  .col-type    { width: 120px; }
  .col-value   { width: auto; }
  .col-actions { width: 64px; }

  .list-row {
    border-bottom: 1px solid var(--border);
    cursor: pointer;
    transition: background 0.08s;
  }
  .list-row:hover { background: var(--bg-2); }
  .list-row.row-disabled { opacity: var(--dimmed-opacity); }

  .entry-list td {
    padding: 6px 10px;
    vertical-align: middle;
    max-width: 0;
  }

  .cell-truncate { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .cell-desc     { color: var(--text-1); }
  .cell-type     { font-size: var(--fs-min); color: var(--text-2); }
  .cell-name     { color: var(--text-1); }
  .cell-mono     { font-family: var(--font-mono); font-size: var(--fs-min); color: var(--text-2); }

  .col-actions-cell {
    display: flex;
    gap: 4px;
    align-items: center;
    max-width: none;
  }

  /* Row-level toggle */
  .row-toggle { width: 28px; height: 16px; flex-shrink: 0; }
  .row-toggle::after { width: 10px; height: 10px; top: 2px; left: 2px; }
  .row-toggle.on::after { left: calc(100% - 12px); }

  /* ── Move buttons ── */
  .move-btn {
    width: 20px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    color: var(--text-3);
    cursor: pointer;
    font-size: var(--fs-min);
    line-height: 1;
    transition: all 0.1s;
    flex-shrink: 0;
  }
  .move-btn:hover:not(:disabled) { color: var(--text-1); border-color: var(--border-lit); }
  .move-btn:disabled { opacity: 0.25; cursor: not-allowed; }

</style>