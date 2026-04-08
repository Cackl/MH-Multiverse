<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { openPath } from '@tauri-apps/plugin-opener'
  import { invoke } from '@tauri-apps/api/core'
  import { appConfig, setConsolePresets, serverRunning, setShutdownConfig, type ShutdownConfig } from '../lib/store'
  import { FALLBACK_COMMANDS } from '../lib/serverCommands'
  import PanelSidebar from './PanelSidebar.svelte'

  export let embedded = false
  export let onBack: (() => void) | null = null

  // -- Types --

  type FieldType = 'bool' | 'number' | 'string' | 'textarea'

  interface Field {
    key: string
    section: string
    type: FieldType
    label: string
    description: string
    min?: number
    max?: number
  }

  interface SubSection {
    title: string
    fields: Field[]
  }

  interface NavSection {
    id: string
    label: string
    fields?: Field[]
    subsections?: SubSection[]
  }

  type IniData = Record<string, Record<string, string>>

  // -- Helper --

  let openDirError = ''

  async function openConfigDir() {
    if (!$appConfig.server_exe) return
    openDirError = ''
    try {
      const dir = await invoke<string>('get_config_dir', { serverExe: $appConfig.server_exe })
      await openPath(dir)
    } catch (e) {
      openDirError = String(e)
    }
  }

  // -- Config schema --

  const schema: NavSection[] = [
    {
      id: 'server',
      label: 'Server',
      subsections: [
        {
          title: 'Frontend',
          fields: [
            { key: 'BindIP',        section: 'Frontend', type: 'string', label: 'Bind IP',        description: 'IP address the frontend server binds to. Set to 0.0.0.0 to listen on all interfaces.' },
            { key: 'Port',          section: 'Frontend', type: 'number', label: 'Port',            description: 'Port for the game client to connect to.', min: 1, max: 65535 },
            { key: 'PublicAddress', section: 'Frontend', type: 'string', label: 'Public Address',  description: 'Address clients use to reach this server. Can be an IP (e.g. 192.168.1.2) or hostname.' },
          ],
        },
        {
          title: 'Web Frontend',
          fields: [
            { key: 'Address',         section: 'WebFrontend', type: 'string', label: 'Address',          description: 'Address the web frontend listens on.' },
            { key: 'Port',            section: 'WebFrontend', type: 'number', label: 'Port',              description: 'Port for the web dashboard and API.', min: 1, max: 65535 },
            { key: 'EnableDashboard', section: 'WebFrontend', type: 'bool',   label: 'Enable Dashboard',  description: 'Enables the web dashboard accessible via browser. Requires EnableWebApi to be true.' },
          ],
        },
        {
          title: 'Identity & Access',
          fields: [
            { key: 'ServerName',               section: 'GroupingManager', type: 'string',   label: 'Server Name',           description: 'Name shown in chat for system messages sent by the server.' },
            { key: 'MotdText',                 section: 'GroupingManager', type: 'textarea', label: 'Message of the Day',    description: 'Message broadcast to players on login.' },
            { key: 'ServerPrestigeLevel',      section: 'GroupingManager', type: 'number',   label: 'Server Name Colour',    description: 'Colour of the server name in chat. 0=white 1=green 2=blue 3=purple 4=orange 5=red 6=yellow (cosmic).', min: 0, max: 6 },
            { key: 'UseWhitelist',             section: 'PlayerManager',   type: 'bool',     label: 'Use Whitelist',         description: 'When enabled, only accounts added via !account whitelist can log in.' },
            { key: 'ServerCapacity',           section: 'PlayerManager',   type: 'number',   label: 'Server Capacity',       description: 'Maximum concurrent players. 0 = unlimited. Players over capacity are queued.', min: 0 },
            { key: 'LoadAllPrototypes',        section: 'GameData',        type: 'bool',     label: 'Load All Prototypes',   description: 'Preloads all game data on startup. Makes the server start slower but eliminates in-game lag spikes when new areas are loaded for the first time.' },
            { key: 'UseEquipmentSlotTableCache', section: 'GameData',      type: 'bool',     label: 'Equipment Slot Cache',  description: 'Caches the equipment slot table. Slower startup unless used alongside Load All Prototypes.' },
          ],
        },
      ],
    },
    {
      id: 'persistence',
      label: 'Persistence',
      subsections: [
        {
          title: 'Player Manager',
          fields: [
            { key: 'EnablePersistence',          section: 'PlayerManager', type: 'bool', label: 'Enable Persistence',            description: 'Saves player data between sessions. Disable for a fresh-start-every-time experience.' },
            { key: 'AllowClientVersionMismatch', section: 'PlayerManager', type: 'bool', label: 'Allow Version Mismatch',         description: 'Allows clients whose game version does not match the server to connect.' },
            { key: 'UseJsonDBManager',           section: 'PlayerManager', type: 'bool', label: 'Use JSON Backend',              description: 'Use JSON file instead of SQLite for player data. Supports only a single account.' },
            { key: 'AutosaveIntervalMinutes',    section: 'CustomGameOptions', type: 'number', label: 'Autosave Interval (min)', description: 'How often player data is saved outside of region transfers. Set to 0 or less to disable autosaving.', min: -1 },
          ],
        },
        {
          title: 'SQLite DB',
          fields: [
            { key: 'FileName',              section: 'SQLiteDBManager', type: 'string', label: 'Database File',         description: 'SQLite database filename, relative to the server\'s Data directory.' },
            { key: 'MaxBackupNumber',       section: 'SQLiteDBManager', type: 'number', label: 'Max Backups',           description: 'Maximum number of backup files to keep. 0 disables backups.', min: 0 },
            { key: 'BackupIntervalMinutes', section: 'SQLiteDBManager', type: 'number', label: 'Backup Interval (min)', description: 'Minimum time in minutes between automatic backups.', min: 1 },
          ],
        },
        {
          title: 'JSON DB',
          fields: [
            { key: 'FileName',              section: 'JsonDBManager', type: 'string', label: 'Save File',             description: 'JSON save filename, relative to the server\'s Data directory.' },
            { key: 'MaxBackupNumber',       section: 'JsonDBManager', type: 'number', label: 'Max Backups',           description: 'Maximum number of backup files to keep. 0 disables backups.', min: 0 },
            { key: 'BackupIntervalMinutes', section: 'JsonDBManager', type: 'number', label: 'Backup Interval (min)', description: 'Minimum time in minutes between automatic backups.', min: 1 },
            { key: 'PlayerName',            section: 'JsonDBManager', type: 'string', label: 'Player Name',           description: 'Player name assigned to the single account when using the JSON backend.' },
          ],
        },
        {
          title: 'Leaderboards',
          fields: [
            { key: 'DatabaseFile',           section: 'Leaderboards', type: 'string', label: 'Database File',          description: 'Leaderboard SQLite filename relative to Data/Leaderboards.' },
            { key: 'ScheduleFile',           section: 'Leaderboards', type: 'string', label: 'Schedule File',          description: 'Leaderboard schedule JSON filename relative to Data/Leaderboards.' },
            { key: 'AutoSaveIntervalMinutes',section: 'Leaderboards', type: 'number', label: 'Autosave Interval (min)',description: 'Minimum time in minutes between leaderboard autosaves.', min: 1 },
          ],
        },
      ],
    },
    {
      id: 'gameplay',
      label: 'Gameplay',
      fields: [
        { key: 'AutoUnlockAvatars',                section: 'CustomGameOptions', type: 'bool',   label: 'Auto Unlock Heroes',             description: 'Automatically unlocks all heroes for players who complete the tutorial.' },
        { key: 'AutoUnlockTeamUps',                section: 'CustomGameOptions', type: 'bool',   label: 'Auto Unlock Team-Ups',           description: 'Automatically unlocks all team-ups for players who complete the tutorial.' },
        { key: 'ESCooldownOverrideMinutes',        section: 'CustomGameOptions', type: 'number', label: 'ES Drop Cooldown (min)',          description: 'Overrides the Eternity Splinter drop cooldown duration. Set to a negative value to use the default.' },
        { key: 'CombineESStacks',                  section: 'CustomGameOptions', type: 'bool',   label: 'Combine ES Stacks',              description: 'Merges multiple Eternity Splinter stacks into a single item when they drop at the same time.' },
        { key: 'DisableMovementPowerChargeCost',   section: 'CustomGameOptions', type: 'bool',   label: 'No Movement Power Charge Cost',  description: 'Removes charge costs for movement powers. Imitates pre-Biggest Update Ever behaviour.' },
        { key: 'AllowSameGroupTalents',            section: 'CustomGameOptions', type: 'bool',   label: 'Allow Same-Group Talents',       description: 'Allows mutually exclusive talents to be enabled at the same time.' },
        { key: 'EnableCreditChestConversion',      section: 'CustomGameOptions', type: 'bool',   label: 'Enable Credit Chest Conversion', description: 'Allows players to convert credits to sellable chest items via the !item creditchest command.' },
        { key: 'CreditChestConversionMultiplier',  section: 'CustomGameOptions', type: 'number', label: 'Chest Conversion Multiplier',    description: 'Credit cost multiplier when converting credits to chest items.', min: 0 },
        { key: 'DisableAccountBinding',            section: 'CustomGameOptions', type: 'bool',   label: 'Disable Account Binding',        description: 'Disables account-bound-on-pickup for items.' },
        { key: 'DisableCharacterBinding',          section: 'CustomGameOptions', type: 'bool',   label: 'Disable Character Binding',      description: 'Disables character-bound-on-equip for items.' },
        { key: 'UsePrestigeLootTable',             section: 'CustomGameOptions', type: 'bool',   label: 'Prestige Loot Table',            description: 'Replaces the starting costume prestige reward with items from the loot table.' },
        { key: 'EnableUltimatePrestige',           section: 'CustomGameOptions', type: 'bool',   label: 'Enable Ultimate Prestige',       description: 'Allows prestige level to be reset after reaching the prestige level cap.' },
      ],
    },
    {
      id: 'store',
      label: 'Store',
      fields: [
        { key: 'GazillioniteBalanceForNewAccounts', section: 'MTXStore', type: 'number', label: 'Starting G Balance',            description: 'Amount of Gs (Gazillionite) new accounts receive on first login.', min: 0 },
        { key: 'ESToGazillioniteConversionRatio',   section: 'MTXStore', type: 'number', label: 'ES to G Conversion Ratio',      description: 'Amount of Gs awarded per Eternity Splinter when converting.' },
        { key: 'ESToGazillioniteConversionStep',    section: 'MTXStore', type: 'number', label: 'ES to G Conversion Step',       description: 'Eternity Splinter step size for conversion, used to avoid rounding errors.', min: 1 },
        { key: 'GiftingOmegaLevelRequired',         section: 'MTXStore', type: 'number', label: 'Omega Level for Gifting',       description: 'Minimum Omega level required to purchase gifts for other players. 0 = no requirement.', min: 0 },
        { key: 'GiftingInfinityLevelRequired',      section: 'MTXStore', type: 'number', label: 'Infinity Level for Gifting',    description: 'Minimum Infinity level required to purchase gifts for other players. 0 = no requirement.', min: 0 },
      ],
    },
    {
      id: 'logging',
      label: 'Logging',
      fields: [
        { key: 'EnableLogging',           section: 'Logging', type: 'bool',   label: 'Enable Logging',              description: 'Master switch for the logging system.' },
        { key: 'HideSensitiveInformation',section: 'Logging', type: 'bool',   label: 'Hide Sensitive Info',         description: 'Masks email addresses and IP addresses in log output.' },
        { key: 'EnableConsole',           section: 'Logging', type: 'bool',   label: 'Console Output',              description: 'Outputs log messages to the console (captured here in MH Multiverse).' },
        { key: 'ConsoleIncludeTimestamps',section: 'Logging', type: 'bool',   label: 'Console Timestamps',          description: 'Includes message timestamps in console output.' },
        { key: 'ConsoleMinLevel',         section: 'Logging', type: 'number', label: 'Console Min Level',           description: 'Minimum log level for console output. 0=trace 1=debug 2=info 3=warn 4=error 5=fatal.', min: 0, max: 5 },
        { key: 'ConsoleMaxLevel',         section: 'Logging', type: 'number', label: 'Console Max Level',           description: 'Maximum log level for console output.', min: 0, max: 5 },
        { key: 'EnableFile',              section: 'Logging', type: 'bool',   label: 'File Output',                 description: 'Outputs log messages to a file in the server directory.' },
        { key: 'FileIncludeTimestamps',   section: 'Logging', type: 'bool',   label: 'File Timestamps',             description: 'Includes message timestamps in file output.' },
        { key: 'FileMinLevel',            section: 'Logging', type: 'number', label: 'File Min Level',              description: 'Minimum log level for file output.', min: 0, max: 5 },
        { key: 'FileMaxLevel',            section: 'Logging', type: 'number', label: 'File Max Level',              description: 'Maximum log level for file output.', min: 0, max: 5 },
        { key: 'FileSplitOutput',         section: 'Logging', type: 'bool',   label: 'Split File Output',           description: 'Splits log file output into separate files based on message category.' },
      ],
    },
    {
      id: 'multiverse',
      label: 'MH Multiverse',
    }
  ]

  // -- State --

  let activeSection = schema[0].id
  let values: IniData = {}
  let savedValues: IniData = {}
  let overridden: Record<string, string[]> = {}
  let loaded = false
  let loadError = ''
  let saveError = ''
  let saveSuccess = false
  let tooltip = ''
  let tooltipX = 0
  let tooltipY = 0
  let tooltipVisible = false
  let presets: string[] = []

  $: canLoad = !!$appConfig.server_exe
  $: activeSchema = schema.find(s => s.id === activeSection)!
  $: allFields = activeSchema.fields
    ?? activeSchema.subsections?.flatMap(s => s.fields)
    ?? []
  $: sections = activeSchema.subsections
    ? activeSchema.subsections.map(s => ({ title: s.title, fields: s.fields }))
    : [{ title: activeSchema.label, fields: activeSchema.fields ?? [] }]
  $: dirty = loaded && allFields.some(f =>
    (values[f.section]?.[f.key] ?? '') !== (savedValues[f.section]?.[f.key] ?? '')
  )
  $: presets = [...($appConfig.console_presets ?? [])]

  // -- Load / save --

  async function load() {
    loadError = ''
    try {
      const state = await invoke<{ values: IniData; overridden: Record<string, string[]> }>(
        'read_config', { serverExe: $appConfig.server_exe }
      )
      values = state.values
      savedValues = JSON.parse(JSON.stringify(state.values))
      overridden = state.overridden
      loaded = true
    } catch (e) {
      loadError = String(e)
    }
  }

  async function save() {
    saveError = ''
    saveSuccess = false
    const updates: IniData = {}
    for (const field of allFields) {
      const val = values[field.section]?.[field.key]
      if (val !== undefined) {
        if (!updates[field.section]) updates[field.section] = {}
        updates[field.section][field.key] = val
      }
    }
    try {
      await invoke('write_config', { serverExe: $appConfig.server_exe, updates })
      await load()
      saveSuccess = true
      setTimeout(() => saveSuccess = false, 3000)
    } catch (e) {
      saveError = String(e)
    }
  }

  async function resetSection() {
    saveError = ''
    const iniSections = [...new Set(allFields.map(f => f.section))]
    try {
      for (const section of iniSections) {
        await invoke('reset_config_section', { serverExe: $appConfig.server_exe, section })
      }
      await load()
    } catch (e) {
      saveError = String(e)
    }
  }

  function getValue(field: Field): string {
    return values[field.section]?.[field.key] ?? ''
  }

  function setValue(field: Field, val: string) {
    if (!values[field.section]) values[field.section] = {}
    values[field.section][field.key] = val
    values = { ...values }
  }

  function isOverridden(field: Field): boolean {
    return (overridden[field.section] ?? []).includes(field.key)
  }

  function isModified(field: Field): boolean {
    return (values[field.section]?.[field.key] ?? '') !== (savedValues[field.section]?.[field.key] ?? '')
  }

  // -- Tooltip --

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

    if (x + rect.width + VIEWPORT_PAD > vw) {
      x = e.clientX - rect.width - TOOLTIP_OFFSET
    }
    x = Math.max(VIEWPORT_PAD, Math.min(x, vw - rect.width - VIEWPORT_PAD))

    if (y < VIEWPORT_PAD) {
      y = e.clientY + TOOLTIP_OFFSET
    }
    y = Math.max(VIEWPORT_PAD, Math.min(y, vh - rect.height - VIEWPORT_PAD))

    tooltipX = x
    tooltipY = y
  }

  function hideTooltip() {
    tooltipVisible = false
  }

  // -- Console presets --

  function syncPresets() {
    setConsolePresets(presets)
  }

  function removePreset(i: number) {
    presets = presets.filter((_, idx) => idx !== i)
    syncPresets()
  }

  function movePresetUp(i: number) {
    if (i === 0) return
    const next = [...presets]
    ;[next[i - 1], next[i]] = [next[i], next[i - 1]]
    presets = next
    syncPresets()
  }

  function movePresetDown(i: number) {
    if (i === presets.length - 1) return
    const next = [...presets]
    ;[next[i], next[i + 1]] = [next[i + 1], next[i]]
    presets = next
    syncPresets()
  }

  // -- Preset autocomplete --

  interface Cmd { f: string; a: string; d: string; invokerType?: string }
  const presetCommands: Cmd[] = FALLBACK_COMMANDS.map(c => ({ ...c, invokerType: 'Any' }))

  let presetInput = ''
  let presetInputEl: HTMLInputElement | null = null
  let pacVisible = false
  let pacSuggs: Cmd[] = []
  let pacResolved: Cmd | null = null
  let pacSel = -1
  let pacFocused = false
  let pacPanelEl: HTMLElement | null = null

  $: if (pacSel >= 0 && pacPanelEl) {
    const selected = pacPanelEl.querySelector<HTMLElement>('.pac-sugg.sel')
    selected?.scrollIntoView({ block: 'nearest' })
  }

  function pacLcp(strs: string[]): string {
    if (!strs.length) return ''
    let p = strs[0]
    for (let i = 1; i < strs.length; i++) {
      while (!strs[i].startsWith(p)) p = p.slice(0, -1)
      if (!p) return ''
    }
    return p
  }

  function pacGetSuggs(v: string): Cmd[] {
    if (!v || v === '!') return []
    const lo = v.toLowerCase()
    return presetCommands.filter(c => c.f.startsWith(lo))
  }

  function pacFindResolved(v: string): Cmd | null {
    const lo = v.toLowerCase()
    return presetCommands.find(c => lo === c.f || lo.startsWith(c.f + ' ')) ?? null
  }

  function pacUpdate(v: string) {
    if (!v || v === '!') {
      pacVisible = false; pacSuggs = []; pacResolved = null; pacSel = -1
      return
    }
    const r = pacFindResolved(v)
    if (r) {
      pacResolved = r; pacSuggs = []; pacSel = -1; pacVisible = true
    } else {
      pacResolved = null; pacSel = -1; pacSuggs = pacGetSuggs(v); pacVisible = pacSuggs.length > 0
    }
  }

  function pacCompleteToCmd(c: Cmd) {
    presetInput = c.f + (c.a ? ' ' : '')
    pacSel = -1
    pacUpdate(presetInput)
  }

  function pacDoTab() {
    if (pacResolved) return
    if (!pacSuggs.length) return
    const prefix = pacLcp(pacSuggs.map(c => c.f))
    if (prefix.length > presetInput.length) {
      if (pacSuggs.length === 1) {
        pacCompleteToCmd(pacSuggs[0])
      } else {
        presetInput = prefix
        pacSel = -1
        pacUpdate(prefix)
      }
      return
    }
    if (pacSel === -1) {
      pacSel = 0
    } else {
      pacCompleteToCmd(pacSuggs[pacSel])
    }
  }

  function onPresetInputKeydown(e: KeyboardEvent) {
    if (e.key === 'Tab') {
      e.preventDefault()
      pacDoTab()
      return
    }
    if (e.key === 'Escape') {
      pacVisible = false; pacSel = -1; pacResolved = null
      return
    }
    if (e.key === 'Enter') {
      if (pacSel >= 0 && !pacResolved) {
        e.preventDefault()
        pacCompleteToCmd(pacSuggs[pacSel])
        return
      }
      addPreset()
      return
    }
    if (pacResolved || !pacVisible) return
    if (e.key === 'ArrowDown') { e.preventDefault(); pacSel = Math.min(pacSel + 1, pacSuggs.length - 1) }
    else if (e.key === 'ArrowUp') { e.preventDefault(); pacSel = Math.max(pacSel - 1, -1) }
  }

  function addPreset() {
    const v = presetInput.trim()
    if (!v) return
    presets = [...presets, v]
    presetInput = ''
    pacVisible = false; pacSuggs = []; pacResolved = null; pacSel = -1
    syncPresets()
  }

  // -- Preset drag reorder (pointer events) --
  //
  // HTML5 drag-and-drop is unreliable in Tauri WebViews. Instead we use pointer
  // events on the drag handle only: pointerdown starts the drag, pointermove and
  // pointerup are registered on window for the duration, and elementFromPoint
  // identifies the target chip under the cursor.

  let dragIndex: number | null = null
  let dragOverIndex: number | null = null

  function onHandlePointerDown(e: PointerEvent, i: number) {
    e.preventDefault()
    ;(e.currentTarget as HTMLElement).setPointerCapture(e.pointerId)
    dragIndex = i
    dragOverIndex = i

    function onPointerMove(me: PointerEvent) {
      const el = document.elementFromPoint(me.clientX, me.clientY)
      const chip = el?.closest<HTMLElement>('[data-chip-index]')
      if (chip) {
        const idx = parseInt(chip.dataset.chipIndex ?? '', 10)
        if (!isNaN(idx)) dragOverIndex = idx
      }
    }

    function onPointerUp() {
      if (dragIndex !== null && dragOverIndex !== null && dragIndex !== dragOverIndex) {
        const next = [...presets]
        const [item] = next.splice(dragIndex, 1)
        next.splice(dragOverIndex, 0, item)
        presets = next
        syncPresets()
      }
      dragIndex = null
      dragOverIndex = null
      window.removeEventListener('pointermove', onPointerMove)
      window.removeEventListener('pointerup', onPointerUp)
    }

    window.addEventListener('pointermove', onPointerMove)
    window.addEventListener('pointerup', onPointerUp)
  }

  // -- Preset chip keyboard reorder --

  function onChipKeydown(e: KeyboardEvent, i: number) {
    if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {
      e.preventDefault()
      if (i === 0) return
      movePresetUp(i)
      tick().then(() => {
        document.querySelectorAll<HTMLElement>('[data-chip-index]')[i - 1]?.focus()
      })
    } else if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {
      e.preventDefault()
      if (i === presets.length - 1) return
      movePresetDown(i)
      tick().then(() => {
        document.querySelectorAll<HTMLElement>('[data-chip-index]')[i + 1]?.focus()
      })
    } else if (e.key === 'Delete' || e.key === 'Backspace') {
      e.preventDefault()
      removePreset(i)
      tick().then(() => {
        const chips = document.querySelectorAll<HTMLElement>('[data-chip-index]')
        if (chips.length) {
          chips[Math.min(i, chips.length - 1)]?.focus()
        } else {
          presetInputEl?.focus()
        }
      })
    }
  }

  // --

  onMount(() => {
    if (canLoad) load()
  })
