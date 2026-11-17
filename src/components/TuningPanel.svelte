<script lang="ts">
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { openPath } from '@tauri-apps/plugin-opener'
  import { appConfig, serverRunning, tuningFocusFile, askConfirm } from '../lib/store'
  import { KNOWN_EVENTS, TUNING_GROUPS, tuningDisplayName, tuningGroup, type TuningFileInfo, type EventDefinition, type EventsData } from '../lib/tuningMeta'
  import PanelSidebar from './PanelSidebar.svelte'
  import TuningFileEditor from './TuningFileEditor.svelte'
  import { tip } from '../lib/tooltip'
  import { reloadLiveTuning } from '../lib/serverReload'

  // ── State ──────────────────────────────────────────────────────────────────

  let files: TuningFileInfo[] = []
  let scanError = ''
  let scanning = false

  let editingFile: TuningFileInfo | null = null
  let creatingNew = false
  let editorDirty = false
  let eventDefsById: Record<string, EventDefinition> = {}

  // ── Derived ────────────────────────────────────────────────────────────────

  $: existingNames = files.map(f => f.canonical_name)

  // At runtime, treat files with event_id as the event set; fall back to static KNOWN_EVENTS
  // when no Events.json is present (older servers).
  $: runtimeEventFiles = new Set(files.filter(f => f.event_id !== null).map(f => f.canonical_name))
  $: effectiveEventSet = runtimeEventFiles.size > 0 ? runtimeEventFiles : KNOWN_EVENTS

  // Sidebar groups, each in scan order (root files in server load order).
  $: groups = TUNING_GROUPS.map(label => ({
    label,
    files: files.filter(f => tuningGroup(f, effectiveEventSet) === label),
  }))

  // ── Helpers ────────────────────────────────────────────────────────────────

  function displayName(file: TuningFileInfo): string {
    return tuningDisplayName(file.canonical_name)
  }

  // ── Scan ───────────────────────────────────────────────────────────────────

  async function scan() {
    if (!$appConfig.server_exe) return
    scanning = true
    scanError = ''
    try {
      files = await invoke<TuningFileInfo[]>('scan_tuning_files', {
        serverExe: $appConfig.server_exe,
        applyAutoEnable: true,
      })
      // Load event definitions for "managed by" labels — non-fatal if absent
      try {
        const evtsData = await invoke<EventsData>('load_events', { serverExe: $appConfig.server_exe })
        eventDefsById = Object.fromEntries(evtsData.definitions.map(d => [d.id, d]))
      } catch {
        eventDefsById = {}
      }
      // Keep modal in sync if the file it's editing was rescanned
      if (editingFile) {
        const still = files.find(f => f.relative_path === editingFile!.relative_path)
        editingFile = still ?? null
      }
    } catch (e) {
      scanError = String(e)
    } finally {
      scanning = false
    }
  }

  /** Shows a file in the editor, or a new one with `file` null. Unsaved edits are confirmed first. */
  async function openFile(file: TuningFileInfo | null) {
    const isNew = file === null
    if (isNew === creatingNew && file?.relative_path === editingFile?.relative_path) return
    if (editorDirty && !await askConfirm({
      title: 'Discard changes?', message: 'This file has unsaved changes. Discard them?',
      okLabel: 'Discard', cancelLabel: 'Keep editing',
    })) return
    editorDirty = false
    editingFile = file
    creatingNew = isNew
  }

  async function handleCreated(relativePath: string) {
    creatingNew = false
    editorDirty = false
    await scan()
    const created = files.find(f => f.relative_path === relativePath)
    if (created) editingFile = created
  }

  // ── Toggle file ────────────────────────────────────────────────────────────

  async function toggleFile(file: TuningFileInfo) {
    if (!file.toggleable || file.event_id) return
    const newEnabled = !file.enabled
    try {
      await invoke('toggle_tuning_file', {
        serverExe: $appConfig.server_exe,
        relativePath: file.relative_path,
        enabled: newEnabled,
      })
      files = files.map(f =>
        f.relative_path === file.relative_path ? { ...f, enabled: newEnabled } : f
      )
    } catch (err) {
      scanError = String(err)
    }
  }

  // ── Reload ─────────────────────────────────────────────────────────────────
  // Only a failed reload shows a message. A newer reload drops an older result.

  let reloadError = ''
  let reloadRun = 0
  async function reload() {
    const run = ++reloadRun
    reloadError = ''
    const error = await reloadLiveTuning()
    if (run === reloadRun) reloadError = error
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

  // When EventsPanel navigates here with a file to open, pre-select it once files are loaded.
  $: if ($tuningFocusFile && files.length > 0) {
    const target = files.find(f => f.relative_path === $tuningFocusFile)
    if (target) openFile(target)
    tuningFocusFile.set(null)
  }

  onMount(() => {
    if ($appConfig.server_exe) scan()
  })
</script>

<div class="tuning-panel">
  <div class="panel-bg"></div>
  <div class="grid-overlay"></div>
  <div class="tuning-layout">

  <!-- Left: sidebar -->
  <PanelSidebar width="var(--sidebar-wide)">
    <svelte:fragment slot="header">
      <div class="section-title">Tuning Files</div>
      <button
        class="btn-icon"
        on:click={() => openFile(null)}
        aria-label="Create new tuning file"
        use:tip={'Create new tuning file'}
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
        aria-label="Rescan LiveTuning directory"
        use:tip={'Rescan LiveTuning directory'}
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
        aria-label="Open LiveTuning folder"
        use:tip={'Open LiveTuning folder'}
        disabled={!$appConfig.server_exe}
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
        </svg>
      </button>
    </svelte:fragment>

    <!-- File list (unfiltered — full navigation list) -->
    <div class="file-list">
      {#if !$appConfig.server_exe}
        <div class="file-notice">Set server exe in Settings to load Tuning files.</div>
      {:else if scanning}
        <div class="file-notice">Scanning...</div>
      {:else if scanError}
        <div class="file-notice error">{scanError}</div>
      {:else if openDirError}
        <div class="file-notice error">{openDirError}</div>
      {:else if files.length === 0}
        <div class="file-notice">No LiveTuning files found.</div>
      {:else}
        {#each groups as group (group.label)}
          {#if group.files.length > 0}
            <div class="side-label">{group.label}</div>
            {#each group.files as file (file.relative_path)}
              <div
                class="side-item"
                class:off={!file.enabled}
                class:active={editingFile?.relative_path === file.relative_path}
                role="button"
                tabindex="0"
                aria-label="Edit {file.canonical_name}"
                on:click={() => openFile(file)}
                on:keydown={e => e.key === 'Enter' && openFile(file)}
              >
                <span class="file-text">
                  <span class="side-name">{group.label === 'Other' ? file.relative_path : displayName(file)}</span>
                  {#if file.event_id && eventDefsById[file.event_id]}
                    <span class="file-sub">{eventDefsById[file.event_id].display_name}</span>
                  {/if}
                </span>
                {#if file.toggleable}
                  <div
                    class="toggle-switch"
                    class:on={file.enabled}
                    role="switch"
                    aria-checked={file.enabled}
                    aria-label="{file.canonical_name} {file.enabled ? 'on' : 'off'}"
                    tabindex="0"
                    use:tip={file.enabled ? 'On. Switch off to stop the server loading this file.' : 'Off. Switch on to load this file again.'}
                    on:click|stopPropagation={() => toggleFile(file)}
                    on:keydown|stopPropagation={e => e.key === 'Enter' && toggleFile(file)}
                  ></div>
                {/if}
              </div>
            {/each}
          {/if}
        {/each}
      {/if}
    </div>
  </PanelSidebar>

  <!-- Right: the selected file's editor -->
  <section class="data-main">

    <header class="page-head">
      {#if creatingNew}
        <h3 class="page-title">New Tuning File</h3>
      {:else if editingFile}
        <h3 class="page-title" use:tip={editingFile.relative_path}>{tuningDisplayName(editingFile.canonical_name)}</h3>
      {:else}
        <h3 class="page-title">Live Tuning</h3>
      {/if}
      <div class="page-acts">
        {#if reloadError}<span class="reload-error">{reloadError}</span>{/if}
        <button
          class="btn btn-sm btn-outline"
          on:click={reload}
          disabled={!$serverRunning}
          use:tip={$serverRunning ? 'Send !server reloadlivetuning' : 'Server not running'}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:11px;height:11px;">
            <polyline points="23 4 23 10 17 10"/>
            <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/>
          </svg>
          Reload Live Tuning
        </button>
      </div>
    </header>

    {#if creatingNew}
      <TuningFileEditor
        file={null}
        serverExe={$appConfig.server_exe}
        {existingNames}
        bind:dirty={editorDirty}
        onCancel={() => { editorDirty = false; creatingNew = false }}
        onCreated={handleCreated}
      />
    {:else if editingFile}
      {#key editingFile.relative_path}
        <TuningFileEditor
          file={editingFile}
          serverExe={$appConfig.server_exe}
          {existingNames}
          bind:dirty={editorDirty}
        />
      {/key}
    {:else}
    <div class="pane-empty">
      {#if !$appConfig.server_exe}
        <div class="empty-state">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:20px;height:20px;color:var(--text-2)">
            <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/>
            <circle cx="12" cy="12" r="3"/>
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
          <span class="empty-state-label error">Scan error</span>
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
        <div class="empty-state">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
            <polyline points="14 2 14 8 20 8"/>
          </svg>
          <span class="empty-state-label">No file selected</span>
          <span class="empty-state-sub">Select a Live Tuning file from the sidebar.</span>
        </div>
      {/if}
    </div>
    {/if}
  </section>

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
  .file-notice.error {
    color: var(--text-error);
    text-transform: none;
    font-family: var(--font-body);
  }

  .file-list > .side-label:first-child { margin-top: 0; border-top: none; }
  /* Advanced sidebar size, matching the Patches and Store file rows. */
  .side-item { gap: 6px; padding: 7px 10px; min-height: 42px; font-size: 11px; }
  .side-item.off { opacity: var(--dimmed-opacity); }
  .file-text { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  .file-sub {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-family: var(--font-body);
    font-size: var(--fs-min);
    font-weight: 400;
    color: var(--text-3);
  }

  /* ── Grid pane ── */
  .reload-error { font-size: 12px; color: var(--text-error); }
  .pane-empty {
    flex: 1;
    overflow-y: auto;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

</style>