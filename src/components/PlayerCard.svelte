<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import {
    type PlayerSession,
    USER_LEVEL_OPTIONS,
    userLevelLabel,
    isBanned,
    isWhitelisted,
    formatLastSeen,
  } from '../lib/playerMeta'

  export let player: PlayerSession
  export let compact = false

  // ── Types ──────────────────────────────────────────────────────────────────

  type ConfirmAction = 'kick' | 'ban' | null

  type CommandFeedback = {
    ok: boolean
    msg: string
  }

  // ── State ──────────────────────────────────────────────────────────────────

  let confirmAction: ConfirmAction = null
  let feedback: CommandFeedback | null = null
  let feedbackTimer: ReturnType<typeof setTimeout> | null = null
  let setLevelValue: number = player.user_level ?? 0
  let setLevelPending = false
  let commandRunning = false

  let prevSessionId = player.session_id
  $: if (player.session_id !== prevSessionId) {
    prevSessionId = player.session_id
    setLevelValue = player.user_level ?? 0
    setLevelPending = false
    confirmAction = null
    feedback = null
  }

  // ── Helpers ────────────────────────────────────────────────────────────────

  function showFeedback(ok: boolean, msg: string) {
    feedback = { ok, msg }
    if (feedbackTimer) clearTimeout(feedbackTimer)
    feedbackTimer = setTimeout(() => (feedback = null), 4000)
  }

  async function runCommand(cmd: string): Promise<boolean> {
    commandRunning = true
    try {
      await invoke('send_command', { cmd })
      return true
    } catch (e) {
      showFeedback(false, String(e))
      return false
    } finally {
      commandRunning = false
    }
  }

  // ── Commands ──────────────────────────────────────────────────────────────

  async function executeKick() {
    confirmAction = null
    const ok = await runCommand(`!client kick ${player.username}`)
    if (ok) showFeedback(true, `Kicked ${player.username}`)
  }

  async function executeBan() {
    confirmAction = null
    if (!player.email) { showFeedback(false, 'No email on record — cannot ban this player'); return }
    const ok = await runCommand(`!account ban ${player.email}`)
    if (ok) showFeedback(true, `Banned ${player.username} (${player.email})`)
  }

  async function executeUnban() {
    if (!player.email) { showFeedback(false, 'No email on record — cannot unban this player'); return }
    const ok = await runCommand(`!account unban ${player.email}`)
    if (ok) showFeedback(true, `Unbanned ${player.username}`)
  }

  async function executeSetLevel() {
    if (!player.email) { showFeedback(false, 'No email on record — cannot change user level'); return }
    setLevelPending = false
    const ok = await runCommand(`!account userlevel ${player.email} ${setLevelValue}`)
    if (ok) showFeedback(true, `Set ${player.username} to ${userLevelLabel(setLevelValue)}`)
  }

  async function executeClientInfo() {
    const ok = await runCommand(`!client info ${player.session_id}`)
    if (ok) showFeedback(true, `Info for ${player.username} sent to console`)
  }

  async function executeWhitelist() {
    if (!player.email) { showFeedback(false, 'No email on record'); return }
    const ok = await runCommand(`!account whitelist ${player.email}`)
    if (ok) showFeedback(true, `Whitelisted ${player.username}`)
  }

  async function executeUnwhitelist() {
    if (!player.email) { showFeedback(false, 'No email on record'); return }
    const ok = await runCommand(`!account unwhitelist ${player.email}`)
    if (ok) showFeedback(true, `Removed ${player.username} from whitelist`)
  }
</script>

