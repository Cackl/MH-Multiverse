<script lang="ts">
  import { tick } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { open } from '@tauri-apps/plugin-dialog'
  import { appConfig, serverRunning, upsertServer, type Server } from '../lib/store'

  export let server: Server | null = null
  export let onClose: () => void

  const isEdit = server !== null

  // ── Manual tab fields ────────────────────────────────────────────────────

  let name = server?.name ?? ''
  let host = server?.host ?? ''
  let email = server?.email ?? ''
  let password = ''
  let saving = false
  let error = ''
  let isLocal  = server?.is_local  ?? true
  let useHttps = server?.use_https ?? false

  async function save() {
    name = name.trim()
    host = host.trim()
    email = email.trim()
    password = password.trim()

    if (!name) { error = 'Name is required.'; return }
    if (!isLocal && !host) { error = 'Host is required for non-local servers.'; return }
    if (!isEdit && !password) { error = 'Password is required for a new server.'; return }

    saving = true
    error = ''

    try {
      const entry: Server = {
        id: server?.id ?? crypto.randomUUID(),
        name,
        host: isLocal ? '' : host,
        email,
        is_local: isLocal,
        use_https: useHttps,
      }
      await upsertServer(entry, password)
      onClose()
    } catch (e) {
      error = String(e)
    } finally {
      saving = false
    }
  }

  // ── Tabs ─────────────────────────────────────────────────────────────────
  // Import is only offered when adding a new server — it needs direct
  // Account.db access and always creates a local profile, neither of which
  // makes sense while editing an existing profile's connection details.

  type TabId = 'manual' | 'import' | 'restore'
  let activeTab: TabId = 'manual'

  // ── Import tab types ─────────────────────────────────────────────────────

  interface ImportSummary {
    player_name: string
    email: string
    avatar_count: number
    team_up_count: number
    item_count: number
    controlled_entity_count: number
    user_level: number
    flags: number
  }

  interface AccountEntry {
    id: number
    player_name: string
    email: string
  }

  interface BackupFileEntry {
    path: string
    file_name: string
    player_name: string
    account_id_hex: string
    modified: string | null
  }

  // ── Import tab state ─────────────────────────────────────────────────────

  let downloadBackups: BackupFileEntry[] = []
  let loadingDownloads = false
  let downloadsLoaded = false

  let existingAccounts: AccountEntry[] = []

  let importFilePath = ''
  let importSummary: ImportSummary | null = null
  let importParseError = ''

  let importServerName = ''
  let importPlayerName = ''
  let importEmail = ''
  let importPassword = ''

  let importSaving = false
  let importError = ''
  let importEmailError = ''
  let importNameError = ''

  // ── Restore tab state (Edit mode, local only) ────────────────────────────
  // Never touches Email/PlayerName/Password — see do_replace in accounts.rs.
  // Target account is resolved automatically from server.email; there is
  // no picker, since editing a profile already implies which account it is.

  let restoreFilePath = ''
  let restoreSummary: ImportSummary | null = null
  let restoreParseError = ''
  let restoreSaving = false
  let restoreError = ''

  // ── Derived ──────────────────────────────────────────────────────────────

  $: hasGameExe = !!$appConfig.game_exe
  $: hasServerExe = !!$appConfig.server_exe

  $: importConflictMsg = (() => {
    if (!importSummary) return ''
    const checkEmail = (importEmail.trim() || importSummary.email).toLowerCase()
    const checkName = (importPlayerName.trim() || importSummary.player_name).toLowerCase()
    if (existingAccounts.some(a => a.email.toLowerCase() === checkEmail)) {
      return `Email '${checkEmail}' is already registered.`
    }
    if (existingAccounts.some(a => a.player_name.toLowerCase() === checkName)) {
      return `Player name '${checkName}' is already in use.`
    }
    return ''
  })()

  $: canImport =
    !!importSummary &&
    !$serverRunning &&
    hasServerExe &&
    !!importPassword.trim() &&
    !!importServerName.trim() &&
    !importConflictMsg

  $: restoreTargetAccount = existingAccounts.find(
    a => a.email.toLowerCase() === (server?.email ?? '').toLowerCase()
  ) ?? null

  $: restoreMismatchMsg = (() => {
    if (!restoreSummary || !restoreTargetAccount) return ''
    const sameEmail = restoreSummary.email.toLowerCase() === restoreTargetAccount.email.toLowerCase()
    const sameName = restoreSummary.player_name.toLowerCase() === restoreTargetAccount.player_name.toLowerCase()
    if (sameEmail && sameName) return ''
    return `This backup is for ${restoreSummary.player_name} (${restoreSummary.email}), but this profile is for ${restoreTargetAccount.player_name} (${restoreTargetAccount.email}).`
  })()

  $: canRestore =
    !!restoreSummary &&
    !!restoreTargetAccount &&
    !restoreMismatchMsg &&
    !$serverRunning &&
    hasServerExe

  $: if (activeTab === 'restore' && !isLocal) activeTab = 'manual'

  // ── Tab switching ────────────────────────────────────────────────────────

  async function selectTab(tab: TabId) {
    activeTab = tab
    if ((tab === 'import' || tab === 'restore') && !downloadsLoaded) {
      downloadsLoaded = true
      await Promise.all([loadDownloadBackups(), loadExistingAccounts()])
    }
  }

  // ── Download folder scanning ─────────────────────────────────────────────

  let downloadScanError = ''

  async function loadDownloadBackups() {
    if (!hasGameExe) { downloadBackups = []; downloadScanError = ''; return }
    loadingDownloads = true
    downloadScanError = ''
    try {
      downloadBackups = await invoke<BackupFileEntry[]>('scan_download_backups', {
        gameExe: $appConfig.game_exe,
      })
    } catch (e) {
      downloadBackups = []
      downloadScanError = String(e)
    } finally {
      loadingDownloads = false
    }
  }

  async function loadExistingAccounts() {
    if (!hasServerExe) { existingAccounts = []; return }
    try {
      existingAccounts = await invoke<AccountEntry[]>('list_accounts_for_import', {
        serverExe: $appConfig.server_exe,
      })
    } catch {
      existingAccounts = []
    }
  }

  // ── File selection ───────────────────────────────────────────────────────

  async function pickImportFile() {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'JSON Account Export', extensions: ['json'] }],
    })
    if (typeof selected === 'string') await loadImportFile(selected)
  }

  async function loadImportFile(path: string) {
    importParseError = ''
    importEmailError = ''
    importNameError = ''
    importError = ''
    try {
      const result = await invoke<ImportSummary>('parse_import_json', { jsonPath: path })
      importFilePath = path
      importSummary = result
      importServerName = ''
      importPlayerName = result.player_name
      importEmail = result.email
      importPassword = ''
      if (existingAccounts.length === 0) await loadExistingAccounts()
    } catch (e) {
      importSummary = null
      importParseError = String(e)
    }
  }

  // ── Import + create profile ──────────────────────────────────────────────

  async function runImport() {
    if (!canImport || !importSummary) return
    importEmailError = ''
    importNameError = ''
    importError = ''
    importSaving = true

    const finalEmail = importEmail.trim() || importSummary.email

    try {
      await invoke('import_account', {
        serverExe: $appConfig.server_exe,
        jsonPath: importFilePath,
        mode: 'add',
        targetId: null,
        overrides: {
          email: importEmail.trim() || null,
          player_name: importPlayerName.trim() || null,
          new_password: importPassword.trim(),
        },
      })

      const entry: Server = {
        id: crypto.randomUUID(),
        name: importServerName.trim(),
        host: '',
        email: finalEmail,
        is_local: true,
        use_https: false,
      }
      await upsertServer(entry, importPassword.trim())
      onClose()
    } catch (e) {
      const msg = String(e)
      if (msg.startsWith('EMAIL_CONFLICT:')) {
        importEmailError = msg.slice('EMAIL_CONFLICT:'.length)
      } else if (msg.startsWith('NAME_CONFLICT:')) {
        importNameError = msg.slice('NAME_CONFLICT:'.length)
      } else {
        importError = msg
      }
    } finally {
      importSaving = false
    }
  }

  // ── Restore: file selection ──────────────────────────────────────────────

  async function pickRestoreFile() {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'JSON Account Export', extensions: ['json'] }],
    })
    if (typeof selected === 'string') await loadRestoreFile(selected)
  }

  async function loadRestoreFile(path: string) {
    restoreParseError = ''
    restoreError = ''
    try {
      const result = await invoke<ImportSummary>('parse_import_json', { jsonPath: path })
      restoreFilePath = path
      restoreSummary = result
    } catch (e) {
      restoreSummary = null
      restoreParseError = String(e)
    }
  }

  // ── Restore: run ─────────────────────────────────────────────────────────

  async function runRestore() {
    if (!canRestore || !restoreTargetAccount) return
    restoreError = ''
    restoreSaving = true
    try {
      await invoke('import_account', {
        serverExe: $appConfig.server_exe,
        jsonPath: restoreFilePath,
        mode: 'replace',
        targetId: restoreTargetAccount.id,
        overrides: null,
      })
      onClose()
    } catch (e) {
      restoreError = String(e)
    } finally {
      restoreSaving = false
    }
  }

  function fileName(path: string): string {
    return path.split(/[\\/]/).pop() ?? path
  }

  function formatDate(iso: string | null): string {
    if (!iso) return ''
    return new Date(iso).toLocaleDateString(undefined, { day: 'numeric', month: 'short' })
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onClose()
  }

  function submit() {
    if (activeTab === 'manual') {
      save()
    } else if (activeTab === 'import') {
      runImport()
    } else if (activeTab === 'restore') {
      runRestore()
    }
  }

  // ── Tooltip ──────────────────────────────────────────────────────────────

  let tooltip = ''
  let tooltipX = 0
  let tooltipY = 0
  let tooltipVisible = false
  let tooltipEl: HTMLDivElement | null = null

  const TOOLTIP_OFFSET = 12
  const VIEWPORT_PAD = 10

  async function showTooltip(e: MouseEvent, text: string) {
    tooltip = text
    tooltipVisible = true
    await tick()
    if (!tooltipEl) return
    const rect = tooltipEl.getBoundingClientRect()
    const vw = window.innerWidth
    const vh = window.innerHeight
    let x = e.clientX + TOOLTIP_OFFSET
    let y = e.clientY - 8
    if (x + rect.width + VIEWPORT_PAD > vw) x = e.clientX - rect.width - TOOLTIP_OFFSET
    x = Math.max(VIEWPORT_PAD, Math.min(x, vw - rect.width - VIEWPORT_PAD))
    if (y < VIEWPORT_PAD) y = e.clientY + TOOLTIP_OFFSET
    y = Math.max(VIEWPORT_PAD, Math.min(y, vh - rect.height - VIEWPORT_PAD))
    tooltipX = x
    tooltipY = y
  }

  function hideTooltip() {
    tooltipVisible = false
  }
