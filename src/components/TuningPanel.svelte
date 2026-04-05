<script lang="ts">
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { openPath } from '@tauri-apps/plugin-opener'
  import { appConfig, serverRunning, setTuningTags, setTuningFavourites } from '../lib/store'
  import { categoryForSetting, KNOWN_CORE, KNOWN_EVENTS, CATEGORY_PREFIXES } from '../lib/tuningMeta'
  import PanelSidebar from './PanelSidebar.svelte'
  import TuningEditorModal from './TuningEditorModal.svelte'

  // ── Types ──────────────────────────────────────────────────────────────────

  interface TuningFileInfo {
    canonical_name: string
    enabled: boolean
    toggleable: boolean
  }

  type Tag = 'core' | 'event' | 'custom' | ''

  const TAG_LABELS: Record<string, string> = {
    core:   'Core',
    event:  'Event',
    custom: 'Custom',
    '':     'Untagged',
  }

  // ── State ──────────────────────────────────────────────────────────────────

  let files: TuningFileInfo[] = []
  let scanError = ''
  let scanning = false

  let editingFile: TuningFileInfo | null = null
  let creatingNew = false
  let tagFilter: Tag | '' = ''
  let searchQuery = ''
  let editingTag: string | null = null

  // ── Derived ────────────────────────────────────────────────────────────────

  $: tags = $appConfig.tuning_tags ?? {}
  $: favourites = $appConfig.tuning_favourites ?? []
  $: existingNames = files.map(f => f.canonical_name)

  $: searchLower = searchQuery.toLowerCase()

  // Pinned grid: favourited files, search-filtered, tag-filter ignored
  $: favouriteGrid = files.filter(f => {
    if (!favourites.includes(f.canonical_name)) return false
    if (!searchLower) return true
    const name = displayName(f).toLowerCase()
    return f.canonical_name.toLowerCase().includes(searchLower) || name.includes(searchLower)
  })

  // Main grid: non-favourited, tag+search filtered
  $: mainGrid = files.filter(f => {
    if (favourites.includes(f.canonical_name)) return false
    const tag = effectiveTag(f.canonical_name)
    if (tagFilter !== '' && tag !== tagFilter) return false
    if (!searchLower) return true
    const name = displayName(f).toLowerCase()
    return f.canonical_name.toLowerCase().includes(searchLower) || name.includes(searchLower)
  })

  // Sidebar: all files mapped with reactive starred state so {#each} re-renders on favourite changes
  $: knownSidebarFiles = files
    .filter(f => f.toggleable)
    .map(f => ({ ...f, starred: favourites.includes(f.canonical_name) }))

  $: enabledSidebarFiles  = knownSidebarFiles.filter(f => f.enabled)
  $: disabledSidebarFiles = knownSidebarFiles.filter(f => !f.enabled)

  $: unknownSidebarFiles = files
    .filter(f => !f.toggleable)
    .map(f => ({ ...f, starred: favourites.includes(f.canonical_name) }))

  $: aggregates = (() => {
    const total = files.length
    const active = files.filter(f => f.enabled).length
    const eventFiles = files.filter(
      f => (tags[f.canonical_name] || (KNOWN_CORE.has(f.canonical_name) ? 'core' : KNOWN_EVENTS.has(f.canonical_name) ? 'event' : '')) === 'event'
    )
    return {
      total,
      active,
      events: eventFiles.length,
      activeEvents: eventFiles.filter(f => f.enabled).length,
    }
  })()

  // ── Helpers ────────────────────────────────────────────────────────────────

  function displayName(file: TuningFileInfo): string {
    return file.canonical_name
      .replace(/^LiveTuningData_?/, '')
      .replace(/\.json$/, '') || 'LiveTuningData'
  }

  function effectiveTag(canonical: string): Tag {
    if (tags[canonical]) return tags[canonical] as Tag
    if (KNOWN_CORE.has(canonical)) return 'core'
    if (KNOWN_EVENTS.has(canonical)) return 'event'
    return ''
  }

  // ── Tag management ─────────────────────────────────────────────────────────

  async function setTag(canonical: string, tag: Tag) {
    const updated = { ...tags }
    if (tag === '') {
      delete updated[canonical]
    } else {
      updated[canonical] = tag
    }
    await setTuningTags(updated)
    editingTag = null
  }

  // ── Favourites ─────────────────────────────────────────────────────────────

  async function toggleFavourite(file: { canonical_name: string }) {
    const updated = favourites.includes(file.canonical_name)
      ? favourites.filter(f => f !== file.canonical_name)
      : [...favourites, file.canonical_name]
    await setTuningFavourites(updated)
  }

  // ── Scan ───────────────────────────────────────────────────────────────────

  async function scan() {
    if (!$appConfig.server_exe) return
    scanning = true
    scanError = ''
    try {
      files = await invoke<TuningFileInfo[]>('scan_tuning_files', {
        serverExe: $appConfig.server_exe,
      })
      // Keep modal in sync if the file it's editing was rescanned
      if (editingFile) {
        const still = files.find(f => f.canonical_name === editingFile!.canonical_name)
        editingFile = still ?? null
      }
    } catch (e) {
      scanError = String(e)
    } finally {
      scanning = false
    }
  }

  async function handleCreated(canonicalName: string) {
    creatingNew = false
    await scan()
    const created = files.find(f => f.canonical_name === canonicalName)
    if (created) editingFile = created
  }

  // ── Toggle file ────────────────────────────────────────────────────────────

  async function toggleFile(file: TuningFileInfo) {
    if (!file.toggleable) return
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
    } catch (err) {
      scanError = String(err)
    }
  }

  // ── Reload ─────────────────────────────────────────────────────────────────

  async function reloadLiveTuning() {
    try {
      await invoke('send_command', { cmd: '!server reloadlivetuning' })
    } catch {}
  }

  let openDirError = ''

  async function openLiveTuningDir() {
    if (!$appConfig.server_exe) return
    openDirError = ''
    try {
      const dir = await invoke<string>('get_live_tuning_dir', { serverExe: $appConfig.server_exe })
      await openPath(dir)
    } catch (e) {
      openDirError = String(e)
    }
  }

  // ── Click outside (tag picker) ─────────────────────────────────────────────

  function handleClickOutside(e: MouseEvent) {
    if (editingTag && !(e.target as Element).closest('.card-tag-picker, .card-tag-btn')) {
      editingTag = null
    }
  }

  onMount(() => {
    if ($appConfig.server_exe) scan()
  })
