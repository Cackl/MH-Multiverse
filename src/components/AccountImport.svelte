<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { open } from '@tauri-apps/plugin-dialog'
  import { onMount } from 'svelte'
  import { appConfig, serverRunning, upsertServer, type Server } from '../lib/store'
  import ServerModal from './ServerModal.svelte'

  // ── Types ──────────────────────────────────────────────────────────────────

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

  // ── State ──────────────────────────────────────────────────────────────────

  type Phase = 'idle' | 'parsed' | 'importing' | 'success'

  let phase: Phase = 'idle'
  let isDragging = false

  // Parsed data
  let filePath = ''
  let summary: ImportSummary | null = null
  let parseError = ''

  // Form
  let overrideEmail = ''
  let overrideName = ''
  let newPassword = ''
  let mode: 'add' | 'replace' = 'add'
  let targetId: number | null = null
  let accounts: AccountEntry[] = []
  let loadingAccounts = false

  // Field-level conflict errors
  let emailError = ''
  let nameError = ''
  let importError = ''

  // Success
  let finalEmail = ''
  let finalPassword = ''
  let showServerModal = false

  // ── Derived ────────────────────────────────────────────────────────────────

  $: hasServerExe = !!$appConfig.server_exe
  $: canImport =
    phase === 'parsed' &&
    !$serverRunning &&
    hasServerExe &&
    (mode === 'add' || targetId !== null)

  // ── File handling ──────────────────────────────────────────────────────────

  async function pickFile() {
    const selected = await open({
      multiple: false,
      filters: [{ name: 'JSON Account Export', extensions: ['json'] }],
    })
    if (typeof selected === 'string') {
      await loadFile(selected)
    }
  }

  async function loadFile(path: string) {
    parseError = ''
    emailError = ''
    nameError = ''
    importError = ''
    try {
      const result = await invoke<ImportSummary>('parse_import_json', { jsonPath: path })
      filePath = path
      summary = result
      overrideEmail = result.email
      overrideName = result.player_name
      newPassword = ''
      phase = 'parsed'
      if (hasServerExe) await fetchAccounts()
    } catch (e) {
      parseError = String(e)
    }
  }

  function handleDragOver(e: DragEvent) {
    e.preventDefault()
    isDragging = true
  }

  function handleDragLeave() {
    isDragging = false
  }

  async function handleDrop(e: DragEvent) {
    e.preventDefault()
    isDragging = false
    const file = e.dataTransfer?.files[0]
    if (!file) return
    // In Tauri 2 the File object from an OS drag-drop carries a .path property.
    const path = (file as File & { path?: string }).path
    if (path) await loadFile(path)
  }

  // ── Accounts list (for replace picker) ────────────────────────────────────

  async function fetchAccounts() {
    if (!hasServerExe) return
    loadingAccounts = true
    try {
      accounts = await invoke<AccountEntry[]>('list_accounts_for_import', {
        serverExe: $appConfig.server_exe,
      })
    } catch {
      accounts = []
    } finally {
      loadingAccounts = false
    }
  }

  // ── Import ─────────────────────────────────────────────────────────────────

  async function runImport() {
    if (!canImport) return
    emailError = ''
    nameError = ''
    importError = ''
    phase = 'importing'

    try {
      await invoke('import_account', {
        serverExe: $appConfig.server_exe,
        jsonPath: filePath,
        mode,
        targetId: mode === 'replace' ? targetId : null,
        overrides: {
          email: overrideEmail.trim() || null,
          player_name: overrideName.trim() || null,
          new_password: newPassword.trim() || null,
        },
      })
      finalEmail = overrideEmail.trim() || summary?.email || ''
      finalPassword = newPassword.trim()
      phase = 'success'
    } catch (e) {
      const msg = String(e)
      phase = 'parsed'
      if (msg.startsWith('EMAIL_CONFLICT:')) {
        emailError = msg.slice('EMAIL_CONFLICT:'.length)
      } else if (msg.startsWith('NAME_CONFLICT:')) {
        nameError = msg.slice('NAME_CONFLICT:'.length)
      } else {
        importError = msg
      }
    }
  }

  function reset() {
    phase = 'idle'
    filePath = ''
    summary = null
    parseError = ''
    emailError = ''
    nameError = ''
    importError = ''
    overrideEmail = ''
    overrideName = ''
    newPassword = ''
    mode = 'add'
    targetId = null
    finalEmail = ''
    finalPassword = ''
    showServerModal = false
  }

  // ── Helpers ────────────────────────────────────────────────────────────────

  function userLevelLabel(level: number): string {
    return level === 2 ? 'Admin' : level === 1 ? 'Moderator' : 'User'
  }

  function fileName(path: string): string {
    return path.split(/[\\/]/).pop() ?? path
  }

  onMount(() => {
    if (hasServerExe) fetchAccounts()
  })