</script>

<svelte:window on:keydown={onKeydown} />

{#if tooltipVisible}
  <div bind:this={tooltipEl} class="tooltip" style="left:{tooltipX}px; top:{tooltipY}px">
    {tooltip}
  </div>
{/if}

<div class="modal-backdrop" role="dialog" aria-modal="true">
  <div class="modal">

    <div class="modal-header">
      <div class="modal-title">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <rect x="2" y="3" width="20" height="18" rx="2"/>
          <line x1="2" y1="9" x2="22" y2="9"/>
          <circle cx="7" cy="6" r="1" fill="currentColor" stroke="none"/>
          <circle cx="11" cy="6" r="1" fill="currentColor" stroke="none"/>
        </svg>
        <span>{isEdit ? 'Edit Server' : 'Add Server'}</span>
      </div>
      <button class="close-btn" aria-label="Close" on:click={onClose}>
        <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5">
          <line x1="3.5" y1="3.5" x2="10.5" y2="10.5"/>
          <line x1="10.5" y1="3.5" x2="3.5" y2="10.5"/>
        </svg>
      </button>
    </div>

    {#if !isEdit}
      <div class="tab-bar">
        <button
          type="button"
          class="tab-btn"
          class:active={activeTab === 'manual'}
          on:click={() => selectTab('manual')}
        >
          Manual
        </button>
        <button
          type="button"
          class="tab-btn"
          class:active={activeTab === 'import'}
          on:click={() => selectTab('import')}
        >
          Import Account
        </button>
      </div>
    {:else if isLocal}
      <div class="tab-bar">
        <button
          type="button"
          class="tab-btn"
          class:active={activeTab === 'manual'}
          on:click={() => selectTab('manual')}
        >
          Manual
        </button>
        <button
          type="button"
          class="tab-btn"
          class:active={activeTab === 'restore'}
          on:click={() => selectTab('restore')}
        >
          Restore Backup
        </button>
      </div>
    {/if}

    <div class="modal-body">

      {#if activeTab === 'manual'}
        <div class="form-group">
          <label class="field-label" for="modal-name">Name</label>
          <input id="modal-name" type="text" bind:value={name} placeholder="Local Server">
        </div>

        <label class="check-row">
          <input type="checkbox" bind:checked={isLocal}>
          <span class="check-label">Local</span>
          <button
            class="info-btn"
            type="button"
            on:mouseenter={(e) => showTooltip(e, 'Server runs on this machine - port and dashboard path are read from MHServerEmu Config.')}
            on:mouseleave={hideTooltip}
            tabindex="-1"
          >?</button>
        </label>

        {#if !isLocal}
          <div class="form-group">
            <div class="label-row">
              <label class="field-label" for="modal-host">Host / IP</label>
              <button
                class="info-btn"
                type="button"
                on:mouseenter={(e) => showTooltip(e, 'Hostname or IP address only - no http:// prefix or path suffix required. Include a port as hostname:port if not on standard 80/443.')}
                on:mouseleave={hideTooltip}
                tabindex="-1"
              >?</button>
            </div>
            <input id="modal-host" type="text" bind:value={host} placeholder="e.g. 192.168.xxx, mhphoenix.net, &lt;hostname&gt;:&lt;port&gt;">
          </div>
          <label class="check-row">
            <input type="checkbox" bind:checked={useHttps}>
            <span class="check-label">Use HTTPS</span>
            <button
              class="info-btn"
              type="button"
              on:mouseenter={(e) => showTooltip(e, 'Use HTTPS for the dashboard URL and SiteConfig requests. Requires SSL to be configured on the remote server.')}
              on:mouseleave={hideTooltip}
              tabindex="-1"
            >?</button>
          </label>
        {/if}

        <div class="form-group">
          <label class="field-label" for="modal-email">Email</label>
          <input id="modal-email" type="text" bind:value={email} placeholder="player1@local.host">
        </div>

        <div class="form-group">
          <label class="field-label" for="modal-password">
            Password
            {#if isEdit}<span class="hint">(leave blank to keep existing)</span>{/if}
          </label>
          <input id="modal-password" type="password" bind:value={password} placeholder={isEdit ? '............' : 'Required'}>
        </div>

        {#if error}
          <div class="error">{error}</div>
        {/if}

      {:else if activeTab === 'import'}
        <!-- ── Import tab ── -->

        <div class="import-note">
          <i class="note-icon">i</i>
          Creates a local server profile from an account export file.
        </div>

        <div class="form-group">
          <div class="label-row">
            <span class="field-label">From Download folder</span>
          </div>
          {#if !hasGameExe}
            <div class="hint-block">Set the game executable path in Settings to enable this.</div>
          {:else if loadingDownloads}
            <div class="hint-block">Scanning...</div>
          {:else if downloadScanError}
            <div class="error">{downloadScanError}</div>
          {:else if downloadBackups.length === 0}
            <div class="hint-block">No backups found. Use Upload File below.</div>
          {:else}
            <div class="file-list">
              {#each downloadBackups as backup (backup.path)}
                <button
                  type="button"
                  class="file-row"
                  class:selected={importFilePath === backup.path}
                  on:click={() => loadImportFile(backup.path)}
                >
                  <span class="file-name">{backup.player_name}</span>
                  <span class="file-date">{formatDate(backup.modified)}</span>
                </button>
              {/each}
            </div>
          {/if}
        </div>

        <button type="button" class="btn btn-outline btn-sm" on:click={pickImportFile}>
          Upload File
        </button>

        {#if importParseError}
          <div class="error">{importParseError}</div>
        {/if}

        {#if importSummary}
          <div class="import-summary">
            <div class="summary-row">
              <span class="summary-label">File</span>
              <span class="summary-value mono" title={importFilePath}>{fileName(importFilePath)}</span>
            </div>
            <div class="summary-row">
              <span class="summary-label">Player</span>
              <span class="summary-value">{importSummary.player_name}</span>
            </div>
            <div class="summary-row">
              <span class="summary-label">Email</span>
              <span class="summary-value mono">{importSummary.email}</span>
            </div>
            <div class="summary-row">
              <span class="summary-label">Contents</span>
              <span class="summary-value">
                {importSummary.avatar_count} avatars, {importSummary.team_up_count} team-ups, {importSummary.item_count} items
              </span>
            </div>
          </div>

          <div class="form-group">
            <label class="field-label" for="modal-import-server-name">Server Name</label>
            <input id="modal-import-server-name" type="text" bind:value={importServerName} placeholder={importSummary.player_name}>
          </div>

          <div class="form-group">
            <label class="field-label" for="modal-import-player-name">Player Name</label>
            <input
              id="modal-import-player-name"
              type="text"
              bind:value={importPlayerName}
              class:field-error={!!importNameError}
              on:input={() => (importNameError = '')}
            >
            {#if importNameError}<span class="field-error-msg">{importNameError}</span>{/if}
          </div>

          <div class="form-group">
            <label class="field-label" for="modal-import-email">Email</label>
            <input
              id="modal-import-email"
              type="text"
              bind:value={importEmail}
              class:field-error={!!importEmailError}
              on:input={() => (importEmailError = '')}
            >
            {#if importEmailError}<span class="field-error-msg">{importEmailError}</span>{/if}
          </div>

          <div class="form-group">
            <label class="field-label" for="modal-import-password">Password</label>
            <input id="modal-import-password" type="password" bind:value={importPassword} placeholder="Required">
          </div>

          {#if importConflictMsg}
            <div class="error">{importConflictMsg}</div>
          {/if}

          {#if $serverRunning}
            <div class="warning-notice">Stop the server before importing.</div>
          {/if}

          {#if importError}
            <div class="error">{importError}</div>
          {/if}
        {/if}

      {:else if activeTab === 'restore'}
        <!-- ── Restore tab ── -->

        <div class="import-note note-warning">
          <i class="note-icon">!</i>
          Restoring overwrites this account's current avatars, items, and progress. This cannot be undone.
        </div>

        {#if !restoreTargetAccount}
          <div class="hint-block">No account in Account.db matches {server?.email ?? 'this profile\'s email'}.</div>
        {:else}
          <div class="form-group">
            <span class="field-label">From Download folder</span>
            {#if !hasGameExe}
              <div class="hint-block">Set the game executable path in Settings to enable this.</div>
            {:else if loadingDownloads}
              <div class="hint-block">Scanning...</div>
            {:else if downloadScanError}
              <div class="error">{downloadScanError}</div>
            {:else if downloadBackups.length === 0}
              <div class="hint-block">No backups found. Use Upload File below.</div>
            {:else}
              <div class="file-list">
                {#each downloadBackups as backup (backup.path)}
                  <button
                    type="button"
                    class="file-row"
                    class:selected={restoreFilePath === backup.path}
                    on:click={() => loadRestoreFile(backup.path)}
                  >
                    <span class="file-name">{backup.player_name}</span>
                    <span class="file-date">{formatDate(backup.modified)}</span>
                  </button>
                {/each}
              </div>
            {/if}
          </div>

          <button type="button" class="btn btn-outline btn-sm" on:click={pickRestoreFile}>
            Upload File
          </button>

          {#if restoreParseError}
            <div class="error">{restoreParseError}</div>
          {/if}

          {#if restoreSummary}
            <div class="import-summary">
              <div class="summary-row">
                <span class="summary-label">File</span>
                <span class="summary-value mono" title={restoreFilePath}>{fileName(restoreFilePath)}</span>
              </div>
              <div class="summary-row">
                <span class="summary-label">Player</span>
                <span class="summary-value">{restoreSummary.player_name}</span>
              </div>
              <div class="summary-row">
                <span class="summary-label">Email</span>
                <span class="summary-value mono">{restoreSummary.email}</span>
              </div>
              <div class="summary-row">
                <span class="summary-label">Contents</span>
                <span class="summary-value">
                  {restoreSummary.avatar_count} avatars, {restoreSummary.team_up_count} team-ups, {restoreSummary.item_count} items
                </span>
              </div>
            </div>

            {#if restoreMismatchMsg}
              <div class="error">{restoreMismatchMsg}</div>
            {/if}

            {#if $serverRunning}
              <div class="warning-notice">Stop the server before restoring.</div>
            {/if}

            {#if restoreError}
              <div class="error">{restoreError}</div>
            {/if}
          {/if}
        {/if}
      {/if}
    </div>

    <div class="modal-footer">
      <button
        type="button"
        class="btn btn-outline"
        on:click={onClose}
        disabled={saving || importSaving || restoreSaving}
      >Cancel</button>
      {#if activeTab === 'manual'}
        <button type="button" class="btn btn-accent" on:click={submit} disabled={saving}>
          {saving ? 'Saving...' : isEdit ? 'Save Changes' : 'Add Server'}
        </button>
      {:else if activeTab === 'import'}
        <button type="button" class="btn btn-accent" on:click={submit} disabled={!canImport || importSaving}>
          {importSaving ? 'Importing...' : 'Add Account'}
        </button>
      {:else if activeTab === 'restore'}
        <button type="button" class="btn btn-accent" on:click={submit} disabled={!canRestore || restoreSaving}>
          {restoreSaving ? 'Restoring...' : 'Restore Backup'}
        </button>
      {/if}
    </div>

  </div>
</div>

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.65);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    backdrop-filter: blur(2px);
  }

  .modal {
    background: var(--bg-2);
    border: 1px solid var(--border-lit);
    border-radius: var(--radius-md);
    width: 500px;
    max-height: 85vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 24px 80px rgba(0, 0, 0, 0.5), 0 0 0 1px rgba(255, 255, 255, 0.03) inset;
  }

  .modal-header {
    padding: 14px 16px;
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .modal-title {
    display: flex;
    align-items: center;
    gap: 10px;
    font-family: var(--font-head);
    font-size: 14px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-0);
  }
  .modal-title svg {
    width: 18px;
    height: 18px;
    color: var(--accent);
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

  /* ── Tabs ── */
  .tab-bar {
    display: flex;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }
  .tab-btn {
    flex: 1;
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    border-radius: 0;
    padding: 10px;
    font-family: var(--font-head);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
    cursor: pointer;
    transition: color 0.12s, border-color 0.12s;
  }
  .tab-btn:hover { color: var(--text-1); }
  .tab-btn.active {
    color: var(--accent-bright);
    border-bottom-color: var(--accent);
  }

  .modal-body {
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    overflow-y: auto;
    min-height: 0;
  }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .label-row {
    display: flex;
    align-items: center;
    gap: 5px;
  }
  .label-row .field-label {
    margin-bottom: 0;
  }

  .hint {
    font-family: var(--font-body);
    font-size: 10px;
    color: var(--text-3);
    text-transform: none;
    letter-spacing: 0;
    margin-left: 6px;
  }

  .error {
    font-size: 12px;
    color: #e74c3c;
    padding: 8px 10px;
    border: 1px solid rgba(192, 57, 43, 0.4);
    background: var(--red-dim);
    border-radius: var(--radius-sm);
  }

  .field-error {
    border-color: rgba(192, 57, 43, 0.6);
  }

  .field-error-msg {
    font-size: 11px;
    color: #e74c3c;
  }

  .modal-footer {
    padding: 12px 16px;
    border-top: 1px solid var(--border);
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    flex-shrink: 0;
  }

  /* ── Checkbox rows ── */
  .check-row {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    user-select: none;
  }
  .check-row input[type="checkbox"] {
    width: 12px;
    height: 12px;
    cursor: pointer;
    accent-color: var(--accent);
    flex-shrink: 0;
  }
  .check-label {
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-1);
  }

  /* ── Info button ── */
  .info-btn {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    border: 1px solid var(--border-lit);
    background: var(--bg-3);
    color: var(--text-3);
    font-size: 9px;
    cursor: help;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    transition: all 0.12s;
    padding: 0;
  }
  .info-btn:hover { border-color: var(--accent-dim); color: var(--accent-bright); }

  /* ── Import tab ── */
  .import-note {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    background: var(--bg-3);
    border-radius: var(--radius-sm);
    font-size: 12px;
    color: var(--text-2);
  }
  .import-note.note-warning {
    background: var(--amber-dim);
    color: var(--amber-bright);
  }
  .import-note.note-warning .note-icon {
    border-color: rgba(200, 146, 10, 0.4);
    color: var(--amber-bright);
  }
  .note-icon {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
    border-radius: 50%;
    border: 1px solid var(--border-lit);
    display: flex;
    align-items: center;
    justify-content: center;
    font-style: normal;
    font-size: 10px;
    color: var(--text-3);
  }

  .hint-block {
    font-size: 12px;
    color: var(--text-3);
    padding: 6px 0;
  }

  .file-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-height: 140px;
    overflow-y: auto;
  }

  .file-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 7px 10px;
    background: var(--bg-3);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    cursor: pointer;
    text-align: left;
    transition: border-color 0.1s, background 0.1s;
  }
  .file-row:hover { background: var(--bg-2); border-color: var(--border-mid); }
  .file-row.selected {
    border-color: var(--accent-dim);
    background: var(--accent-glow);
  }

  .file-name {
    font-size: 12px;
    color: var(--text-1);
  }
  .file-date {
    font-size: 11px;
    color: var(--text-3);
    flex-shrink: 0;
  }

  .import-summary {
    background: var(--bg-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .summary-row {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }
  .summary-label {
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-3);
    width: 56px;
    flex-shrink: 0;
  }
  .summary-value {
    font-size: 12px;
    color: var(--text-1);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .summary-value.mono { font-family: var(--font-mono); }

  .warning-notice {
    padding: 8px 12px;
    background: var(--amber-dim);
    border: 1px solid rgba(200,146,10,0.3);
    border-radius: var(--radius-sm);
    font-size: 12px;
    color: var(--amber-bright);
  }

  .btn-sm { padding: 5px 10px; font-size: 11px; align-self: flex-start; }

  /* ── Tooltip ── */
  .tooltip {
    position: fixed;
    z-index: calc(var(--z-modal) + 1);
    background: var(--bg-3);
    border: 1px solid var(--border-lit);
    border-radius: var(--radius-sm);
    color: var(--text-1);
    font-size: 12px;
    font-family: var(--font-body);
    padding: 6px 10px;
    max-width: 280px;
    line-height: 1.5;
    pointer-events: none;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
  }
</style>