<div class="player-card" class:compact>

  <!-- ── Header ── -->
  <div class="card-header">
    <!-- <div class="avatar" class:avatar-banned={isBanned(player.flags)}>
      <span class="avatar-initial">{player.username[0]?.toUpperCase() ?? '?'}</span>
    </div> -->
    <div class="header-info">
      <div class="header-name-row">
        <span class="player-name">{player.username}
          {#if player.email}
            <span class="header-email">({player.email})</span>
          {/if}
        </span>
        {#if player.user_level && player.user_level > 0}
          <span class="level-badge level-{player.user_level}">{userLevelLabel(player.user_level)}</span>
        {/if}
        {#if isBanned(player.flags)}
          <span class="banned-badge">BANNED</span>
        {/if}
      </div>
      
      <span class="header-session">{player.session_id}</span>
    </div>
  </div>

  <!-- ── Body ── -->
  <div class="card-body">

    <!-- Moderation -->
    <div class="card-section">
      <div class="section-label">Moderation</div>

      {#if confirmAction !== null}
        <div class="confirm-box">
          {#if confirmAction === 'kick'}
            <p>Kick<strong>{player.username}</strong>?</p>
          {:else if confirmAction === 'ban'}
            <p>Ban<strong>{player.username}</strong>?
              <span class="confirm-sub">This will prevent them from logging in.</span>
            </p>
          {/if}
          <div class="btn-group">
            <button class="btn btn-danger" on:click={confirmAction === 'kick' ? executeKick : executeBan}>
              {confirmAction === 'kick' ? 'Kick' : 'Ban'}
            </button>
            <button class="btn btn-ghost" on:click={() => (confirmAction = null)}>Cancel</button>
          </div>
        </div>
      {:else}
        <div class="action-row">
          <div class="btn-group">
            <button class="btn btn-warn" disabled={commandRunning} on:click={() => (confirmAction = 'kick')}>
              Kick
            </button>
            {#if !isBanned(player.flags)}
              <button class="btn btn-danger" disabled={commandRunning} on:click={() => (confirmAction = 'ban')}>
                Ban
              </button>
            {:else}
              <button class="btn btn-ok" disabled={commandRunning} on:click={executeUnban}>
                Unban
              </button>
            {/if}
            <div class="btn-group">
              {#if !isWhitelisted(player.flags)}
                <button class="btn btn-ghost" disabled={commandRunning} on:click={executeWhitelist}
                  title="!account whitelist [email]">Whitelist</button>
              {:else}
                <button class="btn btn-ghost" disabled={commandRunning} on:click={executeUnwhitelist}
                  title="!account unwhitelist [email]">Unwhitelist</button>
              {/if}
            </div>
            <button
            class="btn btn-ghost"
            disabled={commandRunning}
            on:click={executeClientInfo}
            title="Client Info — output goes to server console"
            >Client Info</button>
          </div>
        </div>
      {/if}
    </div>

    <!-- Admin -->
    <div class="card-section">
      <div class="section-label">Admin</div>

      <div class="level-row">
        <span class="field-label">User Level</span>
        <select
          class="level-select"
          bind:value={setLevelValue}
          on:change={() => (setLevelPending = true)}
          disabled={commandRunning}
        >
          {#each USER_LEVEL_OPTIONS as opt}
            <option value={opt.value}>{opt.label}</option>
          {/each}
        </select>
        {#if setLevelPending}
          <button class="btn btn-accent" disabled={commandRunning} on:click={executeSetLevel}>Apply</button>
          <button class="btn btn-ghost" on:click={() => { setLevelPending = false; setLevelValue = player.user_level ?? 0 }}>Cancel</button>
        {/if}
      </div>
    </div>

    <!-- Feedback -->
    <!-- {#if feedback}
      <div class="feedback" class:feedback-ok={feedback.ok} class:feedback-err={!feedback.ok}>
        {feedback.msg}
      </div>
    {/if} -->

    <!-- Info -->
    <div class="info-section">
      <dl class="info-grid">
        {#if player.guild_name}
          <dt>Guild</dt><dd>{player.guild_name}</dd>
        {/if}
        <dt>Last Logout</dt><dd>{formatLastSeen(player.last_logout_time)}</dd>
        <dt>Avatars</dt><dd>{player.avatar_count ?? '—'}</dd>
        <dt>G Balance</dt><dd>{player.gazillionite_balance?.toLocaleString() ?? '—'}</dd>
      </dl>
    </div>

  </div>
</div>

<style>
  /* ── Card shell ── */
  .player-card {
    width: 100%;
    max-width: 820px;
    margin: 0 auto;
    background: var(--bg-0);
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-md);
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .player-card.compact {
    max-width: none;
    margin: 0;
    border: none;
    border-radius: 0;
  }

  /* ── Header ── */
  .card-header {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 14px 16px;
    background: var(--bg-2);
    border-bottom: 1px solid var(--border-mid);
  }

  .avatar {
    width: 44px;
    height: 44px;
    border-radius: 50%;
    background: var(--accent-glow);
    border: 1px solid var(--accent-dim);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .avatar.avatar-banned {
    background: rgba(200, 50, 50, 0.1);
    border-color: rgba(200, 50, 50, 0.4);
  }

  .avatar-initial {
    font-family: var(--font-head);
    font-size: 18px;
    font-weight: 700;
    color: var(--accent-bright);
    line-height: 1;
  }

  .header-info {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }

  .header-name-row {
    display: flex;
    align-items: center;
    gap: 7px;
    flex-wrap: wrap;
  }

  .player-name {
    font-family: var(--font-head);
    font-size: 15px;
    font-weight: 600;
    color: var(--text-0);
    letter-spacing: 0.01em;
  }

  .header-email {
    font-family: var(--font-body);
    font-size: 11px;
    color: var(--text-1);
  }

  .header-session {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-3);
  }

  /* ── Badges ── */
  .level-badge,
  .banned-badge {
    font-family: var(--font-head);
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    padding: 2px 5px;
    border-radius: 2px;
    border: 1px solid transparent;
    flex-shrink: 0;
  }

  .level-1 { color: var(--accent); border-color: var(--accent-dim); background: var(--accent-glow); }
  .level-2 { color: var(--accent-bright); border-color: var(--accent-dim); background: var(--accent-glow); }
  .banned-badge { color: var(--text-error); border-color: rgba(200, 50, 50, 0.4); background: rgba(200, 50, 50, 0.1); }

  /* ── Body ── */
  .card-body {
    display: flex;
    flex-direction: column;
  }

  /* ── Sections ── */
  .card-section {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px 16px;
    border-bottom: 1px solid var(--border);
  }

  .section-label {
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--text-3);
  }

  /* ── Action rows ── */
  .action-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .btn-group {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }

  /* ── User level row ── */
  .level-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .field-label {
    font-family: var(--font-head);
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-2);
    white-space: nowrap;
    flex-shrink: 0;
  }

  /* ── Feedback ── */
  .feedback {
    font-family: var(--font-body);
    font-size: 12px;
    padding: 8px 16px;
    border-top: 1px solid transparent;
    border-bottom: 1px solid transparent;
  }

  .feedback-ok {
    color: var(--green-bright);
    background: var(--green-dim);
    border-color: rgba(39, 174, 96, 0.2);
  }

  .feedback-err {
    color: var(--text-error);
    background: rgba(200, 50, 50, 0.08);
    border-color: rgba(200, 50, 50, 0.25);
  }

  /* ── Info section ── */
  .info-section {
    padding: 12px 16px;
    background: var(--bg-2);
  }

  .info-grid {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 7px 14px;
    margin: 0;
  }

  .info-grid dt {
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-3);
    white-space: nowrap;
    align-self: center;
  }

  .info-grid dd {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-1);
    margin: 0;
    word-break: break-all;
    align-self: center;
  }

  /* ── Buttons ── */
  .btn {
    font-family: var(--font-head);
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    padding: 5px 12px;
    border-radius: var(--radius-sm);
    border: 1px solid transparent;
    cursor: pointer;
    transition: opacity 0.1s, background 0.1s;
    white-space: nowrap;
    line-height: 1.4;
  }

  .btn:disabled { opacity: 0.4; cursor: not-allowed; }

  .btn-warn   { color: var(--amber-bright); background: var(--amber-dim); border-color: rgba(200, 146, 10, 0.35); }
  .btn-danger { color: var(--text-error); background: rgba(200, 50, 50, 0.1); border-color: rgba(200, 50, 50, 0.4); }
  .btn-ok     { color: var(--green-bright); background: var(--green-dim); border-color: rgba(39, 174, 96, 0.3); }
  .btn-accent { color: var(--accent-bright); background: var(--accent-glow); border-color: var(--accent-dim); }
  .btn-ghost  { color: var(--text-2); background: transparent; border-color: var(--border-mid); }

  .btn-warn:not(:disabled):hover   { opacity: 0.8; }
  .btn-danger:not(:disabled):hover { background: rgba(200, 50, 50, 0.18); }
  .btn-ok:not(:disabled):hover     { opacity: 0.8; }
  .btn-accent:not(:disabled):hover { background: var(--accent-glow-strong); }
  .btn-ghost:not(:disabled):hover  { background: var(--bg-2); color: var(--text-1); }

  /* ── Level select ── */
  .level-select {
    background: var(--bg-1);
    border: 1px solid var(--border-mid);
    color: var(--text-1);
    font-family: var(--font-body);
    font-size: 12px;
    padding: 4px 8px;
    border-radius: var(--radius-sm);
    outline: none;
    cursor: pointer;
    transition: border-color 0.12s;
  }
  .level-select:focus { border-color: var(--accent-dim); }
  .level-select option { background: var(--bg-2); }

  /* ── Confirm box ── */
  .confirm-box {
    background: var(--bg-1);
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .confirm-box p {
    margin: 0;
    font-size: 12px;
    color: var(--text-1);
    line-height: 1.5;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .confirm-box strong { color: var(--text-0); font-weight: 600; }
  .confirm-sub { font-size: 11px; color: var(--text-3); }
</style>