<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { open } from '@tauri-apps/plugin-dialog'
  import { appConfig, activeTab as appTab, serverRunning, upsertServer, jsonPersistence, refreshPersistence, type Server } from '../lib/store'
  import { syncAccount } from '../lib/accountCommands'
  import { tip } from '../lib/tooltip'

  export let server: Server | null = null
  export let initialTab: 'remote' | 'local' | null = null
  export let onClose: () => void

  const isEdit = server !== null

  // ── Manual tab fields ────────────────────────────────────────────────────

  let name = server?.name ?? ''
  let host = server?.host ?? ''
  let email = server?.email ?? ''
  let password = ''
  let saving = false
  let error = ''
  // A new profile starts on Local once there is a server to run it on, otherwise on Remote.
  let isLocal  = server?.is_local  ?? (initialTab ? initialTab === 'local' : !!$appConfig.server_exe)
  let useHttps = server?.use_https ?? false

  // `account`: 'sync' makes the server account match first (Local, Account.db servers only),
  // 'overwrite' also sets the password on an existing account, 'skip' saves the profile as is.
  async function save(account: 'sync' | 'overwrite' | 'skip' = 'sync') {
    name = name.trim()
    host = host.trim()
    email = email.trim()
    password = password.trim()
    checkEmail()

    if (!name) { error = 'Name is required.'; return }
    if (!isLocal && !host) { error = 'Host is required for remote servers.'; return }
    if (!isEdit && !password) { error = 'Password is required for a new server.'; return }
    const dup = sameAccountAs(email, isLocal, host, $appConfig.servers)
    if (dup) { error = sameAccountText(dup); return }

    saving = true
    error = ''
    acctError = ''
    acctMismatch = false
    acctTimedOut = false
    acctWarning = ''

    try {
      // A blank password in edit mode keeps the saved one, so there is nothing to sync.
      if (canManageAccount && password && account !== 'skip') {
        acctMsg = 'Checking the account...'
        const r = await syncAccount(
          { email, playerName: playerName.trim(), password, makeAdmin, overwritePassword: ownAccount || account === 'overwrite' },
          { onStatus: m => { acctMsg = m } },
        )
        acctMsg = ''
        if (!r.ok) {
          acctError = r.error
          acctMismatch = !!r.mismatch
          acctTimedOut = !!r.timedOut
          return
        }
        // Shown only if the profile save below fails; otherwise the modal closes.
        if (r.created) acctMsg = 'The account was created on the server.'
        if (r.warning) acctWarning = r.warning
      }
      const entry: Server = {
        id: server?.id ?? crypto.randomUUID(),
        name,
        host: isLocal ? '' : host,
        email,
        is_local: isLocal,
        use_https: useHttps,
      }
      await upsertServer(entry, password)
      if (acctWarning) { acctMsg = ''; return }
      onClose()
    } catch (e) {
      error = String(e)
    } finally {
      saving = false
    }
  }

  // ── Tabs ─────────────────────────────────────────────────────────────────
  // Remote and Local are the same 'manual' form with isLocal switched, so the entered
  // fields carry across. From Backup imports into a new local profile when adding, and
  // restores into the existing account when editing a profile that was saved as local.

  type TabId = 'manual' | 'import' | 'restore'
  let activeTab: TabId = 'manual'

  // JSON storage has one built-in account and accepts any login, so a new local profile can
  // start filled in. Tracked so switching to Remote does not leave those values behind.
  const JSON_ACCOUNT_EMAIL = 'default@mhserveremu'
  const JSON_PLACEHOLDER_PASSWORD = 'local'
  let jsonPrefilled = false
  $: refreshPersistence($appConfig.server_exe)
  $: if (!isEdit && isLocal && $jsonPersistence && !jsonPrefilled) {
    jsonPrefilled = true
    if (!email) email = JSON_ACCOUNT_EMAIL
    checkEmail()
    if (!password) password = JSON_PLACEHOLDER_PASSWORD
  }

  function selectKind(local: boolean) {
    if (!local && jsonPrefilled) {
      if (email === JSON_ACCOUNT_EMAIL) email = ''
      if (password === JSON_PLACEHOLDER_PASSWORD) password = ''
      jsonPrefilled = false
      checkEmail()
    }
    isLocal = local
    activeTab = 'manual'
  }

  // Another Multiverse profile that logs into the same account: the same email on the local
  // server (every local profile uses the one linked MHServerEmu), or on the same remote host.
  const lower = (s: string) => s.trim().toLowerCase()
  const sameAccountAs = (e: string, local: boolean, h: string, servers: Server[]) => !e.trim() ? null :
    servers.find(s => s.id !== server?.id && s.is_local === local && (local || lower(s.host) === lower(h)) && lower(s.email) === lower(e)) ?? null
  const sameAccountText = (s: Server) =>
    `Your profile "${s.name}" already signs in with this email ${s.is_local ? 'on the local server' : 'on this host'}.`

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
    id: string
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
    ($jsonPersistence
      ? !!localProfile || !!importServerName.trim()
      : !!importPassword.trim() && !!importServerName.trim() && !importConflictMsg)

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
    !$serverRunning &&
    hasServerExe &&
    ($jsonPersistence || (!!restoreTargetAccount && !restoreMismatchMsg))

  // ── From Backup on JSON-storage servers ──────────────────────────────────
  // restore_json_account replaces the server's one save. Every local profile signs in to that
  // single account, so adding from a backup only creates a profile when there is no local one.

  const JSON_RESTORE_WARNING = "Restoring replaces all of this server's saved progress with the backup and makes it an admin account. A copy of the current save is kept in the server's Data folder."
  $: localProfile = $appConfig.servers.find(s => s.is_local) ?? null
  let jsonRestoreDone = ''

  async function runJsonRestore(path: string, profileName: string | null) {
    const copy = await invoke<string | null>('restore_json_account', { serverExe: $appConfig.server_exe, jsonPath: path })
    jsonRestoreDone = copy
      ? `Backup restored. The previous save was copied to Data/${copy}.`
      : 'Backup restored. There was no previous save to copy.'
    if (profileName) {
      const entry: Server = { id: crypto.randomUUID(), name: profileName, host: '', email: JSON_ACCOUNT_EMAIL, is_local: true, use_https: false }
      await upsertServer(entry, JSON_PLACEHOLDER_PASSWORD)
    }
  }

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
      if ($jsonPersistence) {
        await runJsonRestore(importFilePath, localProfile ? null : importServerName.trim())
        return
      }
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
    if (!canRestore) return
    restoreError = ''
    restoreSaving = true
    try {
      if ($jsonPersistence) {
        await runJsonRestore(restoreFilePath, null)
        return
      }
      if (!restoreTargetAccount) return
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

  $: busy = saving || importSaving || restoreSaving

  // ── Server account (Local, Account.db servers only) ───────────────────
  // Saving keeps the server account in step with the profile, see save().
  let playerName = ''
  let makeAdmin = true
  let acctMsg = ''
  let acctError = ''
  let acctMismatch = false
  let acctTimedOut = false
  let acctWarning = ''
  $: canManageAccount = isLocal && hasServerExe && !$jsonPersistence
  $: if (canManageAccount) loadExistingAccounts()
  // Empty until the server's first start creates Account.db, so a fresh install asks for a player name.
  // The email is checked after a pause in typing, on leaving the field and on save, not per
  // keystroke: every partial email is "not on the server", which would flash the extra fields.
  let checkedEmail = email.trim()
  let emailTimer: ReturnType<typeof setTimeout> | undefined
  function checkEmail() {
    clearTimeout(emailTimer)
    checkedEmail = email.trim()
  }
  function checkEmailSoon() {
    clearTimeout(emailTimer)
    emailTimer = setTimeout(checkEmail, 600)
  }
  $: sameAccountProfile = sameAccountAs(checkedEmail, isLocal, host, $appConfig.servers)
  $: knownAccount = existingAccounts.find(a => a.email.toLowerCase() === checkedEmail.toLowerCase()) ?? null
  $: newAccount = canManageAccount && !!checkedEmail && !knownAccount && (!isEdit || !!password.trim())
  // Editing a profile's own password changes it on the server; pointing a profile at some
  // other existing account never resets that account's password without asking.
  $: ownAccount = isEdit && lower(email) === lower(server?.email ?? '')
  // Says up front whether saving writes to MHServerEmu, since the modal just closes afterwards.
  $: serverEffect = !canManageAccount || !knownAccount ? '' : `Existing account: ${knownAccount.player_name}. ` + (
    ownAccount && password.trim() ? "If this password differs from the server's, saving changes it there too."
    : isEdit ? 'Saving changes this profile only. Nothing on the server changes.'
    : 'Saving adds a Multiverse profile only. Nothing on the server changes.')
  $: saveLabel = saving ? 'Saving...' : newAccount ? 'Create Account & Save' : isEdit ? 'Save Changes' : 'Add Server'

  function openConsole() {
    onClose()
    appTab.set('server')
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && !busy) onClose()
  }

  function submit() {
    if (jsonRestoreDone) {
      onClose()
    } else if (activeTab === 'manual') {
      if (acctWarning) onClose()
      else save()
    } else if (activeTab === 'import') {
      runImport()
    } else if (activeTab === 'restore') {
      runRestore()
    }
  }