</script>

{#if tooltipVisible}
  <div
    bind:this={tooltipEl}
    class="tooltip"
    style="left:{tooltipX}px; top:{tooltipY}px"
  >
    {tooltip}
  </div>
{/if}

<div class="config-panel">
  <div class="panel-bg"></div>
  <div class="grid-overlay"></div>
  <div class="config-layout">

  <!-- Left nav -->
  <PanelSidebar width="var(--sidebar-narrow)">
    <svelte:fragment slot="header">
      {#if onBack}
        <button class="btn-icon" on:click={onBack} title="Back" style="margin-right:4px;">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="15 18 9 12 15 6"/>
          </svg>
        </button>
      {/if}
      {#if !embedded}
        <div class="section-title">Config</div>
      {/if}

      <button
        class="btn-icon"
        on:click={openConfigDir}
        title="Open MHServerEmu folder"
        disabled={!$appConfig.server_exe}
        style="margin-left:auto;"
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
        </svg>
      </button>
    </svelte:fragment>
    <div class="config-nav-list">
      {#each schema as section}
        <button
          class="config-nav-item"
          class:active={activeSection === section.id}
          on:click={() => activeSection = section.id}
        >
          {section.label}
        </button>
      {/each}
    </div>
  </PanelSidebar>

  <!-- Main content -->
  <div class="config-main">

    {#if !canLoad && activeSection !== 'multiverse'}
      <div class="config-notice">
        <div class="notice-icon">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:20px;height:20px;color:var(--text-3)">
            <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/>
            <circle cx="12" cy="12" r="3"/>
          </svg>
        </div>
        <div class="notice-text">Set the server executable path in App settings to load configuration.</div>
      </div>
    {:else if activeSection === 'multiverse'}
      <div class="config-content">
        <div class="config-section-head">
          <div class="section-title">MH Multiverse</div>
        </div>
        <div class="config-body">
          <div class="multiverse-note">
            These settings are stored in MH Multiverse, not in ConfigOverride.ini.
          </div>

          <div class="subsection-title">Shutdown</div>
          <div class="config-grid">
            <div class="config-field">
              <div class="config-field-head">
                <span class="config-field-label">Delay (min)</span>
              </div>
              <input
                type="number"
                class="config-input"
                min="0"
                value={$appConfig.shutdown.delay_minutes}
                on:change={(e) => setShutdownConfig({
                  ...$appConfig.shutdown,
                  delay_minutes: Math.max(0, parseInt(e.currentTarget.value) || 0)
                })}
              >
              <span class="config-field-hint">0 = stop immediately with no broadcast</span>
            </div>
            <div class="config-field">
              <div class="config-field-head">
                <span class="config-field-label">Broadcast Message</span>
              </div>
              <input
                type="text"
                class="config-input"
                value={$appConfig.shutdown.broadcast_message}
                placeholder="Server is shutting down in {'{minutes}'} minute(s)."
                on:change={(e) => setShutdownConfig({
                  ...$appConfig.shutdown,
                  broadcast_message: e.currentTarget.value
                })}
              >
              <span class="config-field-hint">Use {'{minutes}'} as a placeholder for the remaining time.</span>
            </div>
          </div>

          <div class="subsection-title">Console</div>
          <p class="subsection-desc">
            Saved commands shown as quick-access chips in the console.
            Drag by the handle or use arrow keys on a focused chip to reorder. Delete or Backspace removes it.
          </p>

          {#if presets.length > 0}
            <div class="preset-chips" role="list" aria-label="Console presets">
              {#each presets as preset, i}
                <div
                  class="preset-chip"
                  class:is-dragging={dragIndex === i}
                  class:is-drag-over={dragOverIndex === i && dragIndex !== null && dragIndex !== i}
                  data-chip-index={i}
                  tabindex="0"
                  role="button"
                  aria-label="{preset}, position {i + 1} of {presets.length}. Arrow keys to reorder, Delete to remove."
                  on:keydown={(e) => onChipKeydown(e, i)}
                >
                  <svg
                    class="preset-chip-handle"
                    viewBox="0 0 8 14"
                    fill="currentColor"
                    aria-hidden="true"
                    on:pointerdown={(e) => onHandlePointerDown(e, i)}
                  >
                    <circle cx="2" cy="2"  r="1.2"/>
                    <circle cx="6" cy="2"  r="1.2"/>
                    <circle cx="2" cy="7"  r="1.2"/>
                    <circle cx="6" cy="7"  r="1.2"/>
                    <circle cx="2" cy="12" r="1.2"/>
                    <circle cx="6" cy="12" r="1.2"/>
                  </svg>
                  <span class="preset-chip-text">{preset}</span>
                  <button
                    class="preset-chip-remove"
                    tabindex="-1"
                    aria-label="Remove {preset}"
                    on:click|stopPropagation={() => removePreset(i)}
                  >
                    <svg viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
                      <line x1="2" y1="2" x2="8" y2="8"/>
                      <line x1="8" y1="2" x2="2" y2="8"/>
                    </svg>
                  </button>
                </div>
              {/each}
            </div>
          {:else}
            <div class="preset-empty">No presets yet</div>
          {/if}

          <div class="preset-add-wrap">
            {#if pacVisible && pacFocused}
              <div class="pac-panel" bind:this={pacPanelEl}>
                <div class="pac-header">
                  <span class="pac-hint">TAB to complete &nbsp;|&nbsp; ↑↓ navigate &nbsp;|&nbsp; ESC dismiss</span>
                  {#if !pacResolved}
                    <span class="pac-count">{pacSuggs.length} match{pacSuggs.length !== 1 ? 'es' : ''}</span>
                  {/if}
                </div>
                {#if pacResolved}
                  <div class="pac-resolved">
                    <div class="pac-resolved-cmd">
                      <span class="pac-resolved-base">{pacResolved.f}</span>
                      {#if pacResolved.a}<span class="pac-resolved-args"> {pacResolved.a}</span>{/if}
                    </div>
                    <div class="pac-resolved-desc">{pacResolved.d}</div>
                  </div>
                {:else}
                  {#each pacSuggs as c, i (c.f)}
                    <div
                      class="pac-sugg"
                      class:sel={i === pacSel}
                      on:mousedown|preventDefault={() => pacCompleteToCmd(c)}
                      role="option"
                      tabindex="-1"
                      aria-selected={i === pacSel}
                    >
                      <div class="pac-sugg-top">
                        <span class="pac-sugg-cmd">
                          <span class="pac-typed">{c.f.slice(0, presetInput.length)}</span>{c.f.slice(presetInput.length)}
                        </span>
                        {#if c.a}<span class="pac-sugg-args"> {c.a}</span>{/if}
                      </div>
                      <div class="pac-sugg-desc">{c.d}</div>
                    </div>
                  {/each}
                {/if}
              </div>
            {/if}
            <div class="preset-add-row">
              <span class="preset-add-prefix">&gt;</span>
              <input
                type="text"
                class="preset-add-input"
                bind:value={presetInput}
                bind:this={presetInputEl}
                placeholder="Type a command..."
                on:input={() => pacUpdate(presetInput)}
                on:keydown={onPresetInputKeydown}
                on:focus={() => { pacFocused = true; if (presetInput) pacUpdate(presetInput) }}
                on:blur={() => setTimeout(() => { pacFocused = false; pacVisible = false }, 150)}
              >
              <button class="btn btn-sm btn-accent" on:click={addPreset}>Add</button>
            </div>
          </div>

        </div>
      </div>

    {:else if !loaded}
      <div class="config-notice">
        <div class="notice-text">Config not loaded yet.</div>
        {#if loadError}
          <div class="notice-error">{loadError}</div>
        {/if}
        <button class="btn btn-accent" on:click={load}>Load Config</button>
      </div>
    {:else}

      <div class="config-content">
        {#if $serverRunning}
          <div class="running-warn">
            Server is currently running -- changes will take effect on next restart.
          </div>
        {/if}

        <div class="config-section-head">
          <div class="section-title">{activeSchema.label}</div>
        </div>

        <div class="config-body">
          {#each sections as section}
            {#if activeSchema.subsections}
              <div class="subsection-title">{section.title}</div>
            {/if}

            <div class="config-grid">
              {#each section.fields as field}
                {#if field.type === 'bool'}
                  <div class="config-field full">
                    <div class="toggle-row">
                      <div class="toggle-info">
                        <span class="config-field-label">{field.label}</span>
                        <span class="config-tooltip">{field.description}</span>
                      </div>
                      <div
                        class="toggle-switch"
                        class:on={getValue(field).toLowerCase() === 'true'}
                        on:click={() => setValue(field, getValue(field).toLowerCase() === 'true' ? 'false' : 'true')}
                        role="switch"
                        aria-checked={getValue(field).toLowerCase() === 'true'}
                        tabindex="0"
                        on:keydown={(e) => e.key === 'Enter' && setValue(field, getValue(field).toLowerCase() === 'true' ? 'false' : 'true')}
                      ></div>
                    </div>
                  </div>
                {:else if field.type === 'textarea'}
                  <div class="config-field full">
                    <div class="config-field-head">
                      <span class="config-field-label">{field.label}</span>
                      {#if isOverridden(field)}
                        <span class="override-dot" title="Overridden from default"></span>
                      {/if}
                      <span class="config-field-default">
                        <button
                          class="info-btn"
                          on:mouseenter={(e) => showTooltip(e, field.description)}
                          on:mouseleave={hideTooltip}
                          tabindex="-1"
                        >?</button>
                      </span>
                    </div>
                    <textarea
                      value={getValue(field)}
                      on:change={(e) => setValue(field, e.currentTarget.value)}
                      rows="3"
                      class:modified={isModified(field)}
                    ></textarea>
                  </div>
                {:else}
                  <div class="config-field">
                    <div class="config-field-head">
                      <span class="config-field-label">{field.label}</span>
                      {#if isOverridden(field)}
                        <span class="override-dot" title="Overridden from default"></span>
                      {/if}
                      <span class="config-field-default">
                        <button
                          class="info-btn"
                          on:mouseenter={(e) => showTooltip(e, field.description)}
                          on:mouseleave={hideTooltip}
                          tabindex="-1"
                        >?</button>
                      </span>
                    </div>
                    {#if field.type === 'number'}
                      <input
                        type="number"
                        class="config-input"
                        class:modified={isModified(field)}
                        value={getValue(field)}
                        min={field.min}
                        max={field.max}
                        on:change={(e) => setValue(field, e.currentTarget.value)}
                      >
                    {:else}
                      <input
                        type="text"
                        class="config-input"
                        class:modified={isModified(field)}
                        value={getValue(field)}
                        on:change={(e) => setValue(field, e.currentTarget.value)}
                      >
                    {/if}
                  </div>
                {/if}
              {/each}
            </div>
          {/each}
        </div>
      </div>

      <!-- Footer -->
      <div class="panel-footer">
        {#if dirty}
          <span class="dirty-badge">Unsaved changes</span>
        {/if}
        {#if saveError}
          <span class="feedback-error">{saveError}</span>
        {/if}
        {#if saveSuccess}
          <span class="feedback-ok">Saved</span>
        {/if}
        <button class="btn btn-sm btn-outline" style="margin-left:auto;" on:click={resetSection} disabled={!loaded}>Reset to Defaults</button>
        <button class="btn btn-sm btn-accent" class:btn-pulse={dirty} on:click={save} disabled={!loaded}>Save</button>
      </div>

    {/if}
  </div>
  </div><!-- config-layout -->
</div>

<style>
  .config-panel {
    display: flex;
    flex: 1;
    flex-direction: column;
    position: relative;
    overflow: hidden;
    min-height: 0;
  }

  .config-layout {
    position: relative;
    z-index: 1;
    flex: 1;
    display: flex;
    overflow: hidden;
    min-height: 0;
  }

  .config-nav-list {
    padding: 6px;
  }

  .config-nav-item {
    display: block;
    width: 100%;
    text-align: left;
    padding: 10px 12px;
    font-family: var(--font-head);
    font-size: 13px;
    font-weight: 600;
    color: var(--text-1);
    background: none;
    border: 1px solid transparent;
    cursor: pointer;
    border-radius: var(--radius-sm);
    transition: all 0.12s;
    margin-bottom: 2px;
  }
  .config-nav-item:hover { color: var(--text-0); background: var(--bg-3); border-color: var(--border-mid); }
  .config-nav-item.active { color: var(--accent-bright); background: var(--accent-glow); border-color: var(--accent-dim); }

  /* -- Main area -- */
  .config-main {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    min-height: 0;
    background: var(--bg-1);
  }

  .config-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .config-section-head {
    display: flex;
    align-items: center;
    padding: 12px 20px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    min-height: 53px;
  }

  .config-body {
    flex: 1;
    overflow-y: auto;
    padding: 16px 20px;
  }

  .config-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 14px;
    margin-bottom: 20px;
  }

  .config-field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .config-field.full { grid-column: 1 / -1; }

  .config-field-head {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .config-field-label {
    font-family: var(--font-head);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-1);
  }

  .config-field-default {
    margin-left: auto;
  }

  .config-input {
    background: var(--bg-0);
    border: 1px solid var(--border-mid);
    color: var(--text-0);
    font-family: var(--font-mono);
    font-size: 12px;
    padding: 7px 10px;
    outline: none;
    border-radius: var(--radius-sm);
    transition: border-color 0.15s;
    width: 100%;
  }
  .config-input:focus { border-color: var(--accent-dim); }
  .config-input.modified {
    border-color: var(--accent-dim);
    background: var(--accent-glow);
  }

  .config-tooltip {
    font-size: 11px;
    color: var(--text-3);
    line-height: 1.4;
  }

  /* -- Toggle rows (booleans) -- */
  .toggle-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 0;
    border-bottom: 1px solid var(--border);
  }
  .toggle-row:last-child { border-bottom: none; }

  .toggle-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  /* -- Override dot -- */
  .override-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    flex-shrink: 0;
  }

  /* -- Info button -- */
  .info-btn {
    width: 14px;
    height: 14px;
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

  /* -- Textarea -- */
  textarea {
    background: var(--bg-0);
    border: 1px solid var(--border-mid);
    color: var(--text-0);
    font-family: var(--font-body);
    font-size: 12px;
    padding: 7px 10px;
    outline: none;
    width: 100%;
    resize: vertical;
    border-radius: var(--radius-sm);
    transition: border-color 0.15s;
  }
  textarea:focus { border-color: var(--accent-dim); }
  textarea.modified {
    border-color: var(--accent-dim);
    background: var(--accent-glow);
  }

  /* -- Notices -- */
  .config-notice {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    padding: 24px;
  }

  .notice-text {
    font-family: var(--font-head);
    font-size: 12px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
  }

  .notice-error {
    font-size: 12px;
    color: var(--text-error);
  }

  .running-warn {
    padding: 8px 20px;
    background: var(--amber-dim);
    border-bottom: 1px solid rgba(200, 146, 10, 0.2);
    font-size: 12px;
    color: var(--amber);
    font-family: var(--font-head);
    letter-spacing: 0.06em;
    flex-shrink: 0;
  }

  .multiverse-note {
    font-size: 11px;
    color: var(--text-3);
    margin-bottom: 14px;
    font-family: var(--font-body);
  }

  .config-field-hint {
    font-size: 10px;
    color: var(--text-3);
    font-family: var(--font-body);
    margin-top: 2px;
  }

  /* -- Tooltip -- */
  .tooltip {
    position: fixed;
    z-index: var(--z-tooltip);
    background: var(--bg-3);
    border: 1px solid var(--border-lit);
    border-radius: var(--radius-sm);
    color: var(--text-1);
    font-size: 12px;
    font-family: var(--font-body);
    padding: 6px 10px;
    max-width: 300px;
    line-height: 1.5;
    pointer-events: none;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
  }

  /* -- Subsection description -- */
  .subsection-desc {
    font-size: 11px;
    color: var(--text-3);
    line-height: 1.5;
    margin: -8px 0 14px;
    font-family: var(--font-body);
  }

  /* -- Preset chips -- */
  .preset-chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 12px;
  }

  .preset-chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 4px 6px 4px 5px;
    background: var(--bg-3);
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-1);
    user-select: none;
    transition: border-color 0.12s, background 0.12s, opacity 0.12s;
    outline: none;
  }
  .preset-chip:focus {
    border-color: var(--accent-dim);
    background: var(--accent-glow);
    box-shadow: 0 0 0 2px var(--accent-glow);
  }
  .preset-chip.is-dragging {
    opacity: 0.4;
  }
  .preset-chip.is-drag-over {
    border-color: var(--accent-dim);
    background: var(--accent-glow);
  }

  .preset-chip-handle {
    width: 8px;
    height: 14px;
    color: var(--text-3);
    flex-shrink: 0;
    cursor: grab;
    touch-action: none;
  }
  .preset-chip-handle:active {
    cursor: grabbing;
  }

  .preset-chip-text {
    line-height: 1;
  }

  .preset-chip-remove {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    padding: 0;
    background: none;
    border: none;
    color: var(--text-3);
    cursor: pointer;
    border-radius: 2px;
    flex-shrink: 0;
    transition: color 0.1s, background 0.1s;
  }
  .preset-chip-remove:hover {
    color: var(--text-error);
    background: var(--red-dim);
  }
  .preset-chip-remove svg {
    width: 10px;
    height: 10px;
  }

  .preset-empty {
    font-size: 11px;
    color: var(--text-3);
    font-family: var(--font-head);
    letter-spacing: 0.06em;
    margin-bottom: 12px;
    padding: 10px 0;
  }

  /* -- Preset add row -- */
  .preset-add-wrap {
    position: relative;
  }

  .preset-add-row {
    display: flex;
    align-items: center;
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    background: var(--bg-0);
    overflow: hidden;
    transition: border-color 0.15s;
  }
  .preset-add-wrap:focus-within .preset-add-row {
    border-color: var(--accent-dim);
  }

  .preset-add-prefix {
    padding: 0 0 0 10px;
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--accent-dim);
    user-select: none;
    flex-shrink: 0;
  }

  .preset-add-input {
    flex: 1;
    background: none;
    border: none;
    color: var(--text-0);
    font-family: var(--font-mono);
    font-size: 12px;
    padding: 7px 8px;
    outline: none;
  }
  .preset-add-input::placeholder { color: var(--text-3); }

  .preset-add-row .btn {
    margin: 4px;
    flex-shrink: 0;
  }

  /* -- Preset autocomplete panel -- */
  .pac-panel {
    position: absolute;
    bottom: 100%;
    left: 0;
    right: 0;
    margin-bottom: 4px;
    background: var(--bg-2);
    border: 1px solid var(--border-lit);
    border-radius: var(--radius-sm);
    max-height: 240px;
    overflow-y: auto;
    z-index: var(--z-dropdown);
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.35);
  }
  .pac-panel::-webkit-scrollbar { width: 4px; }
  .pac-panel::-webkit-scrollbar-thumb { background: var(--border-lit); }

  .pac-header {
    padding: 5px 12px;
    background: var(--bg-0);
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-shrink: 0;
  }
  .pac-hint {
    font-family: var(--font-head);
    font-size: 9px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-3);
  }
  .pac-count {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-3);
  }

  .pac-sugg {
    padding: 7px 12px;
    cursor: pointer;
    border-bottom: 1px solid var(--border);
    border-left: 2px solid transparent;
    transition: background 0.08s;
  }
  .pac-sugg:last-child { border-bottom: none; }
  .pac-sugg:hover { background: var(--accent-glow); }
  .pac-sugg.sel { background: var(--accent-glow); border-left-color: var(--accent-dim); }

  .pac-sugg-top { display: flex; align-items: baseline; gap: 6px; }
  .pac-sugg-cmd { font-family: var(--font-mono); font-size: 12px; color: var(--text-1); }
  .pac-typed { color: var(--accent-bright); }
  .pac-sugg-args { font-family: var(--font-mono); font-size: 12px; color: var(--text-3); }
  .pac-sugg-desc { font-size: 11px; color: var(--text-3); margin-top: 3px; line-height: 1.4; }

  .pac-resolved { padding: 8px 12px; }
  .pac-resolved-cmd { font-family: var(--font-mono); font-size: 12px; }
  .pac-resolved-base { color: var(--green-bright); }
  .pac-resolved-args { color: var(--text-3); }
  .pac-resolved-desc { font-size: 11px; color: var(--text-3); margin-top: 3px; }
</style>