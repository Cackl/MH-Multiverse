<script lang="ts">
  import { onMount, onDestroy } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { openUrl } from '@tauri-apps/plugin-opener'
  import {
    appConfig, activeServerId,
    selectServer, deleteServer,
    gameRunning,
    type Server
  } from '../lib/store'
  import ServerModal from './ServerModal.svelte'

  $: activeServer = $appConfig.servers.find(s => s.id === $activeServerId) ?? null

  let showModal = false
  let editingServer: Server | null = null
  let launchError = ''
  let launching = false
  let pollTimer: ReturnType<typeof setInterval> | null = null

  // -- Inline delete confirmation --

  let pendingDeleteId: string | null = null
  let deleteTimer: ReturnType<typeof setTimeout> | null = null

  $: if ($activeServerId) cancelDelete()

  function requestDelete(server: Server) {
    cancelDelete()
    pendingDeleteId = server.id
    deleteTimer = setTimeout(cancelDelete, 3000)
  }

  function cancelDelete() {
    if (deleteTimer) { clearTimeout(deleteTimer); deleteTimer = null }
    pendingDeleteId = null
  }

  async function executeDelete(server: Server) {
    cancelDelete()
    await deleteServer(server.id)
  }

  onMount(async () => {
    await checkGame()
    pollTimer = setInterval(checkGame, 3000)

    // Read WebFrontend port for localhost dashboard URL
    if ($appConfig.server_exe) {
      try {
        const state = await invoke<{ values: Record<string, Record<string, string>> }>('read_config', { serverExe: $appConfig.server_exe })
        const raw = state.values['WebFrontend']?.['Port']
        const parsed = raw ? parseInt(raw, 10) : NaN
        if (!isNaN(parsed) && parsed > 0) dashboardPort = parsed
      } catch {}
    }
  })

  onDestroy(() => {
    if (pollTimer) clearInterval(pollTimer)
    cancelDelete()
  })

  async function checkGame() {
    try {
      const running = await invoke<boolean>('game_is_running')
      gameRunning.set(running)
    } catch {}
  }

  function openAdd() {
    editingServer = null
    showModal = true
  }

  function openEdit(server: Server) {
    editingServer = server
    showModal = true
  }

  function closeModal() {
    showModal = false
    editingServer = null
  }

  async function launch() {
    if (!activeServer) return
    launching = true
    launchError = ''
    try {
      await invoke('launch_game', { serverId: activeServer.id })
    } catch (e) {
      launchError = String(e)
    } finally {
      launching = false
    }
  }

  let dashboardPort = 8080

  function isLocalhost(host: string): boolean {
    const h = host.split(':')[0]
    return h === 'localhost' || h === '127.0.0.1'
  }

  async function openDashboard() {
    if (!activeServer) return
    let url: string
    if (isLocalhost(activeServer.host)) {
      url = `http://localhost:${dashboardPort}/Dashboard/`
    } else {
      const host = activeServer.host.includes('://') ? activeServer.host : `http://${activeServer.host}`
      url = `${host}/`
    }
    await openUrl(url)
  }
</script>