</script>

<svelte:window on:keydown={onKeydown} />

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
        <span class="modal-display-name">{isEdit ? 'Edit Server' : 'Add Server'}</span>
      </div>
      <button class="close-btn" aria-label="Close" on:click={onClose} disabled={busy}>
        <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5">
          <line x1="3.5" y1="3.5" x2="10.5" y2="10.5"/>
          <line x1="10.5" y1="3.5" x2="3.5" y2="10.5"/>
        </svg>
      </button>
    </div>

    <div class="tab-bar">
      <button
        type="button"
        class="tab-btn"
        class:active={activeTab === 'manual' && !isLocal}
        on:click={() => selectKind(false)}
      >
        Remote
      </button>
      <button
        type="button"
        class="tab-btn"
        class:active={activeTab === 'manual' && isLocal}
        on:click={() => selectKind(true)}
      >
        Local
      </button>
      {#if !isEdit}
        <button
          type="button"
          class="tab-btn"
          class:active={activeTab === 'import'}
          on:click={() => selectTab('import')}
        >
          From Backup
        </button>
      {:else if server?.is_local}
        <button
          type="button"
          class="tab-btn"
          class:active={activeTab === 'restore'}
          on:click={() => { isLocal = true; selectTab('restore') }}
        >
          From Backup
        </button>
      {/if}
    </div>

    <div class="modal-body">

      {#if jsonRestoreDone}
        <div class="import-note" role="status">
          <i class="note-icon">i</i>
          {jsonRestoreDone}
        </div>
        {#if importError}
          <div class="error">The backup was restored, but the profile could not be saved: {importError}</div>
        {/if}

      {:else if activeTab === 'manual'}
        <div class="form-group">
          <label class="field-label" for="modal-name">Name</label>
          <input id="modal-name" type="text" bind:value={name} placeholder={isLocal ? 'Local Server' : 'Server name'}>
        </div>

        {#if !isLocal}
          <div class="form-group">
            <div class="label-row">
              <label class="field-label" for="modal-host">Host / IP</label>
              <button
                class="info-btn"
                type="button"
                use:tip={'Hostname or IP address only - no http:// prefix or path suffix required. Include a port as hostname:port if not on standard 80/443.'}
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
              use:tip={'Use HTTPS for the dashboard URL and SiteConfig requests. Requires SSL to be configured on the remote server.'}
              tabindex="-1"
            >?</button>
          </label>
        {/if}

        {#if !isEdit && isLocal && $jsonPersistence}
          <div class="import-note">
            <i class="note-icon">i</i>
            This server stores data in a JSON file with one built-in account, and any email and password logs in. Set the player name in the Config tab.
          </div>
        {/if}

        <div class="form-group">
          <label class="field-label" for="modal-email">Email</label>
          <input id="modal-email" type="text" bind:value={email} on:input={checkEmailSoon} on:blur={checkEmail} placeholder="player1@local.host">
          {#if sameAccountProfile}
            <div class="hint-block">{sameAccountText(sameAccountProfile)}</div>
          {:else if serverEffect}
            <div class="hint-block">{serverEffect}</div>
          {/if}
        </div>

        {#if newAccount}
          <div class="form-group">
            <label class="field-label" for="modal-player-name">Player name</label>
            <input id="modal-player-name" type="text" bind:value={playerName} placeholder="Your in-game name">
          </div>
        {/if}

        <div class="form-group">
          <label class="field-label" for="modal-password">
            Password
            {#if isEdit}<span class="hint">(leave blank to keep existing)</span>{/if}
          </label>
          <input id="modal-password" type="password" bind:value={password} placeholder={isEdit ? '............' : 'Required'}>
        </div>

        {#if newAccount}
          <label class="check-row">
            <input type="checkbox" bind:checked={makeAdmin}>
            <span class="check-label">Make it an admin account</span>
            <button
              class="info-btn"
              type="button"
              use:tip={'Admins can use server commands in game. Fine for a server only you play on.'}
              tabindex="-1"
            >?</button>
          </label>
          <div class="hint-block">Saving creates this account on your server, starting the server if it isn't running.</div>
        {/if}

        {#if acctMsg}
          <div class="account-status" role="status">
            {#if saving}<span class="spinner" aria-hidden="true"></span>{/if}{acctMsg}
          </div>
        {/if}
        {#if acctWarning}<div class="warning-notice">{acctWarning} The profile was saved.</div>{/if}
        {#if acctError}
          <div class="error">
            {acctError}
            {#if acctMismatch}<button type="button" class="link-btn" on:click={() => save('overwrite')}>Set this password on the server</button>{/if}
            {#if acctTimedOut}<button type="button" class="link-btn" on:click={openConsole}>Open Server console</button>{/if}
            <button type="button" class="link-btn" on:click={() => save('skip')}>Save profile anyway</button>
          </div>
        {/if}

        {#if error}
          <div class="error">{error}</div>
        {/if}

      {:else if activeTab === 'import'}
        <!-- ── Import tab ── -->

        <div class="import-note">
          <i class="note-icon">i</i>
          {#if !$jsonPersistence}
            Creates a local server profile from an account export file.
          {:else if localProfile}
            Restores into the server's built-in account, which your profile "{localProfile.name}" signs in to.
          {:else}
            Restores into the server's built-in account and adds a local profile for it.
          {/if}
        </div>

        {#if $jsonPersistence}
          <div class="import-note note-warning">
            <i class="note-icon">!</i>
            {JSON_RESTORE_WARNING}
          </div>
        {/if}

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
              <span class="summary-value mono" use:tip={importFilePath}>{fileName(importFilePath)}</span>
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

          {#if !$jsonPersistence || !localProfile}
          <div class="form-group">
            <label class="field-label" for="modal-import-server-name">Server Name</label>
            <input id="modal-import-server-name" type="text" bind:value={importServerName} placeholder={importSummary.player_name}>
          </div>
          {/if}

          {#if $jsonPersistence}
          <div class="hint-block">The player name becomes {importSummary.player_name} ([JsonDBManager] PlayerName in Config).</div>
          {:else}
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
          {/if}

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
          {$jsonPersistence ? JSON_RESTORE_WARNING : "Restoring overwrites this account's current avatars, items, and progress. This cannot be undone."}
        </div>

        {#if !restoreTargetAccount && !$jsonPersistence}
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
                <span class="summary-value mono" use:tip={restoreFilePath}>{fileName(restoreFilePath)}</span>
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

            {#if $jsonPersistence}
              <div class="hint-block">The player name becomes {restoreSummary.player_name} ([JsonDBManager] PlayerName in Config).</div>
            {/if}

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
        disabled={busy}
      >Cancel</button>
      {#if jsonRestoreDone}
        <button type="button" class="btn btn-accent" on:click={submit}>Done</button>
      {:else if activeTab === 'manual'}
        <button type="button" class="btn btn-accent" on:click={submit} disabled={saving}>
          {acctWarning ? 'Done' : saveLabel}
        </button>
      {:else if activeTab === 'import'}
        <button type="button" class="btn btn-accent" on:click={submit} disabled={!canImport || importSaving}>
          {importSaving ? ($jsonPersistence ? 'Restoring...' : 'Importing...') : $jsonPersistence ? 'Restore Backup' : 'Add Account'}
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
  .modal {
    width: 500px;
    max-height: 85vh;
  }

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
    font-size: var(--fs-min);
    color: var(--text-3);
    text-transform: none;
    letter-spacing: 0;
    margin-left: 6px;
  }

  .error {
    font-size: 12px;
    color: var(--text-error);
    padding: 8px 10px;
    border: 1px solid color-mix(in srgb, var(--red) 40%, transparent);
    background: var(--red-dim);
    border-radius: var(--radius-sm);
  }

  .account-status {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--text-2);
  }
  .spinner {
    width: 12px;
    height: 12px;
    flex-shrink: 0;
    border: 2px solid var(--border-lit);
    border-top-color: var(--text-1);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (prefers-reduced-motion: reduce) {
    .spinner { animation-duration: 3s; }
  }
  .link-btn {
    margin-left: 6px;
    padding: 0;
    background: none;
    border: none;
    color: inherit;
    font: inherit;
    text-decoration: underline;
    cursor: pointer;
  }

  .field-error {
    border-color: color-mix(in srgb, var(--red) 60%, transparent);
  }

  .field-error-msg {
    font-size: 11px;
    color: var(--text-error);
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
    font-size: var(--fs-min);
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
    font-size: var(--fs-min);
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
    border-color: color-mix(in srgb, var(--amber) 40%, transparent);
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
    font-size: var(--fs-min);
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
    font-size: var(--fs-min);
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
    border: 1px solid color-mix(in srgb, var(--amber) 30%, transparent);
    border-radius: var(--radius-sm);
    font-size: 12px;
    color: var(--amber-bright);
  }

  .btn-sm { padding: 5px 10px; font-size: 11px; align-self: flex-start; }

</style>