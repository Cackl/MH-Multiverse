<script lang="ts">
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { openPath, openUrl } from '@tauri-apps/plugin-opener'
  import {
    appConfig, serverRunning, setBackupTargets, updateRunning, updateProgress, updateOutcome, runServerUpdate,
  } from '../lib/store'
  import PanelSidebar from './PanelSidebar.svelte'
  import { tip } from '../lib/tooltip'

  // ── Types ──────────────────────────────────────────────────────────────────

  interface UpdateInfo {
    build_date: string
    download_url: string
    available: boolean
    installed_build_date?: string | null
    up_to_date?: boolean
    unavailable_reason?: string | null
  }

  interface BackupManifest {
    id: string
    created_at: string
    label: string
    targets: string[]
    size_bytes: number
    replaced?: string[]
    update?: { linked_name?: string | null; from?: string | null; to?: string | null }
  }

  // "1.0.3+build20261027131155" → "1.0.3 build 2026-10-27"
  const serverVer = (v?: string | null) =>
    v ? v.replace(/\+build(\d{4})(\d{2})(\d{2})\d*/, ' build $1-$2-$3') : 'unknown'

  // ── Constants ──────────────────────────────────────────────────────────────

  const STAGE_LABELS: Record<string, string> = {
    backing_up:  'Creating backup...',
    downloading: 'Downloading...',
    extracting:  'Extracting archive...',
    installing:  'Installing files...',
    restoring:   'Restoring user files...',
    done:        'Complete',
  }

  // Backup targets: id is the relative path passed to the backend.
  // TOP_LEVEL targets are shown ungrouped at the top.
  // DATA_TARGETS are children of "Full Data" — disabled when fullData is selected.
  const TOP_TARGETS = [
    { id: 'Config.ini',           label: 'Config.ini',         description: 'Server configuration' },
    { id: 'ConfigOverride.ini',   label: 'ConfigOverride.ini', description: 'Local configuration overrides' },
  ]

  const FULL_DATA_TARGET = { id: 'Data', label: 'Full Data Folder', description: 'Entire Data directory, includes everything below' }

  const DATA_TARGETS = [
    { id: 'Data/Game/LiveTuning', label: 'Live Tuning',    description: 'LiveTuning JSON files' },
    { id: 'Data/Account.db',      label: 'Account Store',  description: 'Player account database (Account.db)' },
    { id: 'Data/Game/MTXStore',   label: 'MTX Store',      description: 'In-game store catalogue files' },
    { id: 'Data/Game/Achievements', label: 'Achievements', description: 'Achievement definition files' },
    { id: 'Data/Game/Patches',    label: 'Patches',        description: 'Bug fix and content patch files' },
    { id: 'Data/Web',             label: 'Web',            description: 'Dashboard and store static web pages' },
  ]

  // ── State ──────────────────────────────────────────────────────────────────

  type Section = 'update' | 'backups'
  let section: Section = 'update'

  // Update
  let updateInfo: UpdateInfo | null = null
  let checking = false
  let checkError = ''

  // Backups
  let backups: BackupManifest[] = []
  let loadingBackups = false
  let backupError = ''
  let backupSuccess = ''
  let creatingBackup = false
  let confirmingRestore: string | null = null
  let confirmingDelete: string | null = null

  // ── Derived ────────────────────────────────────────────────────────────────

  $: hasServerExe = !!$appConfig.server_exe
  $: selectedTargets = new Set($appConfig.backup_targets)
  $: fullDataSelected = selectedTargets.has('Data')
  $: updating = $updateRunning
  $: outcome = $updateOutcome?.serverExe === $appConfig.server_exe ? $updateOutcome : null

  // ── Update ─────────────────────────────────────────────────────────────────

  async function checkForUpdate() {
    checking = true
    checkError = ''
    updateInfo = null
    try {
      updateInfo = await invoke<UpdateInfo>('check_update_available', { serverExe: $appConfig.server_exe })
    } catch (e) {
      checkError = String(e)
    } finally {
      checking = false
    }
  }

  async function runUpdate() {
    if (!hasServerExe || !updateInfo?.available || updateInfo.up_to_date || updating || $serverRunning) return
    await runServerUpdate($appConfig.server_exe, [...selectedTargets])
  }

  // When an update ends, even one started before this panel opened. A failed install keeps its
  // backup, so the list reloads either way.
  let wasUpdating = $updateRunning
  $: {
    if (wasUpdating && !updating) {
      loadBackups()
      if (outcome && !outcome.error) checkForUpdate()
    }
    wasUpdating = updating
  }

  // ── Backups ────────────────────────────────────────────────────────────────

  async function loadBackups() {
    if (!hasServerExe) return
    loadingBackups = true
    try {
      backups = await invoke<BackupManifest[]>('list_backups', { serverExe: $appConfig.server_exe })
    } catch (e) {
      backupError = String(e)
    } finally {
      loadingBackups = false
    }
  }

  async function createBackup() {
    if (!hasServerExe || creatingBackup) return
    creatingBackup = true
    backupError = ''
    backupSuccess = ''
    try {
      await invoke('create_backup', {
        serverExe: $appConfig.server_exe,
        targets: [...selectedTargets],
        label: 'manual',
      })
      backupSuccess = 'Backup created.'
      setTimeout(() => backupSuccess = '', 3000)
      await loadBackups()
    } catch (e) {
      backupError = String(e)
    } finally {
      creatingBackup = false
    }
  }

  async function restoreBackup(id: string) {
    if (confirmingRestore !== id) {
      confirmingRestore = id
      confirmingDelete = null
      return
    }
    confirmingRestore = null
    backupError = ''
    try {
      const moved = await invoke<string[]>('restore_backup', { serverExe: $appConfig.server_exe, backupId: id })
      backupSuccess = moved.length
        ? `Backup restored. Copies switched the other way saved as ${moved.join(', ')}`
        : 'Backup restored.'
      setTimeout(() => backupSuccess = '', moved.length ? 8000 : 3000)
    } catch (e) {
      backupError = String(e)
    }
    // The restore backs up the files it replaces first, failed or not.
    await loadBackups()
  }

  async function deleteBackup(id: string) {
    if (confirmingDelete !== id) {
      confirmingDelete = id
      confirmingRestore = null
      return
    }
    confirmingDelete = null
    backupError = ''
    try {
      await invoke('delete_backup', { serverExe: $appConfig.server_exe, backupId: id })
      await loadBackups()
    } catch (e) {
      backupError = String(e)
    }
  }

  // ── Helpers ────────────────────────────────────────────────────────────────

  let targetsError = ''
  async function toggleTarget(id: string) {
    const updated = new Set(selectedTargets)
    if (updated.has(id)) {
      updated.delete(id)
    } else {
      updated.add(id)
      if (id === 'Data') {
        for (const t of DATA_TARGETS) updated.delete(t.id)
      }
    }
    targetsError = await setBackupTargets([...updated])
  }

  function formatBytes(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`
    return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`
  }

  function formatDate(iso: string): string {
    return new Date(iso).toLocaleString(undefined, {
      day: 'numeric', month: 'short', year: 'numeric',
      hour: '2-digit', minute: '2-digit',
    })
  }

  function targetLabel(id: string): string {
    return (
      TOP_TARGETS.find(t => t.id === id)?.label ??
      DATA_TARGETS.find(t => t.id === id)?.label ??
      (id === 'Data' ? FULL_DATA_TARGET.label : id)
    )
  }

  function cancelConfirm() {
    confirmingRestore = null
    confirmingDelete = null
  }

  let openDirError = ''

  async function openBackupsDir() {
    if (!hasServerExe) return
    openDirError = ''
    try {
      const dir = await invoke<string>('get_backups_dir', { serverExe: $appConfig.server_exe })
      await openPath(dir)
    } catch (e) {
      openDirError = String(e)
    }
  }

  // ── Lifecycle ──────────────────────────────────────────────────────────────

  onMount(async () => {
    // Backups are local and quick, the update check waits on GitHub, so neither waits for the other.
    if (hasServerExe) {
      loadBackups()
      checkForUpdate()
    }
  })