</script>

{#if showServerModal}
  <ServerModal
    server={null}
    prefillEmail={finalEmail}
    prefillPassword={finalPassword}
    onClose={() => (showServerModal = false)}
  />
{/if}

<!-- ── Idle ── -->
{#if phase === 'idle'}
  <div class="import-idle">
    <!-- svelte-ignore a11y-interactive-supports-focus -->
    <div
      class="drop-zone"
      class:dragging={isDragging}
      role="button"
      on:click={pickFile}
      on:keydown={(e) => e.key === 'Enter' && pickFile()}
      on:dragover={handleDragOver}
      on:dragleave={handleDragLeave}
      on:drop={handleDrop}
    >
      <svg class="drop-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
        <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
        <polyline points="14 2 14 8 20 8"/>
        <line x1="12" y1="18" x2="12" y2="12"/>
        <polyline points="9 15 12 12 15 15"/>
      </svg>
      <span class="drop-label">Drop account export here or click to browse</span>
      <span class="drop-sub">.json file from <code>!account download</code></span>
    </div>
    {#if parseError}
      <div class="feedback-error">{parseError}</div>
    {/if}
    {#if !hasServerExe}
      <div class="warning-notice" style="max-width:480px;margin-top:12px;">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:14px;height:14px;flex-shrink:0;">
          <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/>
          <line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/>
        </svg>
        Configure a server executable in Settings before importing.
      </div>
    {/if}
  </div>

<!-- ── Parsed / importing ── -->
{:else if phase === 'parsed' || phase === 'importing'}
  <div class="import-columns">

    <!-- Left: summary -->
    <div class="import-left">
      <div class="subsection-title">Import Summary</div>

      <div class="info-block">
        <div class="info-row">
          <span class="info-label">File</span>
          <span class="info-value mono" title={filePath}>{fileName(filePath)}</span>
        </div>
        <div class="info-row">
          <span class="info-label">Player</span>
          <span class="info-value">{summary?.player_name}</span>
        </div>
        <div class="info-row">
          <span class="info-label">Email</span>
          <span class="info-value mono">{summary?.email}</span>
        </div>
        <div class="info-row">
          <span class="info-label">User Level</span>
          <span class="info-value">{userLevelLabel(summary?.user_level ?? 0)}</span>
        </div>
      </div>

      <div class="subsection-title" style="margin-top:4px;">Contents</div>

      <div class="info-block">
        <div class="info-row">
          <span class="info-label">Avatars</span>
          <span class="info-value mono">{summary?.avatar_count}</span>
        </div>
        <div class="info-row">
          <span class="info-label">Team-Ups</span>
          <span class="info-value mono">{summary?.team_up_count}</span>
        </div>
        <div class="info-row">
          <span class="info-label">Items</span>
          <span class="info-value mono">{summary?.item_count}</span>
        </div>
        {#if (summary?.controlled_entity_count ?? 0) > 0}
          <div class="info-row">
            <span class="info-label">Controlled</span>
            <span class="info-value mono">{summary?.controlled_entity_count}</span>
          </div>
        {/if}
      </div>

      <button class="btn btn-sm btn-outline" style="margin-top:auto;" on:click={reset} disabled={phase === 'importing'}>
        Choose Different File
      </button>
    </div>

    <!-- Right: form -->
    <div class="import-right">
      <div class="subsection-title">Import Settings</div>

      <!-- Player name -->
      <div class="form-group">
        <label class="field-label" for="imp-name">Player Name</label>
        <input
          id="imp-name"
          type="text"
          bind:value={overrideName}
          placeholder={summary?.player_name}
          disabled={phase === 'importing'}
          class:field-error={!!nameError}
          on:input={() => (nameError = '')}
        />
        {#if nameError}
          <span class="field-error-msg">{nameError}</span>
        {/if}
      </div>

      <!-- Email -->
      <div class="form-group">
        <label class="field-label" for="imp-email">Email</label>
        <input
          id="imp-email"
          type="text"
          bind:value={overrideEmail}
          placeholder={summary?.email}
          disabled={phase === 'importing'}
          class:field-error={!!emailError}
          on:input={() => (emailError = '')}
        />
        {#if emailError}
          <span class="field-error-msg">{emailError}</span>
        {/if}
      </div>

      <!-- New password -->
      <div class="form-group">
        <label class="field-label" for="imp-pw">
          New Password
          <span class="field-hint">(leave blank to keep original)</span>
        </label>
        <input
          id="imp-pw"
          type="password"
          bind:value={newPassword}
          placeholder="············"
          disabled={phase === 'importing'}
        />
      </div>

      <!-- Mode -->
      <div class="form-group">
        <span class="field-label">Import Mode</span>
        <div class="mode-row">
          <label class="mode-option" class:selected={mode === 'add'}>
            <input type="radio" bind:group={mode} value="add" disabled={phase === 'importing'} />
            <span class="mode-label">Add New Account</span>
            <span class="mode-desc">Create a new account with a fresh ID</span>
          </label>
          <label class="mode-option" class:selected={mode === 'replace'}>
            <input type="radio" bind:group={mode} value="replace" disabled={phase === 'importing'} />
            <span class="mode-label">Replace Existing</span>
            <span class="mode-desc">Overwrite the player data of an existing account</span>
          </label>
        </div>
      </div>

      <!-- Target picker (replace mode only) -->
      {#if mode === 'replace'}
        <div class="form-group">
          <label class="field-label" for="imp-target">Target Account</label>
          {#if loadingAccounts}
            <div class="info-value dim" style="font-size:12px;">Loading accounts...</div>
          {:else if accounts.length === 0}
            <div class="feedback-error" style="font-size:11px;">No accounts found in database.</div>
          {:else}
            <select id="imp-target" bind:value={targetId} disabled={phase === 'importing'}>
              <option value={null} disabled selected>Select account to replace…</option>
              {#each accounts as acct}
                <option value={acct.id}>{acct.player_name} — {acct.email}</option>
              {/each}
            </select>
          {/if}
        </div>
      {/if}

      <!-- Server running warning -->
      {#if $serverRunning}
        <div class="warning-notice">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:14px;height:14px;flex-shrink:0;">
            <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/>
            <line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/>
          </svg>
          Stop the server before importing.
        </div>
      {/if}

      {#if importError}
        <div class="feedback-error">{importError}</div>
      {/if}

      <button
        class="btn btn-accent btn-sm import-btn"
        on:click={runImport}
        disabled={!canImport || phase === 'importing'}
      >
        {phase === 'importing' ? 'Importing...' : mode === 'add' ? 'Add Account' : 'Replace Account'}
      </button>
    </div>
  </div>

<!-- ── Success ── -->
{:else if phase === 'success'}
  <div class="import-success">
    <div class="success-icon">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/>
        <polyline points="22 4 12 14.01 9 11.01"/>
      </svg>
    </div>
    <div class="success-title">Account Imported</div>
    <div class="success-sub">
      {mode === 'add' ? 'A new account has been added to the database.' : 'The account data has been replaced.'}
    </div>

    <div class="success-actions">
      <button class="btn btn-outline btn-sm" on:click={reset}>Import Another</button>
      <button class="btn btn-accent btn-sm" on:click={() => (showServerModal = true)}>
        Add to Server Profiles
      </button>
    </div>

    <div class="success-note">
      "Add to Server Profiles" opens the server profile dialog with your email pre-filled.
      {#if finalPassword}
        The password you set during import will also be pre-filled.
      {:else}
        Enter your password manually — the original cannot be recovered from the export file.
      {/if}
    </div>
  </div>
{/if}

<style>
  /* ── Idle / drop zone ── */
  .import-idle {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 40px 20px;
    gap: 12px;
  }

  .drop-zone {
    width: 100%;
    max-width: 480px;
    border: 1px dashed var(--border-lit);
    border-radius: var(--radius-md);
    padding: 40px 24px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    cursor: pointer;
    transition: background 0.15s, border-color 0.15s;
    text-align: center;
  }
  .drop-zone:hover,
  .drop-zone.dragging {
    background: var(--bg-2);
    border-color: var(--accent-dim);
  }

  .drop-icon {
    width: 36px;
    height: 36px;
    color: var(--text-3);
  }
  .drop-zone:hover .drop-icon,
  .drop-zone.dragging .drop-icon {
    color: var(--accent);
  }

  .drop-label {
    font-family: var(--font-head);
    font-size: 13px;
    font-weight: 600;
    letter-spacing: 0.05em;
    color: var(--text-1);
  }

  .drop-sub {
    font-size: 11px;
    color: var(--text-3);
  }
  .drop-sub code {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-2);
  }

  /* ── Two-column layout ── */
  .import-columns {
    flex: 1;
    display: flex;
    overflow: hidden;
    min-height: 0;
  }

  .import-left {
    width: 240px;
    flex-shrink: 0;
    border-right: 1px solid var(--border);
    overflow-y: auto;
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .import-right {
    flex: 1;
    overflow-y: auto;
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-width: 0;
  }

  /* ── Form ── */
  .form-group {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .field-hint {
    font-family: var(--font-body);
    font-size: 10px;
    font-weight: 400;
    color: var(--text-3);
    text-transform: none;
    letter-spacing: 0;
    margin-left: 6px;
  }

  input.field-error {
    border-color: rgba(192, 57, 43, 0.6);
  }

  .field-error-msg {
    font-size: 11px;
    color: var(--text-error);
  }

  select {
    width: 100%;
    background: var(--bg-2);
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    color: var(--text-1);
    font-size: 12px;
    font-family: var(--font-body);
    padding: 6px 10px;
    cursor: pointer;
  }
  select:disabled { opacity: 0.5; cursor: default; }
  select:focus { outline: none; border-color: var(--accent-dim); }

  /* ── Mode selector ── */
  .mode-row {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .mode-option {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 9px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: background 0.1s, border-color 0.1s;
    position: relative;
  }
  .mode-option:hover { background: var(--bg-2); }
  .mode-option.selected {
    background: var(--accent-glow);
    border-color: var(--accent-dim);
  }

  .mode-option input[type="radio"] {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }

  .mode-label {
    font-family: var(--font-head);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-1);
  }
  .mode-option.selected .mode-label { color: var(--accent-bright); }

  .mode-desc {
    font-size: 11px;
    color: var(--text-3);
  }

  .import-btn { align-self: flex-start; margin-top: 4px; }

  /* ── Shared info block styles (reused from OpsPanel) ── */
  .info-block {
    background: var(--bg-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .info-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .info-label {
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-3);
    width: 64px;
    flex-shrink: 0;
  }

  .info-value {
    font-size: 12px;
    color: var(--text-1);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .info-value.mono { font-family: var(--font-mono); }
  .info-value.dim  { color: var(--text-3); }

  .subsection-title {
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-3);
  }

  /* ── Warning / feedback ── */
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

  .feedback-error {
    font-size: 11px;
    color: var(--text-error);
    padding: 8px 10px;
    border: 1px solid rgba(192, 57, 43, 0.4);
    background: var(--red-dim);
    border-radius: var(--radius-sm);
  }

  /* ── Success ── */
  .import-success {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 40px 24px;
    gap: 12px;
    text-align: center;
  }

  .success-icon svg {
    width: 36px;
    height: 36px;
    color: var(--green-bright);
  }

  .success-title {
    font-family: var(--font-head);
    font-size: 16px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-0);
  }

  .success-sub {
    font-size: 12px;
    color: var(--text-2);
    max-width: 360px;
  }

  .success-actions {
    display: flex;
    gap: 8px;
    margin-top: 8px;
  }

  .success-note {
    font-size: 11px;
    color: var(--text-3);
    max-width: 360px;
    line-height: 1.5;
  }
</style>