{#if showModal}
  <ServerModal server={editingServer} onClose={closeModal} />
{/if}

<div class="launch-panel">
  <div class="panel-bg"></div>
  <div class="grid-overlay"></div>

  <div class="launch-content">
    <!-- Server sidebar -->
    <aside class="server-sidebar">
      <div class="server-sidebar-head">
        <div class="section-title">Servers</div>
        <button class="btn-icon" title="Add server" on:click={openAdd}>
          <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5">
            <line x1="7" y1="3" x2="7" y2="11"/>
            <line x1="3" y1="7" x2="11" y2="7"/>
          </svg>
        </button>
      </div>

      <div class="server-list">
        {#each $appConfig.servers as server (server.id)}
          <div
            class="server-card"
            class:selected={server.id === $activeServerId}
            on:click={() => selectServer(server.id)}
            role="button"
            tabindex="0"
            on:keydown={(e) => e.key === 'Enter' && selectServer(server.id)}
          >
            <div class="server-card-name">{server.name}</div>
            <div class="server-card-url">{server.host}</div>
          </div>
        {/each}

        {#if $appConfig.servers.length === 0}
          <div class="empty-list">No servers yet</div>
        {/if}
      </div>
    </aside>

    <!-- Detail area -->
    {#if activeServer}
      <div class="launch-detail">
        <!-- Hero -->
        <div class="detail-hero">
          <div class="detail-icon">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <rect x="2" y="3" width="20" height="18" rx="2"/>
              <line x1="2" y1="9" x2="22" y2="9"/>
              <circle cx="7" cy="6" r="1" fill="currentColor" stroke="none"/>
              <circle cx="11" cy="6" r="1" fill="currentColor" stroke="none"/>
            </svg>
          </div>
          <div class="detail-hero-text">
            <h2>{activeServer.name}</h2>
            <p>{activeServer.host}</p>
          </div>
          <div class="detail-hero-actions">
            <button class="btn btn-sm btn-outline" on:click={() => openEdit(activeServer)}>Edit</button>
            {#if pendingDeleteId === activeServer.id}
              <button class="btn btn-sm btn-red confirm-delete" on:click={() => executeDelete(activeServer)}>Confirm?</button>
              <button class="btn btn-sm btn-outline" on:click={cancelDelete}>Cancel</button>
            {:else}
              <button class="btn btn-sm btn-red" on:click={() => requestDelete(activeServer)}>Remove</button>
            {/if}
          </div>
        </div>

        <!-- Credential fields -->
        <div class="detail-fields">
          <div class="field-group">
            <span class="field-label">Username</span>
            <div class="field-value">
              {#if activeServer.email}
                {activeServer.email}
              {:else}
                <span class="field-empty">Not set</span>
              {/if}
            </div>
          </div>
          <div class="field-group">
            <span class="field-label">Password</span>
            <div class="field-value">
              <span class="masked">************</span>
              <span class="badge badge-enc">AES-256</span>
            </div>
          </div>
        </div>
      </div>

      <!-- Launch bar -->
      <div class="launch-bar">
        {#if launchError}
          <div class="launch-error">{launchError}</div>
        {/if}
        <button class="btn btn-dashboard" on:click={openDashboard}>Dashboard</button>
        <button
          class="btn btn-launch"
          disabled={!$appConfig.game_exe || !activeServer || launching || $gameRunning}
          on:click={launch}
        >
          {$gameRunning ? 'Game Running' : launching ? 'Launching...' : 'Launch Game'}
        </button>
      </div>

    {:else}
      <div class="empty-state">
        <div class="empty-state-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" style="width:40px;height:40px;color:var(--text-3)">
            <rect x="2" y="3" width="20" height="18" rx="2"/>
            <line x1="2" y1="9" x2="22" y2="9"/>
          </svg>
        </div>
        <div class="empty-state-text">
          {$appConfig.servers.length === 0 ? 'Add a server to get started' : 'Select a server'}
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  .launch-panel {
    flex: 1;
    display: flex;
    flex-direction: column;
    position: relative;
    overflow: hidden;
  }

  .grid-overlay {
    position: absolute;
    inset: 0;
    pointer-events: none;
    opacity: 0.02;
    background-image:
      linear-gradient(var(--text-0) 1px, transparent 1px),
      linear-gradient(90deg, var(--text-0) 1px, transparent 1px);
    background-size: 40px 40px;
  }

  .launch-content {
    position: relative;
    z-index: 1;
    flex: 1;
    display: grid;
    grid-template-columns: 280px 1fr;
    grid-template-rows: 1fr auto;
    overflow: hidden;
  }

  /* -- Server sidebar -- */
  .server-sidebar {
    grid-row: 1 / -1;
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    background: var(--sidebar-bg);
  }

  .server-sidebar-head {
    display: flex;
    align-items: center;
    padding: 14px 16px 12px;
    border-bottom: 1px solid var(--border);
    min-height: 52px;
  }
  .server-sidebar-head .btn-icon {
    margin-left: auto;
  }

  .server-list {
    flex: 1;
    overflow-y: auto;
    padding: 6px;
  }

  .server-card {
    padding: 10px 12px;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: all 0.12s;
    margin-bottom: 2px;
  }
  .server-card:hover {
    background: var(--bg-3);
    border-color: var(--border-mid);
  }
  .server-card.selected {
    background: var(--accent-glow);
    border-color: var(--accent-dim);
  }

  .server-card-name {
    font-family: var(--font-head);
    font-size: 13px;
    font-weight: 600;
    color: var(--text-0);
    margin-bottom: 3px;
  }
  .server-card.selected .server-card-name {
    color: var(--accent-bright);
  }

  .server-card-url {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-2);
  }

  .empty-list {
    padding: 20px 12px;
    font-family: var(--font-head);
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
    text-align: center;
  }

  /* -- Detail area -- */
  .launch-detail {
    display: flex;
    flex-direction: column;
    padding: 20px 24px;
    gap: 20px;
    overflow-y: auto;
  }

  .detail-hero {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .detail-icon {
    width: 48px;
    height: 48px;
    background: var(--accent-glow);
    border: 1px solid var(--accent-dim);
    border-radius: var(--radius-md);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--accent);
    flex-shrink: 0;
  }
  .detail-icon svg { width: 24px; height: 24px; }

  .detail-hero-text {
    flex: 1;
    min-width: 0;
  }
  .detail-hero-text h2 {
    font-family: var(--font-head);
    font-size: 20px;
    font-weight: 700;
    color: var(--text-0);
    line-height: 1.1;
  }
  .detail-hero-text p {
    font-size: 12px;
    color: var(--text-2);
    margin-top: 2px;
  }

  .detail-hero-actions {
    display: flex;
    gap: 6px;
    flex-shrink: 0;
  }

  .detail-fields {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }

  .field-group {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .field-empty {
    color: var(--text-3);
    font-style: italic;
  }

  .masked {
    letter-spacing: 0.15em;
    color: var(--text-3);
  }

  /* -- Launch bar -- */
  .launch-bar {
    grid-column: 2;
    border-top: 1px solid var(--border);
    padding: 16px 24px;
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .btn-dashboard {
    padding: 10px 20px;
    border-color: var(--border-lit);
    color: var(--text-1);
    font-family: var(--font-head);
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    background: transparent;
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: all 0.14s;
  }
  .btn-dashboard:hover {
    border-color: var(--text-2);
    color: var(--text-0);
    background: var(--bg-3);
  }

  .btn-launch {
    margin-left: auto;
    padding: 10px 32px;
    font-family: var(--font-head);
    font-size: 14px;
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    background: linear-gradient(135deg, var(--accent-dim) 0%, var(--accent) 100%);
    border: 1px solid var(--accent);
    color: #fff;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.3);
    box-shadow: 0 0 20px rgba(62, 199, 199, 0.15), 0 2px 8px rgba(0, 0, 0, 0.3);
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: all 0.14s;
  }
  .btn-launch:hover:not(:disabled) {
    background: linear-gradient(135deg, var(--accent) 0%, var(--accent-bright) 100%);
    box-shadow: 0 0 30px rgba(62, 194, 199, 0.25), 0 2px 12px rgba(0, 0, 0, 0.4);
  }
  .btn-launch:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }

  .launch-error {
    font-size: 12px;
    color: #e74c3c;
    padding: 6px 10px;
    border: 1px solid rgba(192, 57, 43, 0.4);
    background: var(--red-dim);
    border-radius: var(--radius-sm);
  }

  .confirm-delete {
    animation: confirm-pulse 0.4s ease-out;
  }

  @keyframes confirm-pulse {
    0%   { opacity: 0.5; transform: scale(0.95); }
    100% { opacity: 1;   transform: scale(1); }
  }

  /* -- Empty state -- */
  .empty-state {
    grid-column: 2;
    grid-row: 1 / -1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
  }

  .empty-state-text {
    font-family: var(--font-head);
    font-size: 13px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-3);
  }
</style>