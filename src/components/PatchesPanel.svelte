<script lang="ts">
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { openPath } from '@tauri-apps/plugin-opener'
  import { appConfig } from '../lib/store'
  import PanelSidebar from './PanelSidebar.svelte'
  import PatchEditorModal from './PatchEditorModal.svelte'

  // ── Types ──────────────────────────────────────────────────────────────────

  interface PatchFileInfo {
    file_name: string
    enabled: boolean
  }

  interface PatchEntry {
    Enabled: boolean
    Prototype: string
    Path: string
    Description: string
    ValueType: string
    Value: unknown
  }

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

  // New file creation
  let creatingFile = false
  let newFileName = ''
  let createError = ''

  // Modal state — null entry = new, non-null = edit at modalIndex
  let modalOpen  = false
  let modalEntry: PatchEntry | null = null
  let modalIndex: number | null = null

  // Confirm-discard dialog — holds the target file to switch to
  let confirmPending: PatchFileInfo | null = null

  // File-level enable toggle in flight
  let togglingFile: string | null = null

  // ── Derived ────────────────────────────────────────────────────────────────

  $: isDirty       = JSON.stringify(entries) !== JSON.stringify(dirtyEntries)
  $: enabledCount  = dirtyEntries.filter(e => e.Enabled).length
  $: disabledCount = dirtyEntries.length - enabledCount
  $: activeFiles   = files.filter(f => f.enabled).length
  $: enabledFiles  = files.filter(f => f.enabled)
  $: disabledFiles = files.filter(f => !f.enabled)

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
        e.Path.toLowerCase().includes(searchLower)
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

  function displayName(f: PatchFileInfo): string {
    return f.file_name.replace(/^PatchData/, '').replace(/\.json$/, '') || f.file_name
  }

  function formatValue(v: unknown): string {
    if (v === null || v === undefined) return 'null'
    if (typeof v === 'string') return v
    if (typeof v === 'number' || typeof v === 'boolean') return String(v)
    return JSON.stringify(v)
  }

  // ── Backend calls ──────────────────────────────────────────────────────────

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
    if (isDirty && selectedFile) {
      confirmPending = f
      return
    }
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

  async function confirmDiscard() {
    const target   = confirmPending
    confirmPending = null
    if (target) await _doSelectFile(target)
  }

  function cancelDiscard() {
    confirmPending = null
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
      files = files.map(x => x.file_name === f.file_name ? { ...x, enabled: newEnabled } : x)
      if (selectedFile?.file_name === f.file_name) {
        selectedFile = { ...selectedFile, enabled: newEnabled }
      }
    } catch (e) {
      scanError = String(e)
    } finally {
      togglingFile = null
    }
  }

  async function createFile() {
    createError = ''
    const name     = newFileName.trim()
    if (!name) { createError = 'Name required.'; return }
    const canonical = name.startsWith('PatchData') ? name : `PatchData${name}`
    const withExt   = canonical.endsWith('.json')  ? canonical : `${canonical}.json`
    try {
      await invoke('create_patch_file', { serverExe: $appConfig.server_exe, fileName: withExt })
      newFileName  = ''
      creatingFile = false
      await scan()
      const created = files.find(f => f.file_name === withExt)
      if (created) selectFile(created)
    } catch (e) {
      createError = String(e)
    }
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

<svelte:window on:keydown={e => { if (e.key === 'Escape' && confirmPending) cancelDiscard() }} />

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
          title="New patch file"
          disabled={!$appConfig.server_exe}
          on:click={() => { creatingFile = !creatingFile; createError = '' }}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="12" y1="5" x2="12" y2="19"/>
            <line x1="5" y1="12" x2="19" y2="12"/>
          </svg>
        </button>
        <button class="btn-icon" title="Rescan Patches directory" disabled={scanning} on:click={scan}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="23 4 23 10 17 10"/>
            <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/>
          </svg>
        </button>
        <button class="btn-icon" title="Open Patches folder" disabled={!$appConfig.server_exe} on:click={openPatchesDir}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
          </svg>
        </button>
      </svelte:fragment>

      <!-- New file form -->
      {#if creatingFile}
        <div class="new-file-form">
          <input
            type="text"
            placeholder="Name (e.g. MyPatches)"
            bind:value={newFileName}
            on:keydown={e => { if (e.key === 'Enter') createFile(); if (e.key === 'Escape') creatingFile = false }}
          />
          {#if createError}
            <span class="feedback-error">{createError}</span>
          {/if}
          <div class="new-file-actions">
            <button class="btn btn-sm btn-accent" on:click={createFile}>Create</button>
            <button class="btn btn-sm btn-outline" on:click={() => { creatingFile = false; createError = '' }}>Cancel</button>
          </div>
        </div>
      {/if}

      <!-- File stats -->
      {#if files.length > 0}
        <div class="stats-row">
          <div class="stat">
            <span class="stat-value">{activeFiles}</span>
            <span class="stat-label">/ {files.length} active</span>
          </div>
        </div>
      {/if}

      <!-- Restart notice -->
      <!-- <div class="restart-notice">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="12" cy="12" r="10"/>
          <line x1="12" y1="8" x2="12" y2="12"/>
          <line x1="12" y1="16" x2="12.01" y2="16"/>
        </svg>
        Patch changes require a server restart to take effect.
      </div> -->

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
            <div class="file-group-label">Active</div>
            {#each enabledFiles as f (f.file_name)}
              <div
                class="file-item"
                class:active={selectedFile?.file_name === f.file_name}
                role="button" tabindex="0"
                on:click={() => selectFile(f)}
                on:keydown={e => e.key === 'Enter' && selectFile(f)}
              >
                <div class="file-info">
                  <span class="file-name">{displayName(f)}</span>
                </div>
                <button
                  class="toggle-switch file-toggle"
                  class:on={f.enabled}
                  title="Move to Off/ (disable)"
                  disabled={togglingFile === f.file_name}
                  on:click|stopPropagation={() => toggleFileEnabled(f)}
                ></button>
              </div>
            {/each}
          {/if}

          {#if disabledFiles.length > 0}
            <div class="file-group-label">Disabled</div>
            {#each disabledFiles as f (f.file_name)}
              <div
                class="file-item file-item-off"
                class:active={selectedFile?.file_name === f.file_name}
                role="button" tabindex="0"
                on:click={() => selectFile(f)}
                on:keydown={e => e.key === 'Enter' && selectFile(f)}
              >
                <div class="file-info">
                  <span class="file-name">{displayName(f)}</span>
                </div>
                <button
                  class="toggle-switch file-toggle"
                  class:on={f.enabled}
                  title="Move to Patches/ (enable)"
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
    <div class="content-pane">

      {#if !selectedFile}
        <div class="empty-state">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
            <polyline points="14 2 14 8 20 8"/>
          </svg>
          <span class="empty-state-label">No file selected</span>
          <span class="empty-state-sub">Select a patch file from the sidebar.</span>
        </div>

      {:else}

        <!-- Toolbar -->
        <div class="toolbar">
          <span class="toolbar-filename">{displayName(selectedFile)}</span>
          {#if dirtyEntries.length > 0}
            <span class="entry-count">{enabledCount} on · {disabledCount} off</span>
          {/if}
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
          <button class="btn btn-sm btn-outline" disabled={!selectedFile} on:click={openAddModal}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="12" y1="5" x2="12" y2="19"/>
              <line x1="5" y1="12" x2="19" y2="12"/>
            </svg>
            Add Entry
          </button>
        </div>

        <!-- Entry list -->
        {#if loadingEntries}
          <div class="empty-state"><span class="empty-state-label">Loading...</span></div>
        {:else if loadError}
          <div class="empty-state">
            <span class="empty-state-label" style="color:var(--text-error);">Load error</span>
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
                        title={entry.Enabled ? 'Disable entry' : 'Enable entry'}
                        on:click|stopPropagation={() => {
                          dirtyEntries[dirtyIndex].Enabled = !dirtyEntries[dirtyIndex].Enabled
                          dirtyEntries = dirtyEntries
                        }}
                      ></button>
                    </td>
                    <td class="cell-desc cell-truncate">{entry.Description || '—'}</td>
                    <td class="cell-mono cell-truncate">{entry.Prototype || '—'}</td>
                    <td class="cell-mono cell-truncate">{entry.Path || '—'}</td>
                    <td><span class="type-badge">{entry.ValueType}</span></td>
                    <td class="cell-mono cell-truncate">{formatValue(entry.Value)}</td>
                    <td class="col-actions-cell">
                      <button
                        class="move-btn"
                        title="Move up"
                        disabled={sortCol !== null || !!searchLower || dirtyIndex === 0}
                        on:click|stopPropagation={() => moveEntry(dirtyIndex, -1)}
                      >↑</button>
                      <button
                        class="move-btn"
                        title="Move down"
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
    </div><!-- content-pane -->

  </div><!-- patches-layout -->
</div>

<!-- Modal -->
{#if modalOpen && selectedFile}
  <PatchEditorModal
    entry={modalEntry}
    serverExe={$appConfig.server_exe}
    onClose={() => modalOpen = false}
    onSave={handleModalSave}
    onDelete={handleModalDelete}
  />
{/if}

<!-- Confirm discard dialog -->
{#if confirmPending && selectedFile}
  <div
    class="confirm-backdrop"
    role="presentation"
    on:click={cancelDiscard}
  >
    <div
      class="confirm-dialog"
      role="alertdialog"
      aria-modal="true"
      tabindex="-100"
      on:click|stopPropagation
      on:keydown={(e) => {
      if (e.key === 'Escape' || e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        cancelDiscard();
      }
    }}
    >
      <div class="confirm-icon">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/>
          <line x1="12" y1="9" x2="12" y2="13"/>
          <line x1="12" y1="17" x2="12.01" y2="17"/>
        </svg>
      </div>
      <div class="confirm-body">
        <p class="confirm-message">
          Discard unsaved changes to
          <strong class="confirm-filename">{displayName(selectedFile)}</strong>?
        </p>
        <p class="confirm-sub">This cannot be undone.</p>
      </div>
      <div class="confirm-actions">
        <button class="btn btn-sm btn-outline" tabindex="-1" on:click={cancelDiscard}>Cancel</button>
        <button class="btn btn-sm btn-red" on:click={confirmDiscard}>Discard</button>
      </div>
    </div>
  </div>
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

  .panel-bg {
    position: absolute;
    inset: 0;
    background: linear-gradient(180deg, var(--panel-grad-top) 0%, transparent var(--panel-grad-mid));
    pointer-events: none;
  }

  .grid-overlay {
    position: absolute;
    inset: 0;
    background-image:
      linear-gradient(var(--border) 1px, transparent 1px),
      linear-gradient(90deg, var(--border) 1px, transparent 1px);
    background-size: 32px 32px;
    opacity: 0.18;
    pointer-events: none;
  }

  .patches-layout {
    position: relative;
    z-index: 1;
    flex: 1;
    display: flex;
    overflow: hidden;
    min-height: 0;
  }

  /* ── Sidebar internals ── */
  .new-file-form {
    padding: 8px 10px;
    border-bottom: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .new-file-actions { display: flex; gap: 6px; }

  /* .restart-notice {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    padding: 8px 12px 10px;
    font-size: 10px;
    color: var(--text-3);
    border-bottom: 1px solid var(--border);
    line-height: 1.4;
  } */
  /* .restart-notice svg { width: 12px; height: 12px; flex-shrink: 0; margin-top: 1px; } */

  /* ── Sidebar stats ── */
  .stats-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 14px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .stat { display: flex; align-items: baseline; gap: 3px; }

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

  .file-list {
    flex: 1;
    overflow-y: auto;
    padding: 6px;
  }

  .file-notice {
    padding: 16px 14px;
    font-family: var(--font-head);
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  .file-notice.error { color: var(--text-error); text-transform: none; font-family: var(--font-body); letter-spacing: 0; }

  .file-item {
    display: flex;
    align-items: center;
    gap: 8px;
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
  .file-item.active .file-name { color: var(--accent-bright); }
  .file-item-off    { opacity: 0.55; }

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

  .file-info { flex: 1; display: flex; flex-direction: column; gap: 2px; min-width: 0; }

  .file-name {
    font-family: var(--font-head);
    font-size: 11px;
    font-weight: 600;
    color: var(--text-1);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .file-toggle { width: 28px; height: 16px; flex-shrink: 0; }
  .file-toggle::after { width: 10px; height: 10px; top: 2px; left: 2px; }
  .file-toggle.on::after { left: calc(100% - 12px); }

  /* ── Content pane ── */
  .content-pane {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    min-width: 0;
    background: var(--bg-1);
  }

  /* ── Toolbar ── */
  .toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 14px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    min-height: 48px;
  }

  .toolbar-filename {
    font-family: var(--font-head);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.08em;
    color: var(--text-1);
    flex-shrink: 0;
  }

  .entry-count {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-3);
    flex-shrink: 0;
  }

  .search-wrap {
    display: flex;
    align-items: center;
    gap: 6px;
    background: var(--bg-0);
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    padding: 0 8px;
    flex: 1;
    min-width: 0;
    max-width: 300px;
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
    width: auto;
    border-radius: 0;
  }
  .search-input::placeholder { color: var(--text-3); font-family: var(--font-body); }

  /* ── List view ── */
  .list-wrap { flex: 1; overflow: auto; }

  .entry-list {
    width: 100%;
    border-collapse: collapse;
    font-size: 11px;
  }

  .entry-list thead th {
    font-family: var(--font-head);
    font-size: 9px;
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
    font-size: 9px;
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
  .list-row.row-disabled { opacity: 0.5; }

  .entry-list td {
    padding: 6px 10px;
    vertical-align: middle;
    max-width: 0;
  }

  .cell-truncate { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .cell-desc     { color: var(--text-1); }
  .cell-mono     { font-family: var(--font-mono); font-size: 10px; color: var(--text-2); }

  .type-badge {
    font-family: var(--font-head);
    font-size: 8px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--accent-dim);
    background: var(--accent-glow);
    border: 1px solid var(--accent-dim);
    border-radius: 2px;
    padding: 1px 5px;
    white-space: nowrap;
  }

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
    font-size: 10px;
    line-height: 1;
    transition: all 0.1s;
    flex-shrink: 0;
  }
  .move-btn:hover:not(:disabled) { color: var(--text-1); border-color: var(--border-lit); }
  .move-btn:disabled { opacity: 0.25; cursor: not-allowed; }

  /* ── Confirm discard dialog ── */
  .confirm-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: var(--z-modal);
    backdrop-filter: blur(2px);
  }

  .confirm-dialog {
    background: var(--bg-2);
    border: 1px solid var(--border-lit);
    border-radius: var(--radius-md);
    padding: 20px 24px;
    width: min(380px, 90vw);
    display: flex;
    flex-direction: column;
    gap: 16px;
    box-shadow: 0 24px 80px rgba(0, 0, 0, 0.55);
  }

  .confirm-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    border-radius: 50%;
    background: var(--amber-dim);
    border: 1px solid rgba(200, 146, 10, 0.25);
    flex-shrink: 0;
  }
  .confirm-icon svg {
    width: 18px;
    height: 18px;
    color: var(--amber-bright);
  }

  .confirm-body {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .confirm-message {
    font-family: var(--font-body);
    font-size: 13px;
    color: var(--text-0);
    line-height: 1.4;
  }

  .confirm-filename {
    font-family: var(--font-mono);
    font-weight: 500;
    color: var(--accent-bright);
  }

  .confirm-sub {
    font-size: 11px;
    color: var(--text-3);
  }

  .confirm-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
</style>