</script>

<div class="ops-panel">
  <div class="panel-bg"></div>
  <div class="grid-overlay"></div>
  <div class="ops-layout">
  <PanelSidebar width="var(--sidebar-narrow)">
    <svelte:fragment slot="header">
      <div class="section-title">Update</div>
    </svelte:fragment>

    <nav class="ops-nav-list">
      <div
        class="nav-item"
        class:selected={section === 'update'}
        on:click={() => section = 'update'}
        role="button"
        tabindex="0"
        on:keydown={(e) => e.key === 'Enter' && (section = 'update')}
      >
        Update
      </div>
      <div
        class="nav-item"
        class:selected={section === 'backups'}
        on:click={() => section = 'backups'}
        role="button"
        tabindex="0"
        on:keydown={(e) => e.key === 'Enter' && (section = 'backups')}
      >
        Backups
      </div>
    </nav>
  </PanelSidebar>

  <div class="ops-main">

    {#if !hasServerExe}
      <div class="empty-state">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:20px;height:20px;color:var(--text-2)">
          <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/>
          <circle cx="12" cy="12" r="3"/>
        </svg>
        <span class="empty-state-label">No server configured</span>
        <span class="empty-state-sub">Set the server exe path in Settings to use Ops features.</span>
      </div>

    {:else if section === 'update'}
      <!-- ── Update section ── -->
      <div class="page-head">
        <h3 class="page-title">Nightly Update</h3>
        <div class="page-acts">
          <button class="btn btn-sm btn-outline" on:click={checkForUpdate} disabled={checking || updating}>
            {#if checking}
              Checking...
            {:else}
              Check
            {/if}
          </button>
        </div>
      </div>
      <div class="ops-body">

        <!-- Status block -->
        <div class="info-block">
          <div class="info-row">
            <span class="info-label">Build Date</span>
            {#if updateInfo?.build_date}
              <span class="info-value num">{updateInfo.build_date.replace(/(\d{4})(\d{2})(\d{2})/, '$1-$2-$3')}</span>
            {:else if checking}
              <span class="info-value dim">Checking...</span>
            {:else}
              <span class="info-value dim">—</span>
            {/if}
          </div>
          {#if updateInfo?.installed_build_date}
            <div class="info-row">
              <span class="info-label">Installed</span>
              <span class="info-value num">{updateInfo.installed_build_date.replace(/(\d{4})(\d{2})(\d{2})/, '$1-$2-$3')}</span>
            </div>
          {/if}
          <div class="info-row">
            <span class="info-label">Status</span>
            {#if checking}
              <span class="status-pill checking">Checking</span>
            {:else if checkError}
              <span class="status-pill unavailable">Error</span>
            {:else if updateInfo?.up_to_date}
              <span class="status-pill unavailable">Up to date</span>
            {:else if updateInfo?.available}
              <span class="status-pill available">Available</span>
            {:else if updateInfo}
              <span class="status-pill unavailable">Unavailable</span>
            {:else}
              <span class="info-value dim">Not checked</span>
            {/if}
          </div>
          {#if updateInfo?.unavailable_reason}
            <div class="info-value dim">{updateInfo.unavailable_reason}</div>
          {:else if updateInfo}
            <div class="info-row url-row">
              <span class="info-label">URL</span>
              <span class="info-value mono url-value">{updateInfo.download_url}</span>
            </div>
          {/if}
          {#if checkError}
            <div class="feedback-error">{checkError}</div>
          {/if}
        </div>

        <!-- Server running warning -->
        {#if $serverRunning}
          <div class="warning-notice">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:14px;height:14px;flex-shrink:0;">
              <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/>
              <line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/>
            </svg>
            Stop the server before running an update.
          </div>
        {/if}

        <!-- Update button -->
        <div class="update-actions">
          <button
            class="btn btn-accent btn-sm"
            on:click={runUpdate}
            disabled={updating || $serverRunning || !updateInfo?.available || updateInfo.up_to_date}
          >
            {#if updating}
              Updating...
            {:else}
              Update Server
            {/if}
          </button>
          {#if updating && ['downloading', 'extracting', 'backing_up'].includes($updateProgress?.stage ?? '')}
            <button class="btn btn-sm btn-outline" on:click={() => invoke('cancel_update')}>Cancel Update</button>
          {/if}
          <button class="btn btn-sm btn-outline" on:click={() => openUrl('https://github.com/Crypto137/MHServerEmu')}>MHServerEmu GitHub</button>
        </div>

        <!-- Progress -->
        {#if updating && $updateProgress && $updateProgress.stage !== 'done'}
          <div class="progress-wrap">
            <div class="progress-bar">
              <div class="progress-fill" style="width: {$updateProgress.pct}%"></div>
            </div>
            <div class="progress-meta">
              <span class="progress-stage">{STAGE_LABELS[$updateProgress.stage] ?? $updateProgress.stage}</span>
              <span class="progress-pct">{Math.round($updateProgress.pct)}%</span>
            </div>
          </div>
        {/if}

        {#if outcome && !outcome.error}
          <div class="feedback-ok">Update complete.</div>
          {#if outcome.edited.length}
            <div class="kept-files">Replaced your edits to {outcome.edited.join(', ')}. Your copies are in the Pre-update backup.</div>
          {/if}
          {#each [true, false] as on}
            {@const group = outcome.kept.filter(k => k.enabled === on)}
            {#if group.length}
              <div class="kept-files" use:tip={'The update shipped these switched the other way'}>
                {on ? 'Kept on' : 'Kept off'}: {group.map(k => k.file).join(', ')}
              </div>
              <div class="kept-files">Replaced copies saved as {group.map(k => k.kept_as).join(', ')}</div>
            {/if}
          {/each}
        {/if}
        {#if outcome?.error}
          <div class="feedback-error">{outcome.error}</div>
        {/if}

        <div class="restart-notice">
          <span>Before installing, a backup is made of the Targets selected on the Backups page and of every file the update replaces. The Targets are put back afterwards</span>
          <span>Multiverse doesn't check the server's system requirements. If the server stops working after an update, see MHServerEmu's GitHub or Discord</span>
        </div>

      </div>

    {:else if section === 'backups'}
      <!-- ── Backups section ── -->
      <div class="page-head">
        <h3 class="page-title">Backups</h3>
        <div class="page-acts">
          <button
            class="btn btn-sm btn-outline"
            on:click={createBackup}
            disabled={creatingBackup || selectedTargets.size === 0 || $serverRunning}
          >
            {creatingBackup ? 'Creating...' : 'Create Backup'}
          </button>
        </div>
      </div>
      <div class="backups-columns">

        <!-- Left: targets + create -->
        <div class="backups-left">
          <div class="subsection-title">Targets</div>
          <div class="target-list">
            {#each TOP_TARGETS as target}
              <label class="target-row">
                <input
                  type="checkbox"
                  checked={selectedTargets.has(target.id)}
                  on:change={() => toggleTarget(target.id)}
                  disabled={creatingBackup}
                >
                <div class="target-info">
                  <span class="target-label">{target.label}</span>
                  <span class="target-desc">{target.description}</span>
                </div>
              </label>
            {/each}

            <div class="target-divider"></div>

            <!-- Full Data — parent toggle -->
            <label class="target-row">
              <input
                type="checkbox"
                checked={fullDataSelected}
                on:change={() => toggleTarget('Data')}
                disabled={creatingBackup}
              >
              <div class="target-info">
                <span class="target-label">{FULL_DATA_TARGET.label}</span>
                <span class="target-desc">{FULL_DATA_TARGET.description}</span>
              </div>
            </label>

            <!-- Data sub-targets — indented, disabled when full data is selected -->
            <div class="target-children" class:muted={fullDataSelected}>
              {#each DATA_TARGETS as target}
                <label class="target-row child" class:disabled={fullDataSelected}>
                  <input
                    type="checkbox"
                    checked={fullDataSelected || selectedTargets.has(target.id)}
                    on:change={() => toggleTarget(target.id)}
                    disabled={creatingBackup || fullDataSelected}
                  >
                  <div class="target-info">
                    <span class="target-label">{target.label}</span>
                    <span class="target-desc">{target.description}</span>
                  </div>
                </label>
              {/each}
            </div>
            {#if targetsError}<div class="feedback-error">{targetsError}</div>{/if}
          </div>
        </div>

        <!-- Right: history -->
        <div class="backups-right">
          <div class="backups-right-head">
            <span class="subsection-title" style="margin:0; flex:1;">History</span>
            {#if backupSuccess}
              <span class="feedback-ok">{backupSuccess}</span>
            {/if}
            {#if backupError}
              <span class="feedback-error">{backupError}</span>
            {/if}
            {#if openDirError}
              <span class="feedback-error">{openDirError}</span>
            {/if}
            <button
              class="btn-icon"
              on:click={openBackupsDir}
              aria-label="Open Backups folder"
              use:tip={'Open Backups folder'}
              disabled={!hasServerExe}
            >
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
              </svg>
            </button>
          </div>

          {#if $serverRunning}
            <div class="warning-notice">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:14px;height:14px;flex-shrink:0;">
                <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/>
                <line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/>
              </svg>
              Stop the server to create or restore a backup.
            </div>
          {/if}

          {#if loadingBackups}
            <div class="empty-state"><span class="empty-state-label">Loading backups</span></div>
          {:else if backups.length === 0}
            <div class="empty-state"><span class="empty-state-label">No backups yet</span></div>
          {:else}
            <div class="backup-list">
              {#each backups as backup (backup.id)}
                <div class="backup-row" class:confirming={confirmingRestore === backup.id || confirmingDelete === backup.id}>
                  <div class="backup-meta">
                    <span class="backup-date">{formatDate(backup.created_at)}</span>
                    <span class="backup-label-badge" class:pre-update={backup.label === 'pre-update'}>
                      {backup.label === 'pre-update' ? 'Pre-update' : backup.label === 'pre-restore' ? 'Pre-restore' : 'Manual'}
                    </span>
                    <span class="backup-size">{formatBytes(backup.size_bytes)}</span>
                  </div>
                  {#if backup.update?.from || backup.update?.to}
                    <div class="backup-targets-line">Server {serverVer(backup.update.from)} → {serverVer(backup.update.to)}</div>
                  {/if}
                  <div class="backup-targets-line">
                    {[...backup.targets.map(targetLabel), ...(backup.replaced?.length ? [`${backup.replaced.length} files the update replaced`] : [])].join(', ') || 'No files'}
                  </div>

                  {#if confirmingRestore === backup.id}
                    <div class="confirm-row">
                      <span class="confirm-text">Current files are backed up first, then overwritten.</span>
                      <button class="btn btn-sm btn-accent" on:click={() => restoreBackup(backup.id)} disabled={$serverRunning}>Confirm Restore</button>
                      <button class="btn btn-sm btn-outline" on:click={cancelConfirm}>Cancel</button>
                    </div>
                  {:else if confirmingDelete === backup.id}
                    <div class="confirm-row">
                      <span class="confirm-text">Delete this backup permanently?</span>
                      <button class="btn btn-sm btn-red" on:click={() => deleteBackup(backup.id)}>Delete</button>
                      <button class="btn btn-sm btn-outline" on:click={cancelConfirm}>Cancel</button>
                    </div>
                  {:else}
                    <div class="backup-row-actions">
                      <button class="btn btn-sm btn-outline" on:click={() => restoreBackup(backup.id)} disabled={$serverRunning}>Restore</button>
                      <button class="btn btn-sm btn-red" on:click={() => deleteBackup(backup.id)}>Delete</button>
                    </div>
                  {/if}
                </div>
              {/each}
            </div>
          {/if}
        </div>

      </div>

      {/if}

  </div><!-- ops-layout -->
</div>
</div>

<style>
  .ops-panel {
    flex: 1;
    display: flex;
    flex-direction: column;
    position: relative;
    overflow: hidden;
    min-height: 0;
  }

  .ops-layout {
    position: relative;
    z-index: 1;
    flex: 1;
    display: flex;
    overflow: hidden;
    min-height: 0;
  }

  /* ── Sidebar nav ── */
  .ops-nav-list {
    flex: 1;
    overflow-y: auto;
    padding: 6px;
  }

  .nav-item {
    padding: 10px 12px;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: all 0.12s;
    margin-bottom: 2px;
    font-family: var(--font-head);
    font-size: 13px;
    font-weight: 600;
    color: var(--text-1);
  }
  .nav-item:hover {
    background: var(--bg-3);
    border-color: var(--border-mid);
    color: var(--text-0);
  }
  .nav-item.selected {
    background: var(--accent-glow);
    border-color: var(--accent-dim);
    color: var(--accent-bright);
  }

  /* ── Main area ── */
  .ops-main {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    min-width: 0;
    background: var(--bg-1);
  }

  .ops-body {
    flex: 1;
    overflow-y: auto;
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  /* ── Backups two-column layout ── */
  .backups-columns {
    flex: 1;
    display: flex;
    overflow: hidden;
    min-height: 0;
  }

  .backups-left {
    width: 260px;
    flex-shrink: 0;
    border-right: 1px solid var(--border);
    overflow-y: auto;
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .backups-right {
    flex: 1;
    overflow-y: auto;
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-width: 0;
  }

  .backups-right-head {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  /* ── Info block (update status) ── */
  .info-block {
    background: var(--bg-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .info-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .url-row { align-items: flex-start; }

  .info-label {
    font-family: var(--font-head);
    font-size: var(--fs-min);
    font-weight: 600;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-3);
    width: 72px;
    flex-shrink: 0;
  }

  .info-value {
    font-size: 12px;
    color: var(--text-1);
  }
  .info-value.mono { font-family: var(--font-mono); }
  .info-value.num  { font-variant-numeric: tabular-nums; }
  .info-value.dim  { color: var(--text-3); }

  .url-value {
    font-size: var(--fs-min);
    color: var(--text-3);
    word-break: break-all;
    line-height: 1.4;
  }

  .status-pill {
    font-family: var(--font-head);
    font-size: var(--fs-min);
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    padding: 2px 8px;
    border-radius: 2px;
    border: 1px solid transparent;
  }
  .status-pill.available   { color: var(--green-bright); border-color: color-mix(in srgb, var(--green) 40%, transparent); background: var(--green-dim); }
  .status-pill.unavailable { color: var(--text-3);       border-color: var(--border-mid);   background: var(--bg-3); }
  .status-pill.checking    { color: var(--text-2);       border-color: var(--border-mid);   background: var(--bg-3); }

  /* ── Warning notice ── */
  .warning-notice {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    background: var(--amber-dim);
    border: 1px solid color-mix(in srgb, var(--amber) 30%, transparent);
    border-radius: var(--radius-sm);
    font-size: 12px;
    color: var(--amber-bright);
  }

  /* ── Update button ── */
  .update-actions {
    display: flex;
    gap: 8px;
    align-self: flex-start;
  }

  /* ── Progress ── */
  .progress-wrap {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .progress-bar {
    height: 4px;
    background: var(--bg-3);
    border-radius: 2px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: var(--accent);
    border-radius: 2px;
    transition: width 0.2s ease;
  }

  .progress-meta {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .progress-stage {
    font-family: var(--font-head);
    font-size: var(--fs-min);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-2);
  }

  .progress-pct {
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    color: var(--accent-bright);
  }

  .target-divider {
    height: 1px;
    background: var(--border);
    margin: 4px 0;
  }

  .target-children {
    display: flex;
    flex-direction: column;
    gap: 0;
    padding-left: 24px;
    border-left: 2px solid var(--border);
    margin-left: 6px;
    transition: opacity 0.15s;
  }
  .target-children.muted { opacity: var(--dimmed-opacity); }

  .target-row.child { padding: 6px 10px; }
  .target-row.disabled { cursor: default; pointer-events: none; }

  /* ── Backup targets ── */
  .target-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .target-row {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: background 0.1s;
  }
  .target-row:hover { background: var(--bg-3); }

  .target-row input[type="checkbox"] {
    width: 14px;
    height: 14px;
    accent-color: var(--accent);
    flex-shrink: 0;
    margin-top: 1px;
    cursor: pointer;
  }

  .target-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .target-label {
    font-family: var(--font-body);
    font-size: 12px;
    color: var(--text-1);
  }

  .target-desc {
    font-size: 11px;
    color: var(--text-3);
  }

  /* ── Backup history ── */
  .backup-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .backup-row {
    background: var(--bg-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    transition: border-color 0.1s;
  }
  .backup-row.confirming { border-color: var(--border-lit); }

  .backup-meta {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .backup-date {
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    color: var(--text-1);
  }

  .backup-label-badge {
    font-family: var(--font-head);
    font-size: var(--fs-min);
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    padding: 1px 6px;
    border-radius: 2px;
    border: 1px solid var(--border-mid);
    color: var(--text-3);
    background: var(--bg-3);
  }
  .backup-label-badge.pre-update {
    color: var(--amber-bright);
    border-color: color-mix(in srgb, var(--amber) 30%, transparent);
    background: var(--amber-dim);
  }

  .backup-size {
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    color: var(--text-3);
    margin-left: auto;
  }

  .backup-targets-line {
    font-size: 11px;
    color: var(--text-3);
  }

  .backup-row-actions {
    display: flex;
    gap: 6px;
    justify-content: flex-end;
  }

  .confirm-row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .confirm-text {
    font-size: 11px;
    color: var(--text-2);
    flex: 1;
    min-width: 160px;
  }


  .kept-files {
    font-size: 11px;
    color: var(--text-2);
    overflow-wrap: anywhere;
  }

  .restart-notice {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
    padding: 8px 12px 10px;
    font-size: var(--fs-min);
    color: var(--text-3);
    border-top: 1px solid var(--border);
    line-height: 1.4;
  }

</style>