</script>

<div class="tuning-panel" role="presentation" on:click={handleClickOutside}>
  <div class="panel-bg"></div>
  <div class="grid-overlay"></div>
  <div class="tuning-layout">

  <!-- Left: sidebar -->
  <PanelSidebar width="var(--sidebar-wide)">
    <svelte:fragment slot="header">
      <div class="section-title">Tuning Files</div>
      <button
        class="btn-icon"
        on:click={() => { creatingNew = true; editingFile = null }}
        title="Create new tuning file"
        disabled={!$appConfig.server_exe}
        style="margin-left:auto;"
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="12" y1="5" x2="12" y2="19"/>
          <line x1="5" y1="12" x2="19" y2="12"/>
        </svg>
      </button>
      <button
        class="btn-icon"
        on:click={scan}
        title="Rescan LiveTuning directory"
        disabled={scanning}
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <polyline points="23 4 23 10 17 10"/>
          <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/>
        </svg>
      </button>
      <button
        class="btn-icon"
        on:click={openLiveTuningDir}
        title="Open LiveTuning folder"
        disabled={!$appConfig.server_exe}
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
        </svg>
      </button>
    </svelte:fragment>

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

    <!-- File list (unfiltered — full navigation list) -->
    <div class="file-list">
      {#if !$appConfig.server_exe}
        <div class="file-notice">Set server exe in Settings to scan files.</div>
      {:else if scanning}
        <div class="file-notice">Scanning...</div>
      {:else if scanError}
        <div class="file-notice error">{scanError}</div>
      {:else if openDirError}
        <div class="file-notice error">{openDirError}</div>
      {:else if files.length === 0}
        <div class="file-notice">No LiveTuning files found.</div>
      {:else}
        {#if enabledSidebarFiles.length > 0}
          <div class="file-group-label">Active</div>
          {#each enabledSidebarFiles as file (file.canonical_name)}
            {@const tag = effectiveTag(file.canonical_name)}
            <div
              class="file-item"
              class:editing={editingFile?.canonical_name === file.canonical_name}
              role="button"
              tabindex="0"
              aria-label="Edit {file.canonical_name}"
              on:click={() => editingFile = file}
              on:keydown={e => e.key === 'Enter' && (editingFile = file)}
            >
              <button
                class="star-btn"
                class:starred={file.starred}
                aria-label={file.starred ? 'Remove from favourites' : 'Add to favourites'}
                title={file.starred ? 'Remove from favourites' : 'Add to favourites'}
                on:click|stopPropagation={() => toggleFavourite(file)}
              >
                {#if file.starred}
                  <svg viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" stroke-width="1.5">
                    <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/>
                  </svg>
                {:else}
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                    <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/>
                  </svg>
                {/if}
              </button>
              <div class="file-info">
                <span class="file-name">{displayName(file)}</span>
                {#if tag}
                  <span class="file-tag tag-{tag}">{TAG_LABELS[tag]}</span>
                {/if}
              </div>
            </div>
          {/each}
        {/if}

        {#if disabledSidebarFiles.length > 0}
          <div class="file-group-label">Disabled</div>
          {#each disabledSidebarFiles as file (file.canonical_name)}
            {@const tag = effectiveTag(file.canonical_name)}
            <div
              class="file-item file-item-off"
              class:editing={editingFile?.canonical_name === file.canonical_name}
              role="button"
              tabindex="0"
              aria-label="Edit {file.canonical_name}"
              on:click={() => editingFile = file}
              on:keydown={e => e.key === 'Enter' && (editingFile = file)}
            >
              <button
                class="star-btn"
                class:starred={file.starred}
                aria-label={file.starred ? 'Remove from favourites' : 'Add to favourites'}
                on:click|stopPropagation={() => toggleFavourite(file)}
              >
                {#if file.starred}
                  <svg viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" stroke-width="1.5">
                    <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/>
                  </svg>
                {:else}
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                    <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/>
                  </svg>
                {/if}
              </button>
              <div class="file-info">
                <span class="file-name">{displayName(file)}</span>
                {#if tag}
                  <span class="file-tag tag-{tag}">{TAG_LABELS[tag]}</span>
                {/if}
              </div>
            </div>
          {/each}
        {/if}

        {#if unknownSidebarFiles.length > 0}
          <div class="file-group-label">Unknown prefix</div>
          {#each unknownSidebarFiles as file (file.canonical_name)}
            {@const tag = effectiveTag(file.canonical_name)}
            <div
              class="file-item"
              class:editing={editingFile?.canonical_name === file.canonical_name}
              role="button"
              tabindex="0"
              aria-label="Edit {file.canonical_name}"
              on:click={() => editingFile = file}
              on:keydown={e => e.key === 'Enter' && (editingFile = file)}
            >
              <button
                class="star-btn"
                class:starred={file.starred}
                aria-label={file.starred ? 'Remove from favourites' : 'Add to favourites'}
                on:click|stopPropagation={() => toggleFavourite(file)}
              >
                {#if file.starred}
                  <svg viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" stroke-width="1.5">
                    <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/>
                  </svg>
                {:else}
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                    <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/>
                  </svg>
                {/if}
              </button>
              <div class="file-info">
                <span class="file-name">{file.canonical_name}</span>
                {#if tag}
                  <span class="file-tag tag-{tag}">{TAG_LABELS[tag]}</span>
                {/if}
              </div>
            </div>
          {/each}
        {/if}
      {/if}
    </div>
  </PanelSidebar>

  <!-- Right: grid -->
  <div class="grid-pane">

    <!-- Grid header: search + count + reload -->
    <div class="grid-header">
      <div class="search-wrap">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="11" cy="11" r="8"/>
          <line x1="21" y1="21" x2="16.65" y2="16.65"/>
        </svg>
        <input
          type="text"
          class="search-input"
          placeholder="Filter files..."
          bind:value={searchQuery}
        >
      </div>
      <span class="file-count">
        {favouriteGrid.length + mainGrid.length} file{favouriteGrid.length + mainGrid.length !== 1 ? 's' : ''}
      </span>
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
        Reload Live Tuning
      </button>
    </div>

    <!-- Tag filter bar -->
    {#if files.length > 0}
      <div class="grid-filter-bar">
        <button class="filter-chip"             class:active={tagFilter === ''}       on:click={() => tagFilter = ''}>All</button>
        <button class="filter-chip chip-blue"   class:active={tagFilter === 'core'}   on:click={() => tagFilter = tagFilter === 'core'   ? '' : 'core'}>Core</button>
        <button class="filter-chip chip-green"  class:active={tagFilter === 'event'}  on:click={() => tagFilter = tagFilter === 'event'  ? '' : 'event'}>Event</button>
        <button class="filter-chip chip-purple" class:active={tagFilter === 'custom'} on:click={() => tagFilter = tagFilter === 'custom' ? '' : 'custom'}>Custom</button>
      </div>
    {/if}
    <div class="grid-scroll">
      {#if !$appConfig.server_exe}
        <div class="empty-state">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <circle cx="12" cy="12" r="10"/>
            <line x1="12" y1="8" x2="12" y2="12"/>
            <line x1="12" y1="16" x2="12.01" y2="16"/>
          </svg>
          <span class="empty-state-label">No server configured</span>
          <span class="empty-state-sub">Set the server exe path in Settings to load tuning files.</span>
        </div>
      {:else if scanning}
        <div class="empty-state">
          <span class="empty-state-label">Scanning...</span>
        </div>
      {:else if scanError}
        <div class="empty-state">
          <span class="empty-state-label" style="color:var(--text-error);">Scan error</span>
          <span class="empty-state-sub">{scanError}</span>
        </div>
      {:else if files.length === 0}
        <div class="empty-state">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <circle cx="11" cy="11" r="8"/>
            <line x1="21" y1="21" x2="16.65" y2="16.65"/>
          </svg>
          <span class="empty-state-label">No files found</span>
          <span class="empty-state-sub">Check that the server exe is set and the LiveTuning directory exists.</span>
        </div>
      {:else}

        <!-- Favourites section -->
        {#if favouriteGrid.length > 0}
          <div class="grid-section-label">
            <svg viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" stroke-width="1" style="width:10px;height:10px;">
              <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/>
            </svg>
            Favourites
          </div>
          <div class="card-grid">
            {#each favouriteGrid as file (file.canonical_name)}
              {@const tag = effectiveTag(file.canonical_name)}
              <div class="tuning-card" class:enabled={file.enabled}>
                <div class="card-body">
                  <div class="card-top">
                    <span class="card-name">{displayName(file)}</span>
                    <button
                      class="star-btn starred"
                      aria-label="Remove from favourites"
                      title="Remove from favourites"
                      on:click|stopPropagation={() => toggleFavourite(file)}
                    >
                      <svg viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" stroke-width="1.5">
                        <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/>
                      </svg>
                    </button>
                  </div>

                  <!-- Tag badge / picker -->
                  {#if editingTag === file.canonical_name}
                    <div class="card-tag-picker">
                      {#each (['core', 'event', 'custom', ''] as Tag[]) as t}
                        <button
                          class="tag-opt tag-{t || 'none'}"
                          class:active={($appConfig.tuning_tags[file.canonical_name] || (KNOWN_CORE.has(file.canonical_name) ? 'core' : KNOWN_EVENTS.has(file.canonical_name) ? 'event' : '')) === t}
                          on:click|stopPropagation={() => setTag(file.canonical_name, t)}
                        >{TAG_LABELS[t]}</button>
                      {/each}
                    </div>
                  {:else if tag}
                    <button
                      class="file-tag tag-{tag} card-tag-btn"
                      title="Change tag"
                      on:click|stopPropagation={() => editingTag = file.canonical_name}
                    >{TAG_LABELS[tag]}</button>
                  {:else}
                    <button
                      class="card-tag-add card-tag-btn"
                      on:click|stopPropagation={() => editingTag = file.canonical_name}
                    >+ Tag</button>
                  {/if}

                  <span class="card-filename">{file.canonical_name}</span>
                </div>
                <div class="card-footer">
                  <div
                    class="file-toggle"
                    class:on={file.enabled}
                    class:locked={!file.toggleable}
                    role="switch"
                    aria-checked={file.enabled}
                    tabindex="0"
                    title={!file.toggleable ? 'Unknown prefix — cannot toggle' : undefined}
                    on:click={() => toggleFile(file)}
                    on:keydown={e => e.key === 'Enter' && toggleFile(file)}
                  ></div>
                  <span class="toggle-label">{file.enabled ? 'Active' : 'Inactive'}</span>
                  <button class="btn btn-sm btn-outline card-edit-btn" on:click={() => editingFile = file}>
                    Edit
                  </button>
                </div>
              </div>
            {/each}
          </div>
        {/if}

        <!-- Main grid -->
        {#if mainGrid.length > 0}
          {#if favouriteGrid.length > 0}
            <div class="grid-section-label">All Files</div>
          {/if}
          <div class="card-grid">
            {#each mainGrid as file (file.canonical_name)}
              {@const tag = effectiveTag(file.canonical_name)}
              <div class="tuning-card" class:enabled={file.enabled}>
                <div class="card-body">
                  <div class="card-top">
                    <span class="card-name">{displayName(file)}</span>
                    <button
                      class="star-btn"
                      aria-label="Add to favourites"
                      title="Add to favourites"
                      on:click|stopPropagation={() => toggleFavourite(file)}
                    >
                      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                        <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/>
                      </svg>
                    </button>
                  </div>

                  <!-- Tag badge / picker -->
                  {#if editingTag === file.canonical_name}
                    <div class="card-tag-picker">
                      {#each (['core', 'event', 'custom', ''] as Tag[]) as t}
                        <button
                          class="tag-opt tag-{t || 'none'}"
                          class:active={($appConfig.tuning_tags[file.canonical_name] || (KNOWN_CORE.has(file.canonical_name) ? 'core' : KNOWN_EVENTS.has(file.canonical_name) ? 'event' : '')) === t}
                          on:click|stopPropagation={() => setTag(file.canonical_name, t)}
                        >{TAG_LABELS[t]}</button>
                      {/each}
                    </div>
                  {:else if tag}
                    <button
                      class="file-tag tag-{tag} card-tag-btn"
                      title="Change tag"
                      on:click|stopPropagation={() => editingTag = file.canonical_name}
                    >{TAG_LABELS[tag]}</button>
                  {:else}
                    <button
                      class="card-tag-add card-tag-btn"
                      on:click|stopPropagation={() => editingTag = file.canonical_name}
                    >+ Tag</button>
                  {/if}

                  <span class="card-filename">{file.canonical_name}</span>
                </div>
                <div class="card-footer">
                  <div
                    class="file-toggle"
                    class:on={file.enabled}
                    class:locked={!file.toggleable}
                    role="switch"
                    aria-checked={file.enabled}
                    tabindex="0"
                    title={!file.toggleable ? 'Unknown prefix — cannot toggle' : undefined}
                    on:click={() => toggleFile(file)}
                    on:keydown={e => e.key === 'Enter' && toggleFile(file)}
                  ></div>
                  <span class="toggle-label">{file.enabled ? 'Active' : 'Inactive'}</span>
                  <button class="btn btn-sm btn-outline card-edit-btn" on:click={() => editingFile = file}>
                    Edit
                  </button>
                </div>
              </div>
            {/each}
          </div>
        {:else if favouriteGrid.length === 0}
          <div class="empty-state">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
              <circle cx="11" cy="11" r="8"/>
              <line x1="21" y1="21" x2="16.65" y2="16.65"/>
            </svg>
            <span class="empty-state-label">No results</span>
            <span class="empty-state-sub">No files match the current filter.</span>
          </div>
        {/if}

      {/if}
    </div>
  </div>

  <!-- Editor modal (edit existing) -->
  {#if editingFile}
    <TuningEditorModal
      file={editingFile}
      serverExe={$appConfig.server_exe}
      serverRunning={$serverRunning}
      existingNames={existingNames}
      onClose={() => editingFile = null}
    />
  {/if}

  <!-- Create new file modal -->
  {#if creatingNew}
    <TuningEditorModal
      file={null}
      serverExe={$appConfig.server_exe}
      serverRunning={$serverRunning}
      existingNames={existingNames}
      onClose={() => creatingNew = false}
      onCreated={handleCreated}
    />
  {/if}

  </div><!-- tuning-layout -->
</div>

<style>
  .tuning-panel {
    display: flex;
    flex: 1;
    flex-direction: column;
    position: relative;
    overflow: hidden;
    min-height: 0;
  }

  .tuning-layout {
    position: relative;
    z-index: 1;
    flex: 1;
    display: flex;
    overflow: hidden;
    min-height: 0;
  }

  /* ── Sidebar internals ── */

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
  .file-notice.error {
    color: var(--text-error);
    text-transform: none;
    font-family: var(--font-body);
  }

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
    align-items: center;
    gap: 6px;
    padding: 6px 8px;
    min-height: 42px;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: all 0.1s;
    margin-bottom: 2px;
  }
  .file-item:hover { background: var(--bg-3); border-color: var(--border-mid); }
  .file-item.editing { background: var(--accent-glow); border-color: var(--accent-dim); }
  .file-item-off { opacity: 0.55; }

  .file-info {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
    flex: 1;
  }

  .file-name {
    font-family: var(--font-head);
    font-size: 11px;
    font-weight: 600;
    color: var(--text-1);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .file-item.editing .file-name { color: var(--accent-bright); }

  .file-tag {
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    padding: 1px 5px;
    border-radius: 2px;
    border: 1px solid transparent;
    background: none;
    align-self: flex-start;
    cursor: pointer;
    transition: opacity 0.1s;
  }

  /* ── Star button ── */
  .star-btn {
    width: 22px;
    height: 22px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: none;
    cursor: pointer;
    color: var(--text-3);
    border-radius: var(--radius-sm);
    transition: color 0.12s;
    flex-shrink: 0;
    padding: 0;
  }
  .star-btn:hover { color: var(--colour-favourite); }
  .star-btn.starred { color: var(--colour-favourite); }
  .star-btn svg { width: 13px; height: 13px; }

  /* ── Tag colour classes ── */
  .tag-core   { color: var(--blue);          border-color: rgba(46,134,193,0.3);   background: var(--blue-dim); }
  .tag-event  { color: var(--green-bright);  border-color: rgba(39,174,96,0.4);    background: var(--green-dim); }
  .tag-custom { color: var(--purple);        border-color: rgba(130,100,180,0.35); background: var(--purple-dim); }
  .tag-none   { color: var(--text-3);        border-color: var(--border); }

  /* ── Grid pane ── */
  .grid-pane {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    min-width: 0;
    background: var(--bg-1);
  }

  .grid-header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 16px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    min-height: 52px;
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
    max-width: 280px;
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

  .file-count {
    font-family: var(--font-head);
    font-size: 9px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-3);
    white-space: nowrap;
  }

  .grid-scroll {
    flex: 1;
    overflow-y: auto;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .grid-section-label {
    display: flex;
    align-items: center;
    gap: 6px;
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--colour-favourite);
    margin-bottom: 8px;
    margin-top: 4px;
  }
  .grid-section-label:first-child { margin-top: 0; }

  /* ── Grid filter bar (tag chips, below grid-header) ── */
  .grid-filter-bar {
    display: flex;
    gap: 4px;
    padding: 7px 16px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    background: var(--chrome-sunken-bg);
  }

  .card-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(196px, 1fr));
    gap: 10px;
    margin-bottom: 16px;
  }

  /* ── Cards ── */
  .tuning-card {
    display: flex;
    flex-direction: column;
    background: var(--bg-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    transition: border-color 0.14s, background 0.14s;
    overflow: hidden;
  }
  .tuning-card:hover { border-color: var(--border-lit); }
  .tuning-card.enabled {
    border-color: var(--accent-dim);
    background: linear-gradient(160deg, var(--accent-glow) 0%, var(--bg-2) 55%);
  }
  .tuning-card.enabled:hover { border-color: var(--accent); }

  .card-body {
    padding: 12px 12px 10px;
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .card-top {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 6px;
  }

  .card-name {
    font-family: var(--font-head);
    font-size: 12px;
    font-weight: 600;
    color: var(--text-0);
    line-height: 1.3;
    word-break: break-word;
  }
  .tuning-card.enabled .card-name { color: var(--accent-bright); }

  .card-filename {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    margin-top: auto;
    padding-top: 4px;
  }

  .card-footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-top: 1px solid var(--border);
    background: var(--chrome-sunken-bg);
  }

  .toggle-label {
    font-family: var(--font-head);
    font-size: 9px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  .tuning-card.enabled .toggle-label { color: var(--accent-dim); }

  .card-edit-btn {
    margin-left: auto;
    padding: 3px 10px;
  }

  /* ── Toggle (shared with sidebar pattern, sized for cards) ── */
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
  .file-toggle.on { background: var(--accent-glow-strong); border-color: var(--accent-dim); }
  .file-toggle.on::after { left: 14px; background: var(--accent-bright); }
  .file-toggle.locked { opacity: 0.3; cursor: not-allowed; pointer-events: none; }

  /* ── Card tag picker ── */
  .card-tag-picker {
    display: flex;
    gap: 3px;
    flex-wrap: wrap;
  }

  .card-tag-add {
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    padding: 1px 5px;
    border-radius: 2px;
    border: 1px dashed var(--border-mid);
    background: none;
    color: var(--text-3);
    cursor: pointer;
    transition: all 0.12s;
    align-self: flex-start;
  }
  .card-tag-add:hover { border-color: var(--accent-dim); color: var(--accent-bright); }

  .tag-opt {
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    padding: 2px 6px;
    border-radius: 2px;
    border: 1px solid var(--border-mid);
    background: var(--bg-3);
    color: var(--text-2);
    cursor: pointer;
    transition: all 0.1s;
  }
  .tag-opt.active { color: var(--text-0); }
  .tag-opt:hover { border-color: var(--border-lit); color: var(--text-0); }
</style>