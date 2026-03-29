<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { openUrl } from '@tauri-apps/plugin-opener'
  import { fetch as tauriFetch } from '@tauri-apps/plugin-http'
  import { serverRunning, appConfig, serverLog, appendLog, appendLogBatch, clearLog, apacheRunning, uptimeSec, startUptime, stopUptime, type LogLine, type LogLevel } from '../lib/store'
  import ConfigPanel from './ConfigPanel.svelte'

  type Filter = 'all' | 'trace' | 'debug' | 'info' | 'warn' | 'err' | 'fatal'

  const DASHBOARD_PORT_DEFAULT = 8080

  let filter: Filter = 'all'
  let command = ''
  let starting = false
  let stopping = false
  let startingApache = false
  let stoppingApache = false
  let error = ''
  let scrollLocked = false
  let logEl: HTMLDivElement
  let dashboardPort = DASHBOARD_PORT_DEFAULT
  let showConfig = false

  let countdownSec = 0
  let countdownInterval: ReturnType<typeof setInterval> | null = null
  $: countdownActive = countdownInterval !== null

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
      error = ''
      try {
        await invoke('stop_server')
      } catch (e) {
        error = String(e)
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
        error = ''
        try {
          await invoke('stop_server')
        } catch (e) {
          error = String(e)
          stopping = false
        }
      }
    }, 1000)
  }

  async function cancelStop() {
    clearCountdown()
    try { await invoke('send_command', { cmd: '!server broadcast Server shutdown has been cancelled.' }) } catch {}
  }
  let unlistenLog: UnlistenFn | null = null
  let unlistenStarted: UnlistenFn | null = null
  let unlistenStopped: UnlistenFn | null = null

  $: filtered = filter === 'all' ? $serverLog : $serverLog.filter(l => l.level === filter)
  $: filter, scrollToEnd()

  $: uptimeFormatted = [
    Math.floor($uptimeSec / 3600),
    Math.floor(($uptimeSec % 3600) / 60),
    $uptimeSec % 60,
  ].map(n => String(n).padStart(2, '0')).join(':')

  async function scrollToEnd() {
    await tick()
    if (logEl) logEl.scrollTop = logEl.scrollHeight
  }

  function scrollToBottom() {
    if (!scrollLocked) scrollToEnd()
  }

  onMount(async () => {
    if ($appConfig.server_exe) {
      try {
        const state = await invoke<{ values: Record<string, Record<string, string>> }>('read_config', { serverExe: $appConfig.server_exe })
        const raw = state.values['WebFrontend']?.['Port']
        const parsed = raw ? parseInt(raw, 10) : NaN
        if (!isNaN(parsed) && parsed > 0) dashboardPort = parsed
      } catch {}
    }

    const running = await invoke<boolean>('server_is_running')
    serverRunning.set(running)
    if (running) {
      startUptime()
      const apache = await invoke<boolean>('apache_is_running')
      apacheRunning.set(apache)
      await loadServerCommands()
    }

    unlistenLog = await listen<Omit<LogLine, 'id'>[]>('server-log', (event) => {
      appendLogBatch(event.payload)
      scrollToBottom()
    })

    unlistenStarted = await listen('server-started', async () => {
      serverRunning.set(true)
      starting = false
      error = ''
      startUptime()
      appendLog({ time: '', level: 'ok', msg: '-- Server started --' })
      scrollToBottom()
      const apache = await invoke<boolean>('apache_is_running')
      apacheRunning.set(apache)
      await loadServerCommands(3000)
    })

    unlistenStopped = await listen<{ running: boolean; exit_code: number | null }>('server-stopped', (event) => {
      serverRunning.set(false)
      apacheRunning.set(false)
      stopping = false
      clearCountdown()
      stopUptime()
      if (event.payload.exit_code !== null && event.payload.exit_code !== 0) {
        error = `Server exited unexpectedly (code ${event.payload.exit_code})`
        appendLog({ time: '', level: 'err', msg: `-- Server exited unexpectedly (code ${event.payload.exit_code}) --` })
      } else {
        appendLog({ time: '', level: 'info', msg: '-- Server stopped --' })
      }
      scrollToBottom()
    })
  })

  onDestroy(() => {
    unlistenLog?.()
    unlistenStarted?.()
    unlistenStopped?.()
    clearCountdown()
  })

  async function startServer() {
    starting = true
    error = ''
    try {
      await invoke('start_server', { serverExe: $appConfig.server_exe })
    } catch (e) {
      error = String(e)
      starting = false
    }
  }

  async function stopServer() {
    stopping = true
    error = ''
    try {
      await invoke('stop_server')
    } catch (e) {
      error = String(e)
      stopping = false
    }
  }

  async function startApache() {
    startingApache = true
    try {
      await invoke('start_apache', { serverExe: $appConfig.server_exe })
      const apache = await invoke<boolean>('apache_is_running')
      apacheRunning.set(apache)
    } catch (e) {
      error = String(e)
    } finally {
      startingApache = false
    }
  }

  async function stopApache() {
    stoppingApache = true
    try {
      await invoke('stop_apache')
      apacheRunning.set(false)
    } catch (e) {
      error = String(e)
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
      scrollToBottom()
      command = ''
      acVisible = false; acResolved = null; acSuggs = []; acSel = -1
    } catch (e) {
      error = String(e)
    }
  }

  // -- Command autocomplete --

  interface Cmd { f: string; a: string; d: string; invokerType?: string }

  // Fallback hardcoded list — replaced at runtime by /Commands endpoint when server starts
  const FALLBACK_COMMANDS: Cmd[] = [
    {f:'!account ban',a:'[email]',d:'Bans the specified account.'},
    {f:'!account create',a:'[email] [playerName] [password]',d:'Creates a new account.'},
    {f:'!account download',a:'',d:'Downloads a JSON copy of the current account.'},
    {f:'!account info',a:'',d:'Shows information for the logged in account.'},
    {f:'!account password',a:'[email] [password]',d:'Changes password for the specified account.'},
    {f:'!account playername',a:'[email] [playername]',d:'Changes player name for the specified account.'},
    {f:'!account unban',a:'[email]',d:'Unbans the specified account.'},
    {f:'!account unwhitelist',a:'[email]',d:'Removes the specified account from the whitelist.'},
    {f:'!account userlevel',a:'[email] [0/1/2]',d:'Changes user level for the specified account.'},
    {f:'!account verify',a:'[email] [password]',d:'Checks if an email/password combination is valid.'},
    {f:'!account whitelist',a:'[email]',d:'Whitelists the specified account.'},
    {f:'!achievement info',a:'[id]',d:'Outputs info for the specified achievement.'},
    {f:'!achievement localeid',a:'',d:'Generates a LocaleStringId from the argument.'},
    {f:'!aoi print',a:'',d:'Prints player AOI information to the server console.'},
    {f:'!aoi refs',a:'',d:'Prints interest references for the current player.'},
    {f:'!aoi update',a:'',d:'Forces AOI proximity update.'},
    {f:'!aoi volume',a:'[value]',d:'Changes player AOI volume size.'},
    {f:'!boost damage',a:'[1-10000]',d:'Sets DamagePctBonus for the current avatar.'},
    {f:'!boost invulnerable',a:'',d:'Switches Invulnerable for the current avatar.'},
    {f:'!boost mana',a:'',d:'Switches NoEnduranceCosts for the current avatar.'},
    {f:'!boost vsboss',a:'[1-10000]',d:'Sets DamagePctBonusVsBosses for the current avatar.'},
    {f:'!client info',a:'[sessionId]',d:'Prints information about the specified client.'},
    {f:'!client kick',a:'[playerName]',d:'Disconnects the client with the specified player name.'},
    {f:'!debug ai',a:'',d:'No description available.'},
    {f:'!debug area',a:'',d:'Shows current area.'},
    {f:'!debug cell',a:'',d:'Shows current cell.'},
    {f:'!debug compactloh',a:'',d:'Requests GC to compact the large object heap.'},
    {f:'!debug crashgame',a:'',d:'Crashes the current game instance.'},
    {f:'!debug crashserver',a:'',d:'Crashes the entire server.'},
    {f:'!debug difficulty',a:'',d:'Shows current difficulty level information.'},
    {f:'!debug forcegc',a:'',d:'Requests the garbage collector to perform a collection.'},
    {f:'!debug getconditionlist',a:'',d:'Gets all conditions tracked by the ConditionPool.'},
    {f:'!debug geteventpoolreport',a:'',d:'Returns a report of the ScheduledEventPool state.'},
    {f:'!debug metagame',a:'[on/off]',d:'No description available.'},
    {f:'!debug region',a:'',d:'Shows current region.'},
    {f:'!debug seed',a:'',d:'Shows current seed.'},
    {f:'!debug test',a:'',d:'Runs test code.'},
    {f:'!entity create',a:'[pattern] [count]',d:'Create entity near the avatar based on pattern.'},
    {f:'!entity dummy',a:'[pattern]',d:'Replace the training room dummy with the specified entity.'},
    {f:'!entity info',a:'[EntityId]',d:'Displays information about the specified entity.'},
    {f:'!entity near',a:'[radius]',d:'Displays all entities within a radius (default 100).'},
    {f:'!entity tp',a:'[pattern]',d:'Teleports to the first entity matching the given pattern.'},
    {f:'!instance list',a:'',d:"Lists instances in the player's WorldView."},
    {f:'!instance reset',a:'',d:"Resets private instances in the player's WorldView."},
    {f:'!item cleardeliverybox',a:'',d:'Destroys all items in the delivery box inventory.'},
    {f:'!item creditchest',a:'',d:'Converts credits to a sellable chest item.'},
    {f:'!item destroyindestructible',a:'',d:'Destroys indestructible items from general inventory.'},
    {f:'!item drop',a:'[pattern] [count]',d:'Creates and drops the specified item from the current avatar.'},
    {f:'!item give',a:'[pattern] [count]',d:'Creates and gives the specified item to the current player.'},
    {f:'!item roll',a:'[pattern]',d:'Rolls the specified loot table.'},
    {f:'!item rollall',a:'',d:'Rolls all loot tables.'},
    {f:'!leaderboards all',a:'',d:'Shows all leaderboards.'},
    {f:'!leaderboards enabled',a:'',d:'Shows enabled leaderboards.'},
    {f:'!leaderboards instance',a:'[instanceId]',d:'Shows details for the specified leaderboard instance.'},
    {f:'!leaderboards now',a:'',d:'Shows all active instances.'},
    {f:'!leaderboards reloadschedule',a:'',d:'Reloads leaderboard schedule from JSON.'},
    {f:'!level awardxp',a:'[amount]',d:'Awards the specified amount of experience.'},
    {f:'!level max',a:'',d:"Maxes out the current avatar's experience."},
    {f:'!level maxinfinity',a:'',d:'Maxes out Infinity experience.'},
    {f:'!level maxomega',a:'',d:'Maxes out Omega experience.'},
    {f:'!level reset',a:'',d:'Resets the current avatar to level 1.'},
    {f:'!level resetinfinity',a:'',d:'Removes all Infinity progression.'},
    {f:'!level resetomega',a:'',d:'Removes all Omega progression.'},
    {f:'!level up',a:'',d:'Levels up the current avatar.'},
    {f:'!lookup asset',a:'[pattern]',d:'Searches assets.'},
    {f:'!lookup blueprint',a:'[pattern]',d:'Searches blueprints.'},
    {f:'!lookup costume',a:'[pattern]',d:'Searches prototypes using the costume blueprint.'},
    {f:'!lookup item',a:'[pattern]',d:'Searches prototypes using the item blueprint.'},
    {f:'!lookup power',a:'[pattern]',d:'Searches prototypes using the power blueprint.'},
    {f:'!lookup region',a:'[pattern]',d:'Searches prototypes using the region blueprint.'},
    {f:'!metagame event',a:'[next/stop]',d:'Changes current event. Defaults to stop.'},
    {f:'!mission complete',a:'[pattern]',d:'Complete the given mission.'},
    {f:'!mission completestory',a:'',d:'Set all main story missions to completed.'},
    {f:'!mission info',a:'[pattern]',d:'Display information about the given mission.'},
    {f:'!mission region',a:'',d:'List all mission prototypes in the current region.'},
    {f:'!mission reset',a:'[pattern]',d:'Restart the given mission.'},
    {f:'!mission resetstory',a:'',d:'Reset all main story missions.'},
    {f:'!player clearconditions',a:'',d:'Clears persistent conditions.'},
    {f:'!player costume',a:'[name/reset]',d:'Changes costume for the current avatar.'},
    {f:'!player die',a:'',d:'Kills the current avatar.'},
    {f:'!player givecurrency',a:'[amount]',d:'Gives all currencies.'},
    {f:'!player wipe',a:'[playerName]',d:'Wipes all progress for the current account.'},
    {f:'!power cooldownreset',a:'',d:'Resets all cooldowns and charges.'},
    {f:'!power print',a:'',d:'Prints the power collection for the current avatar.'},
    {f:'!power stealpowers',a:'',d:'Unlocks all stolen powers.'},
    {f:'!region generateallsafe',a:'',d:'Generates all safe regions.'},
    {f:'!region info',a:'',d:'Prints info for the current region.'},
    {f:'!region properties',a:'',d:'Prints properties for the current region.'},
    {f:'!region reload',a:'',d:'Reloads the current region.'},
    {f:'!region warp',a:'[name]',d:'Warps the player to another region.'},
    {f:'!server broadcast',a:'',d:'Broadcasts a notification to all players.'},
    {f:'!server reloadcatalog',a:'',d:'Reloads MTX store catalog.'},
    {f:'!server reloaddashboard',a:'',d:'Reloads the web dashboard.'},
    {f:'!server reloadlivetuning',a:'',d:'Reloads live tuning settings.'},
    {f:'!server shutdown',a:'',d:'Shuts the server down.'},
    {f:'!server status',a:'',d:'Prints server status.'},
    {f:'!store addg',a:'[amount]',d:'Adds the specified number of Gs to this account.'},
    {f:'!store convertes',a:'[amount]',d:'Converts Eternity Splinters to Gs.'},
    {f:'!ultimateprestige activate',a:'',d:'Activates Ultimate Prestige for the current hero.'},
    {f:'!ultimateprestige level',a:'',d:'Prints the current Ultimate Prestige level.'},
    {f:'!unlock chapters',a:'',d:'Unlocks all chapters.'},
    {f:'!unlock waypoints',a:'',d:'Unlocks all waypoints.'},
    {f:'!webapi generatekey',a:'',d:'Generates a new web API key.'},
    {f:'!webapi reloadkeys',a:'',d:'Reloads web API keys.'},
    {f:'!commands',a:'',d:'Lists available commands.'},
    {f:'!dance',a:'',d:'Performs the Dance emote (if available).'},
    {f:'!help',a:'',d:'Help needs no help.'},
    {f:'!jail',a:'',d:'Teleports to East Side: Detention Facility (old).'},
    {f:'!position',a:'',d:'Shows current position.'},
    {f:'!syncmana',a:'',d:'Syncs the current mana value with the server.'},
    {f:'!tower',a:'',d:'Teleports to Avengers Tower (original).'},
    {f:'!tp',a:'x:+offset | x y z',d:'Teleports to position.'},
    {f:'!gc collect',a:'',d:'Forces garbage collection.'},
  ]

  let commands: Cmd[] = FALLBACK_COMMANDS.map(c => ({ ...c, invokerType: 'Any' }))

  async function loadServerCommands(retryMs = 0) {
    if (retryMs > 0) await new Promise(r => setTimeout(r, retryMs))
    try {
      const res = await tauriFetch(`http://localhost:${dashboardPort}/Commands`)
      if (!res.ok) return
      const data: { command: string; description: string; userLevel: string; invokerType: string }[] = await res.json()
      commands = data.map(entry => {
        const full = entry.command
        const argsMatch = full.match(/(\[.+)$/)
        const args = argsMatch ? argsMatch[1] : ''
        const base = args ? full.slice(0, full.length - args.length).trimEnd() : full
        return { f: base.toLowerCase(), a: args, d: entry.description, invokerType: entry.invokerType }
      })
      console.log(`[Manifold] Loaded ${commands.length} commands from server`)
    } catch (e) {
      console.warn('[Manifold] Failed to load commands from server, using fallback', e)
    }
  }

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
    await openUrl(`http://localhost:${dashboardPort}/Dashboard/`)
  }

  const presets = [
    '!commands',
    '!account userlevel',
    '!server status',
    '!server broadcast',
    '!server shutdown',
    '!lookup item',
    '!lookup costume',
  ]

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
        {#if error}
          <div class="header-error" title={error}>{error}</div>
        {/if}
        <!-- <button class="btn btn-sm btn-outline" on:click={() => showConfig = !showConfig}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:12px;height:12px;">
            <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/>
            <circle cx="12" cy="12" r="3"/>
          </svg>
          {showConfig ? 'Console' : 'Configure'}
        </button> -->
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
      </div>
      <div class="header-row-2">
        <!-- Process controls -->
        <!-- <div class="ctrl-group">
          {#if $serverRunning}
            <button class="btn btn-sm btn-red" on:click={stopServer} disabled={stopping}>
              <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" style="width:10px;height:10px;"><rect x="3" y="3" width="8" height="8" rx="1"/></svg>
              {stopping ? 'Stopping...' : 'Server'}
            </button>
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
        </div>
        <div class="toolbar-sep"></div> -->

        <button class="btn btn-sm btn-outline" on:click={clearLog}>Clear</button>
        <button class="btn btn-sm btn-outline" class:active={scrollLocked} on:click={() => scrollLocked = !scrollLocked}>
          {scrollLocked ? 'Lock ON' : 'Lock'}
        </button>

        <div class="toolbar-sep"></div>

        <!-- Filters -->
        <span class="filter-label">Filter</span>
        <button class="log-filter" class:on={filter === 'all'} on:click={() => filter = 'all'}>All</button>
        <button class="log-filter trace" class:on={filter === 'trace'} on:click={() => filter = 'trace'}>Trace</button>
        <button class="log-filter debug" class:on={filter === 'debug'} on:click={() => filter = 'debug'}>Debug</button>
        <button class="log-filter info" class:on={filter === 'info'} on:click={() => filter = 'info'}>Info</button>
        <button class="log-filter warn" class:on={filter === 'warn'} on:click={() => filter = 'warn'}>Warn</button>
        <button class="log-filter err" class:on={filter === 'err'} on:click={() => filter = 'err'}>Error</button>
        <button class="log-filter fatal" class:on={filter === 'fatal'} on:click={() => filter = 'fatal'}>Fatal</button>
      </div>
    </div>
  </div>

  <!-- Content: Console or Config -->
  <!-- {#if showConfig}
    <ConfigPanel embedded={true} onBack={() => showConfig = false} />
  {:else} -->
    <!-- Log view -->
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
        {#each presets as p}
          <button class="cmd-chip" on:click={() => applyPreset(p)} disabled={!$serverRunning}>{p}</button>
        {/each}
      </div>
    </div>
  <!-- {/if} -->

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
    color: #e74c3c;
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

  .log-filter {
    font-family: var(--font-head);
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    padding: 3px 7px;
    border-radius: 2px;
    border: 1px solid transparent;
    cursor: pointer;
    background: transparent;
    color: var(--text-3);
    transition: all 0.12s;
  }
  .log-filter:hover { color: var(--text-1); }
  .log-filter.on { border-color: var(--border-lit); color: var(--text-0); background: var(--bg-3); }
  .log-filter.trace.on { border-color: rgba(120, 120, 140, 0.5); color: #8a8aa0; }
  .log-filter.debug.on { border-color: rgba(130, 100, 180, 0.5); color: #a080d0; }
  .log-filter.info.on  { border-color: rgba(46, 134, 193, 0.5); color: #5dade2; }
  .log-filter.warn.on  { border-color: rgba(180, 100, 10, 0.5); color: #d4a017; }
  .log-filter.err.on   { border-color: rgba(192, 57, 43, 0.5); color: #e74c3c; }
  .log-filter.fatal.on { border-color: rgba(180, 30, 30, 0.6); color: #ff4444; }

  .btn.active {
    border-color: var(--accent-dim);
    color: var(--accent-bright);
    background: var(--accent-glow);
  }

  .btn-countdown {
    border-color: rgba(200, 146, 10, 0.4);
    color: #e6b820;
    background: rgba(200, 146, 10, 0.1);
    min-width: 100px;
  }
  .btn-countdown:hover {
    border-color: rgba(200, 146, 10, 0.7);
    background: rgba(200, 146, 10, 0.18);
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
    min-width: 32px;
    text-align: center;
  }
  .log-lvl.trace { color: #8a8aa0; background: rgba(120, 120, 140, 0.1); border: 1px solid rgba(120, 120, 140, 0.2); }
  .log-lvl.debug { color: #a080d0; background: rgba(130, 100, 180, 0.1); border: 1px solid rgba(130, 100, 180, 0.2); }
  .log-lvl.info { color: #5dade2; background: var(--blue-dim); border: 1px solid rgba(46, 134, 193, 0.25); }
  .log-lvl.ok   { color: var(--green-bright); background: var(--green-dim); border: 1px solid rgba(39, 174, 96, 0.25); }
  .log-lvl.warn { color: #d4a017; background: rgba(212, 160, 23, 0.1); border: 1px solid rgba(212, 160, 23, 0.25); }
  .log-lvl.err  { color: #e74c3c; background: var(--red-dim); border: 1px solid rgba(192, 57, 43, 0.25); }
  .log-lvl.fatal { color: #ff4444; background: rgba(180, 30, 30, 0.15); border: 1px solid rgba(180, 30, 30, 0.35); font-weight: 700; }

  .log-msg        { color: var(--console-text); word-break: break-word; min-width: 0; }
  .log-msg.trace  { color: var(--console-text-dim); }
  .log-msg.debug  { color: var(--console-text-dim); }
  .log-msg.log-raw { color: var(--console-text-dim); white-space: pre; }
  .log-msg.info   { color: var(--text-1); }
  .log-msg.ok     { color: var(--green-bright); }
  .log-msg.warn   { color: var(--accent); }
  .log-msg.err    { color: #e74c3c; }
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
    color: var(--accent);
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
    z-index: 10;
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
  .ac-sugg.sel { background: var(--accent-glow); border-left-color: var(--accent); }

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