<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { appConfig, serverRunning, setBackupTargets, setUpdateBackupOptions } from '../lib/store'
  import PanelSidebar from './PanelSidebar.svelte'

  // ── Types ──────────────────────────────────────────────────────────────────

  interface UpdateInfo {
    build_date: string
    download_url: string
    available: boolean
  }

  interface UpdateProgress {
    stage: 'backing_up' | 'downloading' | 'extracting' | 'installing' | 'restoring' | 'done'
    pct: number
    message?: string
  }

  interface BackupManifest {
    id: string
    created_at: string
    label: string
    targets: string[]
    size_bytes: number
  }

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

  const FULL_DATA_TARGET = { id: 'Data', label: 'Full Data Folder', description: 'Entire Data directory — includes everything below' }

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
  let updating = false
  let updateProgress: UpdateProgress | null = null
  let updateError = ''
  let updateSuccess = false

  // Backups
  let backups: BackupManifest[] = []
  let loadingBackups = false
  let backupError = ''
  let backupSuccess = ''
  let creatingBackup = false
  let confirmingRestore: string | null = null
  let confirmingDelete: string | null = null

  let unlistenProgress: UnlistenFn | null = null

  // ── Derived ────────────────────────────────────────────────────────────────

  $: hasServerExe = !!$appConfig.server_exe
  $: updateBackupOptions = $appConfig.update_backup_options
  $: selectedTargets = new Set($appConfig.backup_targets)
  $: fullDataSelected = selectedTargets.has('Data')

  // ── Update ─────────────────────────────────────────────────────────────────

  async function checkForUpdate() {
    checking = true
    checkError = ''
    updateInfo = null
    try {
      updateInfo = await invoke<UpdateInfo>('check_update_available')
    } catch (e) {
      checkError = String(e)
    } finally {
      checking = false
    }
  }

  async function runUpdate() {
    if (!hasServerExe || !updateInfo?.available || updating || $serverRunning) return
    updating = true
    updateError = ''
    updateSuccess = false
    updateProgress = null
    try {
      await invoke('run_update', {
        serverExe: $appConfig.server_exe,
        backupOptions: {
          config_ini:    updateBackupOptions.config_ini,
          live_tuning:   updateBackupOptions.live_tuning,
          billing_store: updateBackupOptions.billing_store,
        },
      })
      updateSuccess = true
      updateProgress = null
      await loadBackups()
    } catch (e) {
      updateError = String(e)
      updateProgress = null
    } finally {
      updating = false
    }
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
      await invoke('restore_backup', { serverExe: $appConfig.server_exe, backupId: id })
      backupSuccess = 'Backup restored.'
      setTimeout(() => backupSuccess = '', 3000)
    } catch (e) {
      backupError = String(e)
    }
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
    await setBackupTargets([...updated])
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
      const { openPath } = await import('@tauri-apps/plugin-opener')
      await openPath(dir)
    } catch (e) {
      openDirError = String(e)
    }
  }

  // ── Lifecycle ──────────────────────────────────────────────────────────────

  onMount(async () => {
    unlistenProgress = await listen<UpdateProgress>('update-progress', e => {
      updateProgress = e.payload
    })
    if (hasServerExe) {
      await checkForUpdate()
      await loadBackups()
    }
  })

  onDestroy(() => {
    if (unlistenProgress) unlistenProgress()
  })
</script>

