<script lang="ts">
  import { tick, onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { openUrl } from '@tauri-apps/plugin-opener'
  import { fetch as tauriFetch } from '@tauri-apps/plugin-http'
  import {
    serverRunning,
    appConfig,
    serverLog,
    serverLogFilter,
    logFilterThreshold,
    LOG_LEVEL_SEVERITY,
    appendLog,
    clearLog,
    apacheRunning,
    uptimeSec,
    serverError,
    clearServerError,
    setServerError,
  } from '../lib/store'
  import { FALLBACK_COMMANDS } from '../lib/serverCommands'
  import PlayersBlade from './PlayersBlade.svelte'

  const DASHBOARD_PORT_DEFAULT = 8080

  let playersOpen = false
  let playerCount = 0

  let command = ''
  let starting = false
  let stopping = false
  let startingApache = false
  let stoppingApache = false
  let scrollLocked = false
  let logEl: HTMLDivElement
  let dashboardPort = DASHBOARD_PORT_DEFAULT
  let dashboardPath = '/Dashboard/'

  let countdownSec = 0
  let countdownInterval: ReturnType<typeof setInterval> | null = null
  $: countdownActive = countdownInterval !== null

  $: if ($serverRunning) starting = false
  $: if (!$serverRunning) stopping = false
  $: if (!$serverRunning && countdownActive) clearCountdown()

  $: countdownLabel = (() => {
    if (!countdownActive) return ''
    const m = Math.floor(countdownSec / 60)
    const s = countdownSec % 60
    if (m > 0) return `${m}m ${String(s).padStart(2, '0')}s`
    return `${s}s`
  })()

  function clearCountdown() {
    if (countdownInterval) clearInterval(countdownInterval)
    countdownInterval = null
    countdownSec = 0
  }

  async function initiateStop() {
    const delay = $appConfig.shutdown.delay_minutes
    if (!delay || delay <= 0) {
      stopping = true
      clearServerError()
      try {
        await invoke('stop_server')
      } catch (e) {
        setServerError(String(e))
        stopping = false
      }
      return
    }

    countdownSec = delay * 60
    const initialMsg = $appConfig.shutdown.broadcast_message.replace('{minutes}', String(delay))
    try { await invoke('send_command', { cmd: `!server broadcast ${initialMsg}` }) } catch {}

    countdownInterval = setInterval(async () => {
      countdownSec -= 1

      if (countdownSec === 60) {
        try { await invoke('send_command', { cmd: '!server broadcast Server is shutting down in 1 minute.' }) } catch {}
      }

      if (countdownSec <= 0) {
        clearCountdown()
        stopping = true
        clearServerError()
        try {
          await invoke('stop_server')
        } catch (e) {
          setServerError(String(e))
          stopping = false
        }
      }
    }, 1000)
  }

  async function cancelStop() {
    clearCountdown()
    try { await invoke('send_command', { cmd: '!server broadcast Server shutdown has been cancelled.' }) } catch {}
  }

  $: filtered = (() => {
    if ($serverLogFilter === 'all') return $serverLog
    if ($logFilterThreshold) {
      const minSev = LOG_LEVEL_SEVERITY[$serverLogFilter]
      return $serverLog.filter(l => LOG_LEVEL_SEVERITY[l.level] >= minSev)
    }
    return $serverLog.filter(l => l.level === $serverLogFilter)
  })()
  $: filtered, scrollToEnd()
  $: if (!scrollLocked) filtered.length, scrollToEnd()

  $: uptimeFormatted = [
    Math.floor($uptimeSec / 3600),
    Math.floor(($uptimeSec % 3600) / 60),
    $uptimeSec % 60,
  ].map(n => String(n).padStart(2, '0')).join(':')

  async function scrollToEnd() {
    await tick()
    if (logEl) logEl.scrollTop = logEl.scrollHeight
  }

  onMount(async () => {
    if ($appConfig.server_exe) {
      try {
        const state = await invoke<{ values: Record<string, Record<string, string>> }>('read_config', { serverExe: $appConfig.server_exe })
        const raw = state.values['WebFrontend']?.['Port']
        const parsed = raw ? parseInt(raw, 10) : NaN
        if (!isNaN(parsed) && parsed > 0) dashboardPort = parsed
        const rawPath = state.values['WebFrontend']?.['DashboardUrlPath']
        if (rawPath?.trim()) dashboardPath = rawPath.trim()
      } catch {}
    }
  })

  async function startServer() {
    starting = true
    clearServerError()
    try {
      await invoke('start_server', { serverExe: $appConfig.server_exe })
    } catch (e) {
      setServerError(String(e))
      starting = false
    }
  }

  async function startApache() {
    startingApache = true
    clearServerError()
    try {
      await invoke('start_apache', { serverExe: $appConfig.server_exe })
      const apache = await invoke<boolean>('apache_is_running')
      apacheRunning.set(apache)
    } catch (e) {
      setServerError(String(e))
    } finally {
      startingApache = false
    }
  }

  async function stopApache() {
    stoppingApache = true
    clearServerError()
    try {
      await invoke('stop_apache')
      apacheRunning.set(false)
    } catch (e) {
      setServerError(String(e))
    } finally {
      stoppingApache = false
    }
  }

  async function sendCommand() {
    const cmd = command.trim()
    if (!cmd) return
    try {
      await invoke('send_command', { cmd })
      appendLog({ time: '', level: 'info', msg: `> ${cmd}` })
      command = ''
      acVisible = false; acResolved = null; acSuggs = []; acSel = -1
    } catch (e) {
      setServerError(String(e))
    }
  }

  // -- Command autocomplete --

  interface Cmd { f: string; a: string; d: string; invokerType?: string }

  let commands: Cmd[] = FALLBACK_COMMANDS.map(c => ({ ...c, invokerType: 'Any' }))

  /* Will be made available when /Commands (or similar) endpoint is made available */
  // async function loadServerCommands(retryMs = 0) {
  //   if (retryMs > 0) await new Promise(r => setTimeout(r, retryMs))
  //   try {
  //     const res = await tauriFetch(`http://localhost:${dashboardPort}/Commands`)
  //     if (!res.ok) return
  //     const data: { command: string; description: string; userLevel: string; invokerType: string }[] = await res.json()
  //     commands = data.map(entry => {
  //       const full = entry.command
  //       const argsMatch = full.match(/(\[.+)$/)
  //       const args = argsMatch ? argsMatch[1] : ''
  //       const base = args ? full.slice(0, full.length - args.length).trimEnd() : full
  //       return { f: base.toLowerCase(), a: args, d: entry.description, invokerType: entry.invokerType }
  //     })
  //     console.log(`[MH Multiverse] Loaded ${commands.length} commands from server`)
  //   } catch (e) {
  //     console.warn('[MH Multiverse] Failed to load commands from server, using fallback', e)
  //   }
  // }

  // let commandsLoadedForRunning = false

  // $: if ($serverRunning && !commandsLoadedForRunning) {
  //   commandsLoadedForRunning = true
  //   loadServerCommands()
  // }

  // $: if (!$serverRunning) {
  //   commandsLoadedForRunning = false
  // }

  let acSel = -1
  let acSuggs: Cmd[] = []
  let acResolved: Cmd | null = null
  let acVisible = false
  let cmdFocused = false
  let acPanelEl: HTMLElement | null = null

  $: if (acSel >= 0 && acPanelEl) {
    const selected = acPanelEl.querySelector('.ac-sugg.sel') as HTMLElement | null
    selected?.scrollIntoView({ block: 'nearest' })
  }

  function acLcp(strs: string[]): string {
    if (!strs.length) return ''
    let p = strs[0]
    for (let i = 1; i < strs.length; i++) {
      while (!strs[i].startsWith(p)) p = p.slice(0, -1)
      if (!p) return ''
    }
    return p
  }

  function acGetSuggs(v: string): Cmd[] {
    if (!v || v === '!') return []
    const lo = v.toLowerCase()
    return commands.filter(c => c.f.startsWith(lo))
  }

  function acFindResolved(v: string): Cmd | null {
    const lo = v.toLowerCase()
    return commands.find(c => lo === c.f || lo.startsWith(c.f + ' ')) ?? null
  }

  function acUpdate(v: string) {
    if (!v || v === '!') {
      acVisible = false; acSuggs = []; acResolved = null; acSel = -1
      return
    }
    const r = acFindResolved(v)
    if (r) {
      acResolved = r; acSuggs = []; acSel = -1; acVisible = true
    } else {
      acResolved = null; acSel = -1; acSuggs = acGetSuggs(v); acVisible = acSuggs.length > 0
    }
  }

  function acCompleteToCmd(c: Cmd) {
    command = c.f + (c.a ? ' ' : '')
    acSel = -1
    acUpdate(command)
  }

  function acDoTab() {
    if (acResolved) return
    if (!acSuggs.length) return
    const prefix = acLcp(acSuggs.map(c => c.f))
    if (prefix.length > command.length) {
      if (acSuggs.length === 1) {
        acCompleteToCmd(acSuggs[0])
      } else {
        command = prefix
        acSel = -1
        acUpdate(prefix)
      }
      return
    }
    if (acSel === -1) {
      acSel = 0
    } else {
      acCompleteToCmd(acSuggs[acSel])
    }
  }

  function onCmdKeydown(e: KeyboardEvent) {
    if (e.key === 'Tab') {
      e.preventDefault()
      acDoTab()
      return
    }
    if (e.key === 'Escape') {
      acVisible = false; acSel = -1; acResolved = null
      return
    }
    if (e.key === 'Enter') {
      if (acSel >= 0 && !acResolved) {
        e.preventDefault()
        acCompleteToCmd(acSuggs[acSel])
        return
      }
      sendCommand()
      return
    }
    if (acResolved || !acVisible) return
    if (e.key === 'ArrowDown') { e.preventDefault(); acSel = Math.min(acSel + 1, acSuggs.length - 1) }
    else if (e.key === 'ArrowUp') { e.preventDefault(); acSel = Math.max(acSel - 1, -1) }
  }

  async function openDashboard() {
    await openUrl(`http://localhost:${dashboardPort}${dashboardPath}`)
  }

  // const presets = [
  //   '!commands',
  //   '!account userlevel',
  //   '!server status',
  //   '!server broadcast',
  //   '!server shutdown',
  //   '!lookup item',
  //   '!lookup costume',
  // ]

  function applyPreset(cmd: string) {
    command = cmd
    acUpdate(cmd)
  }

  // -- Email suggestions --

  function isLocalhost(host: string): boolean {
    return host === 'localhost' || host === '127.0.0.1'
      || host.startsWith('localhost:') || host.startsWith('127.0.0.1:')
  }

  $: emailSuggs = (() => {
    if (!acResolved || !acResolved.a.startsWith('[email]')) return []
    const afterBase = command.slice(acResolved.f.length + 1)
    if (afterBase.includes(' ')) return []
    const typed = afterBase.toLowerCase()
    return $appConfig.servers
      .filter(s => isLocalhost(s.host) && s.email)
      .map(s => s.email)
      .filter(e => e.toLowerCase().startsWith(typed))
  })()

  function applyEmailSugg(email: string) {
    command = acResolved!.f + ' ' + email + ' '
    acUpdate(command)
  }
</script>

<div class="server-panel">

  <!-- Header bar with grid background -->
  <div class="server-header">
    <div class="panel-bg"></div>
    <div class="header-grid-overlay"></div>
    <div class="header-content">
      <div class="header-row-1">
        <div class="section-title">Local Server</div>
        {#if $serverRunning}
          <span class="uptime-display">{uptimeFormatted}</span>
        {/if}
        <div class="header-spacer"></div>
        {#if $serverError}
          <div class="header-error" title={$serverError}>{$serverError}</div>
        {/if}
        {#if $serverRunning}
          {#if countdownActive}
            <button class="btn btn-sm btn-countdown" on:click={cancelStop}>
              <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" style="width:10px;height:10px;"><rect x="3" y="3" width="8" height="8" rx="1"/></svg>
              {countdownLabel}
            </button>
          {:else}
            <button class="btn btn-sm btn-red" on:click={initiateStop} disabled={stopping}>
              <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" style="width:10px;height:10px;"><rect x="3" y="3" width="8" height="8" rx="1"/></svg>
              {stopping ? 'Stopping...' : 'Server'}
            </button>
          {/if}
        {:else}
          <button class="btn btn-sm btn-green" on:click={startServer} disabled={starting || !$appConfig.server_exe} title={!$appConfig.server_exe ? 'Set server exe in App settings' : ''}>
            <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" style="width:10px;height:10px;"><polygon points="4,2 12,7 4,12"/></svg>
            {starting ? 'Starting...' : 'Server'}
          </button>
        {/if}

          {#if $apacheRunning}
            <button class="btn btn-sm btn-red" on:click={stopApache} disabled={stoppingApache}>
              <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" style="width:10px;height:10px;"><rect x="3" y="3" width="8" height="8" rx="1"/></svg>
              Apache
            </button>
          {:else}
            <button class="btn btn-sm btn-outline" on:click={startApache} disabled={startingApache || !$appConfig.server_exe}>
              <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" style="width:10px;height:10px;"><polygon points="4,2 12,7 4,12"/></svg>
              Apache
            </button>
          {/if}

          <button class="btn btn-sm btn-outline" on:click={openDashboard} disabled={!$serverRunning}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:10px;height:10px;">
              <path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>
              <polyline points="15,3 21,3 21,9"/><line x1="10" y1="14" x2="21" y2="3"/>
            </svg>
            Dashboard
          </button>

          <div class="toolbar-sep"></div>

          <button class="btn btn-sm btn-outline" class:active={playersOpen} on:click={() => (playersOpen = !playersOpen)}>
            Players ({playerCount})
          </button>
      </div>
      <div class="header-row-2">
        <button class="btn btn-sm btn-outline" on:click={clearLog}>Clear</button>
        <button class="btn btn-sm btn-outline" class:active={scrollLocked} on:click={() => scrollLocked = !scrollLocked}>
          {scrollLocked ? 'Lock ON' : 'Lock'}
        </button>

        <div class="toolbar-sep"></div>

        <!-- Filters -->
        <span class="filter-label">Filter</span>
        <button class="filter-chip" class:active={$serverLogFilter === 'all'} on:click={() => serverLogFilter.set('all')}>All</button>
        <button class="filter-chip chip-purple" class:active={$serverLogFilter === 'trace'} on:click={() => serverLogFilter.set('trace')}>Trace</button>
        <button class="filter-chip chip-purple" class:active={$serverLogFilter === 'debug'} on:click={() => serverLogFilter.set('debug')}>Debug</button>
        <button class="filter-chip chip-blue" class:active={$serverLogFilter === 'info'} on:click={() => serverLogFilter.set('info')}>Info</button>
        <button class="filter-chip chip-amber" class:active={$serverLogFilter === 'warn'} on:click={() => serverLogFilter.set('warn')}>Warn</button>
        <button class="filter-chip chip-red" class:active={$serverLogFilter === 'err'} on:click={() => serverLogFilter.set('err')}>Error</button>
        <button class="filter-chip chip-red" class:active={$serverLogFilter === 'fatal'} on:click={() => serverLogFilter.set('fatal')}>Fatal</button>
      </div>
    </div>
  </div>

  <div class="console-area">
    <div class="log-view" bind:this={logEl}>
      {#each filtered as line (line.id)}
        <div class="log-line">
          {#if line.time}
            <span class="log-ts">{line.time}</span>
            <span class="log-lvl {line.level}">{line.level.toUpperCase()}</span>
            <span class="log-msg {line.level}">{line.msg}</span>
          {:else}
            <span class="log-msg log-raw">{line.msg}</span>
          {/if}
        </div>
      {/each}
      {#if $serverLog.length === 0}
        <div class="log-empty">No log output yet -- start the server to see output here</div>
      {/if}
    </div>

    <PlayersBlade bind:open={playersOpen} bind:playerCount />
  </div>

    <!-- Command bar -->
    <div class="cmd-bar">

      {#if acVisible && cmdFocused}
        <div class="ac-panel" bind:this={acPanelEl}>
          <div class="ac-header">
            <span class="ac-hint">TAB to complete  |  Up/Down navigate  |  ESC dismiss</span>
            {#if !acResolved}
              <span class="ac-count">{acSuggs.length} match{acSuggs.length !== 1 ? 'es' : ''}</span>
            {/if}
          </div>
          {#if acResolved}
            <div class="ac-resolved">
              <div class="ac-resolved-cmd">
                <span class="ac-resolved-base">{acResolved.f}</span>
                {#if acResolved.a}<span class="ac-resolved-args"> {acResolved.a}</span>{/if}
              </div>
              <div class="ac-resolved-desc">{acResolved.d}</div>
              {#if emailSuggs.length}
                <div class="ac-email-row">
                  {#each emailSuggs as email}
                    <button
                      class="ac-email-chip"
                      on:mousedown|preventDefault={() => applyEmailSugg(email)}
                    >{email}</button>
                  {/each}
                </div>
              {/if}
            </div>
          {:else}
            {#each acSuggs as c, i (c.f)}
              <div
                class="ac-sugg"
                class:sel={i === acSel}
                on:mousedown|preventDefault={() => acCompleteToCmd(c)}
                role="option"
                tabindex="-1"
                aria-selected={i === acSel}
              >
                <div class="ac-sugg-top">
                  <span class="ac-sugg-cmd">
                    <span class="ac-typed">{c.f.slice(0, command.length)}</span>{c.f.slice(command.length)}
                  </span>
                  {#if c.a}<span class="ac-sugg-args"> {c.a}</span>{/if}
                </div>
                <div class="ac-sugg-desc">{c.d}</div>
              </div>
            {/each}
          {/if}
        </div>
      {/if}

      <div class="cmd-input-row">
        <span class="cmd-prefix">&gt;</span>
        <input
          type="text"
          class="cmd-input"
          bind:value={command}
          placeholder={$serverRunning ? 'Type a command...' : 'Start the server to send commands'}
          disabled={!$serverRunning}
          on:input={() => acUpdate(command)}
          on:keydown={onCmdKeydown}
          on:focus={() => { cmdFocused = true; if (command) acUpdate(command) }}
          on:blur={() => setTimeout(() => { cmdFocused = false; acVisible = false }, 150)}
        >
        <button
          class="btn btn-sm btn-accent"
          on:click={sendCommand}
          disabled={!$serverRunning}
        >Send</button>
      </div>

      <div class="cmd-chips">
        {#each $appConfig.console_presets as p}
          <button class="cmd-chip" on:click={() => applyPreset(p)} disabled={!$serverRunning}>{p}</button>
        {/each}
      </div>
    </div>
</div>

<style>
  .server-panel {
    display: flex;
    flex-direction: column;
    flex: 1;
    overflow: hidden;
    min-height: 0;
  }

  /* -- Header with grid bg -- */
  .server-header {
    position: relative;
    flex-shrink: 0;
    border-bottom: 1px solid var(--border);
  }

  .header-grid-overlay {
    position: absolute;
    inset: 0;
    pointer-events: none;
    opacity: 0.02;
    background-image:
      linear-gradient(var(--text-0) 1px, transparent 1px),
      linear-gradient(90deg, var(--text-0) 1px, transparent 1px);
    background-size: 40px 40px;
  }

  .header-content {
    position: relative;
    z-index: 1;
    padding: 1px 16px 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .header-row-1 {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 52px;
  }

  .header-row-2 {
    display: flex;
    align-items: center;
    gap: 6px;
    padding-bottom: 8px;
  }

  .header-spacer { flex: 1; }

  .uptime-display {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-2);
  }

  .header-error {
    font-size: 11px;
    color: var(--text-error);
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .ctrl-group {
    display: flex;
    gap: 4px;
  }

  .toolbar-sep {
    width: 1px;
    height: 16px;
    background: var(--border-mid);
    margin: 0 4px;
    flex-shrink: 0;
  }

  .filter-label {
    font-family: var(--font-head);
    font-size: 9px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--text-3);
    margin-right: 2px;
  }

  /* filter-chip — self-contained so panel works before app.css update */
  :global(.filter-chip) {
    font-family: var(--font-head);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    padding: 3px 9px;
    border-radius: 2px;
    border: 1px solid transparent;
    cursor: pointer;
    background: transparent;
    color: var(--text-3);
    transition: all 0.12s;
    white-space: nowrap;
  }
  :global(.filter-chip:hover:not(:disabled)) { color: var(--text-1); border-color: var(--border-lit); }
  :global(.filter-chip.active)               { color: var(--text-0); border-color: var(--border-lit); background: var(--bg-3); }
  :global(.filter-chip.chip-purple.active)   { color: var(--purple);       border-color: rgba(130,100,180,0.4); background: var(--purple-dim); }
  :global(.filter-chip.chip-blue.active)     { color: var(--blue);         border-color: rgba(46,134,193,0.4);  background: var(--blue-dim); }
  :global(.filter-chip.chip-amber.active)    { color: var(--amber-bright); border-color: rgba(200,146,10,0.4);  background: var(--amber-dim); }
  :global(.filter-chip.chip-red.active)      { color: var(--text-error);   border-color: rgba(192,57,43,0.4);   background: var(--red-dim); }

  .btn.active {
    border-color: var(--accent-dim);
    color: var(--accent-bright);
    background: var(--accent-glow);
  }

  .btn-countdown {
    border-color: rgba(200, 146, 10, 0.4);
    color: var(--amber-bright);
    background: rgba(200, 146, 10, 0.1);
    min-width: 100px;
  }
  .btn-countdown:hover {
    border-color: rgba(200, 146, 10, 0.7);
    background: rgba(200, 146, 10, 0.18);
  }

  /* -- Console area (log + blade overlay container) -- */
  .console-area {
    flex: 1;
    min-height: 0;
    position: relative;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  /* -- Log view -- */
  .log-view {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 12px 16px;
    font-family: var(--font-mono);
    font-size: 11.5px;
    line-height: 1.65;
    background: var(--console-bg);
    min-height: 0;
  }

  .log-line {
    display: flex;
    gap: 10px;
  }

  .log-ts {
    color: var(--console-text-dim);
    flex-shrink: 0;
    user-select: none;
  }

  .log-lvl {
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.06em;
    padding: 1px 5px;
    border-radius: 2px;
    flex-shrink: 0;
    align-self: baseline;
    min-width: 8ch !important;
    text-align: center;
  }
  .log-lvl.trace { color: #8a8aa0; background: rgba(120, 120, 140, 0.1); border: 1px solid rgba(120, 120, 140, 0.2); }
  .log-lvl.debug { color: var(--purple); background: var(--purple-dim); border: 1px solid rgba(130, 100, 180, 0.2); }
  .log-lvl.info { color: var(--blue); background: var(--blue-dim); border: 1px solid rgba(46, 134, 193, 0.25); }
  .log-lvl.ok   { color: var(--green-bright); background: var(--green-dim); border: 1px solid rgba(39, 174, 96, 0.25); }
  .log-lvl.warn { color: var(--amber); background: rgba(212, 160, 23, 0.1); border: 1px solid rgba(212, 160, 23, 0.25); }
  .log-lvl.err  { color: var(--text-error); background: var(--red-dim); border: 1px solid rgba(192, 57, 43, 0.25); }
  .log-lvl.fatal { color: #ff4444; background: rgba(180, 30, 30, 0.15); border: 1px solid rgba(180, 30, 30, 0.35); font-weight: 700; }

  .log-msg        { color: var(--console-text); word-break: break-word; min-width: 0; }
  .log-msg.trace  { color: var(--console-text-dim); }
  .log-msg.debug  { color: var(--console-text-dim); }
  .log-msg.log-raw { color: var(--console-text-dim); white-space: pre; }
  .log-msg.info   { color: var(--text-1); }
  .log-msg.ok     { color: var(--green-bright); }
  .log-msg.warn   { color: var(--accent-dim); }
  .log-msg.err    { color: var(--text-error); }
  .log-msg.fatal  { color: #ff4444; font-weight: 600; }

  .log-empty {
    padding: 20px 0;
    font-family: var(--font-head);
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
  }

  /* -- Command bar -- */
  .cmd-bar {
    border-top: 1px solid var(--border);
    background: var(--console-cmd-bg);
    flex-shrink: 0;
    position: relative;
  }

  .cmd-input-row {
    display: flex;
    align-items: center;
  }

  .cmd-prefix {
    padding: 0 0 0 14px;
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--accent-dim);
    user-select: none;
  }

  .cmd-input {
    flex: 1;
    background: none;
    border: none;
    color: var(--text-0);
    font-family: var(--font-mono);
    font-size: 12px;
    padding: 10px 8px;
    outline: none;
    border-radius: 0;
  }
  .cmd-input::placeholder { color: var(--text-3); }
  .cmd-input:disabled { opacity: 0.5; }

  .cmd-input-row .btn {
    margin-right: 10px;
  }

  .cmd-chips {
    display: flex;
    gap: 4px;
    padding: 8px 14px;
    flex-wrap: wrap;
    background: var(--bg-0);
    border-top: 1px solid var(--border);
  }

  .cmd-chip {
    font-family: var(--font-mono);
    font-size: 10px;
    padding: 3px 8px;
    background: var(--bg-3);
    border: 1px solid var(--border-mid);
    color: var(--text-2);
    cursor: pointer;
    border-radius: 2px;
    transition: all 0.12s;
    white-space: nowrap;
  }
  .cmd-chip:hover:not(:disabled) {
    border-color: var(--accent-dim);
    color: var(--accent-dim);
    background: var(--accent-glow);
  }
  .cmd-chip:disabled { opacity: 0.4; cursor: not-allowed; }

  /* -- Autocomplete panel -- */
  .ac-panel {
    position: absolute;
    bottom: 100%;
    left: 0;
    right: 0;
    background: var(--bg-2);
    border: 1px solid var(--border-lit);
    border-bottom: none;
    border-radius: var(--radius-sm) var(--radius-sm) 0 0;
    max-height: 280px;
    overflow-y: auto;
    z-index: var(--z-dropdown);
  }

  .ac-panel::-webkit-scrollbar { width: 4px; }
  .ac-panel::-webkit-scrollbar-thumb { background: var(--border-lit); }

  .ac-header {
    padding: 5px 12px;
    background: var(--bg-0);
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-shrink: 0;
  }

  .ac-hint {
    font-family: var(--font-head);
    font-size: 9px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-3);
  }

  .ac-count {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-3);
  }

  .ac-sugg {
    padding: 7px 12px;
    cursor: pointer;
    border-bottom: 1px solid var(--border);
    border-left: 2px solid transparent;
    transition: background 0.08s;
  }
  .ac-sugg:last-child { border-bottom: none; }
  .ac-sugg:hover { background: var(--accent-glow); }
  .ac-sugg.sel { background: var(--accent-glow); border-left-color: var(--accent-dim); }

  .ac-sugg-top {
    display: flex;
    align-items: baseline;
    gap: 6px;
  }

  .ac-sugg-cmd {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-1);
  }
  .ac-typed { color: var(--accent-bright); }

  .ac-sugg-args {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-3);
  }

  .ac-sugg-desc {
    font-size: 11px;
    color: var(--text-3);
    margin-top: 3px;
    line-height: 1.4;
  }

  .ac-resolved { padding: 8px 12px; }

  .ac-resolved-cmd { font-family: var(--font-mono); font-size: 12px; }
  .ac-resolved-base { color: var(--green-bright); }
  .ac-resolved-args { color: var(--text-3); }
  .ac-resolved-desc { font-size: 11px; color: var(--text-3); margin-top: 3px; }

  .ac-email-row {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-top: 8px;
  }

  .ac-email-chip {
    font-family: var(--font-mono);
    font-size: 11px;
    padding: 2px 8px;
    border: 1px solid var(--border-mid);
    color: var(--text-2);
    cursor: pointer;
    background: var(--bg-0);
    border-radius: 2px;
    transition: all 0.1s;
  }
  .ac-email-chip:hover {
    border-color: var(--accent-dim);
    color: var(--accent-bright);
    background: var(--accent-glow);
  }
</style>