<div class="ops-panel">
  <div class="panel-bg"></div>
  <div class="grid-overlay"></div>
  <div class="ops-layout">
  <PanelSidebar width="var(--sidebar-narrow)">
    <svelte:fragment slot="header">
      <div class="section-title">Ops</div>
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
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
          <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 1 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/>
          <circle cx="12" cy="12" r="3"/>
        </svg>
        <span class="empty-state-label">No server configured</span>
        <span class="empty-state-sub">Set the server exe path in Settings to use Ops features.</span>
      </div>

    {:else if section === 'update'}
      <!-- ── Update section ── -->
      <div class="ops-section-head">
        <div class="section-title">Nightly Update</div>
        <button class="btn btn-sm btn-outline" on:click={checkForUpdate} disabled={checking || updating}>
          {#if checking}
            Checking...
          {:else}
            Check
          {/if}
        </button>
      </div>
      <div class="ops-body">

        <!-- Status block -->
        <div class="info-block">
          <div class="info-row">
            <span class="info-label">Build Date</span>
            {#if updateInfo}
              <span class="info-value mono">{updateInfo.build_date.replace(/(\d{4})(\d{2})(\d{2})/, '$1-$2-$3')}</span>
            {:else if checking}
              <span class="info-value dim">Checking...</span>
            {:else}
              <span class="info-value dim">—</span>
            {/if}
          </div>
          <div class="info-row">
            <span class="info-label">Status</span>
            {#if checking}
              <span class="status-pill checking">Checking</span>
            {:else if checkError}
              <span class="status-pill unavailable">Error</span>
            {:else if updateInfo?.available}
              <span class="status-pill available">Available</span>
            {:else if updateInfo}
              <span class="status-pill unavailable">Unavailable</span>
            {:else}
              <span class="info-value dim">Not checked</span>
            {/if}
          </div>
          {#if updateInfo}
            <div class="info-row url-row">
              <span class="info-label">URL</span>
              <span class="info-value mono url-value" title={updateInfo.download_url}>{updateInfo.download_url}</span>
            </div>
          {/if}
          {#if checkError}
            <div class="inline-error">{checkError}</div>
          {/if}
        </div>

        <!-- Auto-backup options -->
        <div class="subsection-title">Auto-backup before update</div>
        <div class="checkbox-list">
          <label class="checkbox-row">
            <input type="checkbox"
              checked={updateBackupOptions.config_ini}
              on:change={e => setUpdateBackupOptions({ ...updateBackupOptions, config_ini: (e.target as HTMLInputElement).checked })}
              disabled={updating}>
            <span class="checkbox-label">Config.ini</span>
          </label>
          <label class="checkbox-row">
            <input type="checkbox"
              checked={updateBackupOptions.live_tuning}
              on:change={e => setUpdateBackupOptions({ ...updateBackupOptions, live_tuning: (e.target as HTMLInputElement).checked })}
              disabled={updating}>
            <span class="checkbox-label">Live Tuning</span>
          </label>
          <label class="checkbox-row">
            <input type="checkbox"
              checked={updateBackupOptions.billing_store}
              on:change={e => setUpdateBackupOptions({ ...updateBackupOptions, billing_store: (e.target as HTMLInputElement).checked })}
              disabled={updating}>
            <span class="checkbox-label">Account Store</span>
          </label>
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
        <button
          class="btn btn-accent btn-sm update-btn"
          on:click={runUpdate}
          disabled={updating || $serverRunning || !updateInfo?.available}
        >
          {#if updating}
            Updating...
          {:else}
            Update Server
          {/if}
        </button>

        <!-- Progress -->
        {#if updating && updateProgress && updateProgress.stage !== 'done'}
          <div class="progress-wrap">
            <div class="progress-bar">
              <div class="progress-fill" style="width: {updateProgress.pct}%"></div>
            </div>
            <div class="progress-meta">
              <span class="progress-stage">{STAGE_LABELS[updateProgress.stage] ?? updateProgress.stage}</span>
              <span class="progress-pct">{Math.round(updateProgress.pct)}%</span>
            </div>
          </div>
        {/if}

        {#if updateSuccess}
          <div class="feedback-ok">Update complete.</div>
        {/if}
        {#if updateError}
          <div class="feedback-error">{updateError}</div>
        {/if}

      </div>

    {:else if section === 'backups'}
      <!-- ── Backups section ── -->
      <div class="ops-section-head">
        <div class="section-title">Backups</div>
        <button
          class="btn btn-sm btn-outline"
          on:click={createBackup}
          disabled={creatingBackup || selectedTargets.size === 0}
        >
          {creatingBackup ? 'Creating...' : 'Create Backup'}
        </button>
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
          </div>
        </div>

        <!-- Right: history -->
        <div class="backups-right">
          <div class="backups-right-head">
            <span class="subsection-title" style="margin:0; flex:1;">History</span>
            {#if backupSuccess}
              <span class="feedback-ok" style="font-size:11px;">{backupSuccess}</span>
            {/if}
            {#if backupError}
              <span class="feedback-error" style="font-size:11px;">{backupError}</span>
            {/if}
            {#if openDirError}
              <span class="inline-error" style="font-size:10px;">{openDirError}</span>
            {/if}
            <button
              class="btn-icon"
              on:click={openBackupsDir}
              title="Open Backups folder"
              disabled={!hasServerExe}
            >
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
              </svg>
            </button>
          </div>

          {#if loadingBackups}
            <div class="backup-notice">Loading...</div>
          {:else if backups.length === 0}
            <div class="backup-notice">No backups yet.</div>
          {:else}
            <div class="backup-list">
              {#each backups as backup (backup.id)}
                <div class="backup-row" class:confirming={confirmingRestore === backup.id || confirmingDelete === backup.id}>
                  <div class="backup-meta">
                    <span class="backup-date">{formatDate(backup.created_at)}</span>
                    <span class="backup-label-badge" class:pre-update={backup.label === 'pre-update'}>
                      {backup.label === 'pre-update' ? 'Pre-update' : 'Manual'}
                    </span>
                    <span class="backup-size">{formatBytes(backup.size_bytes)}</span>
                  </div>
                  <div class="backup-targets-line">
                    {backup.targets.map(targetLabel).join(', ') || 'No files'}
                  </div>

                  {#if confirmingRestore === backup.id}
                    <div class="confirm-row">
                      <span class="confirm-text">This will overwrite current files.</span>
                      <button class="btn btn-sm btn-outline" on:click={cancelConfirm}>Cancel</button>
                      <button class="btn btn-sm btn-accent" on:click={() => restoreBackup(backup.id)}>Confirm Restore</button>
                    </div>
                  {:else if confirmingDelete === backup.id}
                    <div class="confirm-row">
                      <span class="confirm-text">Delete this backup permanently?</span>
                      <button class="btn btn-sm btn-outline" on:click={cancelConfirm}>Cancel</button>
                      <button class="btn btn-sm btn-red" on:click={() => deleteBackup(backup.id)}>Delete</button>
                    </div>
                  {:else}
                    <div class="backup-row-actions">
                      <button class="btn btn-sm btn-outline" on:click={() => restoreBackup(backup.id)}>Restore</button>
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

  .ops-section-head {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 20px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    min-height: 52px;
  }
  .ops-section-head .section-title { font-size: 11px; }
  .ops-section-head .btn { margin-left: auto; }

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
    font-size: 9px;
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
  .info-value.dim  { color: var(--text-3); }

  .url-value {
    font-size: 10px;
    color: var(--text-3);
    word-break: break-all;
    line-height: 1.4;
  }

  .status-pill {
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    padding: 2px 8px;
    border-radius: 2px;
    border: 1px solid transparent;
  }
  .status-pill.available   { color: var(--green-bright); border-color: rgba(39,174,96,0.4); background: var(--green-dim); }
  .status-pill.unavailable { color: var(--text-3);       border-color: var(--border-mid);   background: var(--bg-3); }
  .status-pill.checking    { color: var(--text-2);       border-color: var(--border-mid);   background: var(--bg-3); }

  .inline-error {
    font-size: 11px;
    color: var(--text-error);
  }

  /* ── Checkbox list (update backup options) ── */
  .checkbox-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .checkbox-row {
    display: flex;
    align-items: center;
    gap: 10px;
    cursor: pointer;
  }

  .checkbox-row input[type="checkbox"] {
    width: 14px;
    height: 14px;
    accent-color: var(--accent);
    flex-shrink: 0;
    cursor: pointer;
  }

  .checkbox-label {
    font-size: 12px;
    color: var(--text-1);
  }

  /* ── Warning notice ── */
  .warning-notice {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    background: var(--amber-dim);
    border: 1px solid rgba(200,146,10,0.3);
    border-radius: var(--radius-sm);
    font-size: 12px;
    color: var(--amber-bright);
  }

  /* ── Update button ── */
  .update-btn {
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
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-2);
  }

  .progress-pct {
    font-family: var(--font-mono);
    font-size: 11px;
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
  .target-children.muted { opacity: 0.4; }

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
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-1);
  }

  .target-desc {
    font-size: 11px;
    color: var(--text-3);
  }

  .backup-actions {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 0;
    border-top: 1px solid var(--border);
    margin-top: 4px;
  }

  /* ── Backup history ── */
  .backup-notice {
    font-family: var(--font-head);
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
    padding: 16px 0 8px;
  }

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
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-1);
  }

  .backup-label-badge {
    font-family: var(--font-head);
    font-size: 9px;
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
    border-color: rgba(200,146,10,0.3);
    background: var(--amber-dim);
  }

  .backup-size {
    font-family: var(--font-mono);
    font-size: 11px;
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
</style>