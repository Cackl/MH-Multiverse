import { writable, derived, get } from 'svelte/store'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { versionCaps } from './versionCaps'

export type DataTab = 'events' | 'tuning' | 'store' | 'patches'
export type Tab = 'launch' | 'server' | 'config' | 'data' | 'ops' | 'settings'
export type LogLevel = 'all' | 'trace' | 'debug' | 'info' | 'ok' | 'warn' | 'err' | 'fatal'

export const activeTab = writable<Tab>('launch')
export type SettingsSection = 'client' | 'display' | 'integrations' | 'about'
export const settingsSection = writable<SettingsSection>('client')
export const activeDataTab = writable<DataTab>('events')

// Editors that currently hold unsaved edits, keyed by source. Switching tab
// unmounts the panel, so navigation goes through confirmLeave first.
export const unsavedSources = writable<Record<string, string>>({})

export function setUnsaved(key: string, label: string | null) {
  unsavedSources.update(s => {
    if (label === null) {
      if (!(key in s)) return s
      const { [key]: _removed, ...rest } = s
      return rest
    }
    return s[key] === label ? s : { ...s, [key]: label }
  })
}

// The pending in-app confirmation, rendered by ConfirmDialog.svelte.
export type ConfirmRequest = {
  title: string
  message: string
  okLabel: string
  cancelLabel: string
  resolve: (ok: boolean) => void
}
export const confirmRequest = writable<ConfirmRequest | null>(null)

export function askConfirm(opts: Omit<ConfirmRequest, 'resolve'>): Promise<boolean> {
  get(confirmRequest)?.resolve(false) // a newer question replaces an unanswered one
  return new Promise(resolve => {
    const request: ConfirmRequest = {
      ...opts,
      resolve: ok => {
        // Only clear if still ours; a replacement may already be showing.
        if (get(confirmRequest) === request) confirmRequest.set(null)
        resolve(ok)
      },
    }
    confirmRequest.set(request)
  })
}

// True when it's safe to navigate away: nothing unsaved, or the user agreed to
// discard it.
export async function confirmLeave(): Promise<boolean> {
  const labels = Object.values(get(unsavedSources))
  if (labels.length === 0) return true
  const ok = await askConfirm({
    title: 'Discard changes?',
    message: `You have unsaved changes in ${labels.join(', ')}. Discard them?`,
    okLabel: 'Discard',
    cancelLabel: 'Keep editing',
  })
  if (ok) unsavedSources.set({})
  return ok
}

export async function goTab(tab: Tab) {
  if (tab === get(activeTab)) return
  if (await confirmLeave()) activeTab.set(tab)
}

// Switches to the Data rail entry on `sub`, from wherever we are.
export async function goDataTab(sub: DataTab) {
  if (get(activeTab) === 'data' && sub === get(activeDataTab)) return
  if (!(await confirmLeave())) return
  activeTab.set('data')
  activeDataTab.set(sub)
}
export const serverRunning = writable<boolean>(false)

// Events saved since the running server started. The server reads Events and EventSchedule
// only at startup, so both Events views show a restart notice until it starts again.
export const eventsRestartPending = writable<boolean>(false)
let eventsSawStop = false
export function markEventsSaved() {
  eventsRestartPending.set(true)
  eventsSawStop = !get(serverRunning)
}
serverRunning.subscribe(running => {
  if (!running) eventsSawStop = true
  else if (eventsSawStop) eventsRestartPending.set(false)
})
// True once the server has finished starting and reads console commands (see serverEvents.ts).
export const serverReady = writable<boolean>(false)
serverRunning.subscribe(running => { if (!running) serverReady.set(false) })
export const gameRunning = writable<boolean>(false)
export const apacheRunning = writable<boolean>(false)
export const uptimeSec = writable<number>(0)

// True when the linked server keeps one built-in account in a JSON file instead of Account.db.
export const jsonPersistence = writable<boolean>(false)
export async function refreshPersistence(serverExe: string) {
  jsonPersistence.set(
    serverExe ? await invoke<boolean>('uses_json_persistence', { serverExe }).catch(() => false) : false
  )
}
export const serverError = writable<string>('')

export type OverlayStatus = {
  state: 'not_linked' | 'missing' | 'untrusted' | 'trusted' | 'changed'
  dir: string
  auto_inject: boolean
  omega_gear: boolean
  changes: string[]
}
/** Outcome of the most recent overlay injection attempt (null until one happens). */
export type OverlayHit = { time: string; health: string; damage: string; kind: string; attacker: string; power: string; note: string }
export type OverlayLine = { id: number; event: string; text: string; fields: string[]; hits: OverlayHit[] }
/** Live overlay events (plain text only), newest last, capped at 500. */
export const overlayLog = writable<OverlayLine[]>([])
let overlayLineId = 0
export function appendOverlayLog(lines: Omit<OverlayLine, 'id'>[]) {
  overlayLog.update(l => [...l, ...lines.map(x => ({ ...x, id: overlayLineId++ }))].slice(-500))
}
/** Latest overlay status seen by the UI (shared so the Launch page can show the inject indicator). */
export const overlayStatus = writable<OverlayStatus | null>(null)
export const overlayInject = writable<{ ok: boolean; message: string; on_launch: boolean } | null>(null)
export const tuningFocusFile = writable<string | null>(null)
export const schedulerNow = writable<Date | null>(null)
// The server's event clock is UTC + GlobalsPrototype.TimeZone, which the v52 game data sets to -10.
// setSchedulerNow corrects it from the server log.
export const eventTimezoneOffset = writable<number>(-10)


let _uptimeTimer: ReturnType<typeof setInterval> | null = null

export function startUptime() {
  if (_uptimeTimer) return
  uptimeSec.set(0)
  _uptimeTimer = setInterval(() => uptimeSec.update(n => n + 1), 1000)
}

export function stopUptime() {
  if (_uptimeTimer) clearInterval(_uptimeTimer)
  _uptimeTimer = null
  uptimeSec.set(0)
}

// -- Game running poll (persists across tab switches) --
// Previously scoped to LaunchPanel's onMount/onDestroy, so gameRunning only
// updated while that panel was mounted and went stale on any other tab.

let _gameRunningTimer: ReturnType<typeof setInterval> | null = null

async function checkGameRunning() {
  try {
    const running = await invoke<boolean>('game_is_running')
    gameRunning.set(running)
  } catch {}
}

export function startGameRunningPoll() {
  if (_gameRunningTimer) return
  checkGameRunning()
  _gameRunningTimer = setInterval(checkGameRunning, 3000)
}

// -- Shutdown countdown state (persists across tab switches) --

export const shutdownCountdownSec = writable<number>(0)
export const shutdownCountdownActive = writable<boolean>(false)
// True from a Stop until the server exits. Lives here so a remount doesn't re-enable Stop.
export const serverStopping = writable<boolean>(false)

/** Asks the server to shut down now, once. */
export async function stopServerNow() {
  if (get(serverStopping)) return
  serverStopping.set(true)
  clearServerError()
  try {
    await invoke('stop_server')
  } catch (e) {
    setServerError(String(e))
    serverStopping.set(false)
  }
}

let _shutdownCountdownInterval: ReturnType<typeof setInterval> | null = null

export function clearShutdownCountdown() {
  if (_shutdownCountdownInterval) clearInterval(_shutdownCountdownInterval)
  _shutdownCountdownInterval = null
  shutdownCountdownActive.set(false)
  shutdownCountdownSec.set(0)
}

// Auto-cancel if the server stops running for any other reason (crash, manual
// stop elsewhere), even while ServerPanel isn't mounted to observe it itself.
serverRunning.subscribe(running => {
  if (!running) {
    clearShutdownCountdown()
    serverStopping.set(false)
  }
})

export function startShutdownCountdown(delayMinutes: number, broadcastMessage: string) {
  clearShutdownCountdown()

  shutdownCountdownSec.set(delayMinutes * 60)
  shutdownCountdownActive.set(true)

  const initialMsg = broadcastMessage.replace('{minutes}', String(delayMinutes))
  invoke('send_command', { cmd: `!server broadcast ${initialMsg}` }).catch(() => {})

  _shutdownCountdownInterval = setInterval(async () => {
    let remaining = 0
    shutdownCountdownSec.update(s => { remaining = s - 1; return remaining })

    if (remaining === 60) {
      try { await invoke('send_command', { cmd: '!server broadcast Server is shutting down in 1 minute.' }) } catch {}
    }

    if (remaining <= 0) {
      clearShutdownCountdown()
      await stopServerNow()
    }
  }, 1000)
}

export async function cancelShutdownCountdown() {
  clearShutdownCountdown()
  try { await invoke('send_command', { cmd: '!server broadcast Server shutdown has been cancelled.' }) } catch {}
}

// -- Log state (persists across tab switches) --

const SERVER_LOG_FILTER_KEY = 'server-log-filter'

function loadServerLogFilter(): LogLevel {
  if (typeof localStorage === 'undefined') return 'all'
  const value = localStorage.getItem(SERVER_LOG_FILTER_KEY)
  switch (value) {
    case 'ok':
    case 'trace':
    case 'debug':
    case 'info':
    case 'warn':
    case 'err':
    case 'fatal':
    case 'all':
      return value
    default:
      return 'all'
  }
}

export const serverLogFilter = writable<LogLevel>(loadServerLogFilter())

serverLogFilter.subscribe(value => {
  if (typeof localStorage !== 'undefined') {
    localStorage.setItem(SERVER_LOG_FILTER_KEY, value)
  }
})

export interface LogLine {
  id: number
  time: string
  level: LogLevel
  msg: string
}

let _logId = 0

export const serverLog = writable<LogLine[]>([])

export function appendLog(line: Omit<LogLine, 'id'>) {
  serverLog.update(log => [...log.slice(-1999), { ...line, id: _logId++ }])
}

export function appendLogBatch(lines: Omit<LogLine, 'id'>[]) {
  if (!lines.length) return
  serverLog.update(log => {
    const combined = log.concat(lines.map(l => ({ ...l, id: _logId++ })))
    return combined.length > 2000 ? combined.slice(-2000) : combined
  })
}

export function clearLog() {
  serverLog.set([])
}

export function setServerError(message: string) {
  serverError.set(message)
}

export function clearServerError() {
  serverError.set('')
}

export const startingServer = writable<boolean>(false)
export const startingApache = writable<boolean>(false)
// Last start failure per process, shown by the Rail dots; '' when there is none.
export const serverStartError = writable<string>('')
export const apacheStartError = writable<string>('')
serverRunning.subscribe(running => { if (running) { startingServer.set(false); serverStartError.set('') } })
apacheRunning.subscribe(running => { if (running) apacheStartError.set('') })

export async function startServer() {
  startingServer.set(true)
  serverStartError.set('')
  clearServerError()
  try {
    await invoke('start_server', { serverExe: get(appConfig).server_exe })
  } catch (e) {
    setServerError(String(e))
    serverStartError.set(String(e))
    startingServer.set(false)
    return
  }
  // A failed Apache start shows on its Rail dot and doesn't stop the server.
  if (get(appConfig).apache_auto_start && !get(apacheRunning) && !get(startingApache)) await startApache()
}

export async function startApache() {
  startingApache.set(true)
  apacheStartError.set('')
  clearServerError()
  try {
    await invoke('start_apache', { serverExe: get(appConfig).server_exe })
    apacheRunning.set(await invoke<boolean>('apache_is_running'))
  } catch (e) {
    setServerError(String(e))
    apacheStartError.set(String(e))
  } finally {
    startingApache.set(false)
  }
}

export const LOG_LEVEL_SEVERITY: Record<LogLevel, number> = {
  all:   -1,
  trace:  0,
  debug:  1,
  info:   2,
  ok:     3,
  warn:   4,
  err:    5,
  fatal:  6,
}

const LOG_FILTER_THRESHOLD_KEY = 'server-log-filter-threshold'

function loadLogFilterThreshold(): boolean {
  if (typeof localStorage === 'undefined') return true
  return localStorage.getItem(LOG_FILTER_THRESHOLD_KEY) !== 'false'
}

export const logFilterThreshold = writable<boolean>(loadLogFilterThreshold())

logFilterThreshold.subscribe(value => {
  if (typeof localStorage !== 'undefined') {
    localStorage.setItem(LOG_FILTER_THRESHOLD_KEY, String(value))
  }
})

export interface LaunchOptions {
  auto_login: boolean
  patched_client: boolean
  skip_startup_movies: boolean
  skip_motion_comics: boolean
  no_sound: boolean
  enable_client_log: boolean
  no_steam: boolean
  force_32bit: boolean
  custom_resolution: boolean
  resolution_width: number
  resolution_height: number
  extra_args: string
}


export interface ShutdownConfig {
  delay_minutes: number
  broadcast_message: string
  ask_before_close: boolean
}

export interface Server {
  id: string
  name: string
  host: string
  email: string
  is_local?: boolean
  use_https?: boolean
  /** The linked version this profile plays on. Empty or missing follows the active version. */
  linked_id?: string
  /** Local profiles: Launch starts the server first when it isn't running. */
  start_server?: boolean
}

export interface AppConfig {
  game_exe: string
  server_exe: string
  linked: LinkedInstall[]
  active_linked_id: string
  active_server_id: string
  servers: Server[]
  theme: string
  /** Whole-window zoom (1 = 100%). */
  text_size: number
  launch_options: LaunchOptions
  shutdown: ShutdownConfig
  console_presets: string[]
  overlay?: { dir: string; auto_inject?: boolean }
  backup_targets: string[]
  /** Automatic (pre-update and pre-restore) backups to keep. 0 keeps all. */
  keep_auto_backups: number
  /** Data tab view: true = Advanced, false = Simple (fresh installs start in Simple). */
  advanced_data: boolean
  apache_auto_start: boolean
}

/** A client and server pair. The active one is copied into game_exe, server_exe and launch_options.no_steam. */
export interface LinkedInstall {
  id: string
  name: string
  /** '1.48', '1.52', '1.53', or '' when not detected and not picked. */
  version: string
  game_exe: string
  server_exe: string
  custom: boolean
  no_steam: boolean
}

/** Settings edits the active copy, so the active entry follows it (as sync_active_linked does). */
function syncActiveLinked(c: AppConfig): AppConfig {
  return {
    ...c,
    linked: c.linked.map(l => l.id !== c.active_linked_id ? l
      : { ...l, game_exe: c.game_exe, server_exe: c.server_exe, no_steam: c.launch_options.no_steam }),
  }
}

const defaultLaunchOptions: LaunchOptions = {
  auto_login: true,
  patched_client: false,
  skip_startup_movies: false,
  skip_motion_comics: false,
  no_sound: false,
  enable_client_log: false,
  no_steam: true,
  force_32bit: false,
  custom_resolution: false,
  resolution_width: 0,
  resolution_height: 0,
  extra_args: '',
}

const defaultShutdownConfig: ShutdownConfig = {
  delay_minutes: 0,
  broadcast_message: 'Server is shutting down in {minutes} minute(s).',
  ask_before_close: false,
}

export const appConfig = writable<AppConfig>({
  game_exe: '',
  server_exe: '',
  linked: [],
  active_linked_id: '',
  active_server_id: '',
  servers: [],
  theme: '',
  text_size: 1,
  launch_options: defaultLaunchOptions,
  shutdown: defaultShutdownConfig,
  console_presets: ["!commands","!account userlevel","!server status","!server broadcast","!server shutdown","!server reloadcatalog","!server reloadlivetuning"],
  backup_targets: ['ConfigOverride.ini', 'Players', 'Data/Game/LiveTuning'],
  keep_auto_backups: 10,
  // Until the real config loads, show the existing editors rather than flash Simple.
  advanced_data: true,
  apache_auto_start: true,
})

export const activeLinked = derived(appConfig, c => c.linked.find(l => l.id === c.active_linked_id))

/** What the active linked version supports. With none linked, today's 1.52 behaviour. */
export const activeCaps = derived(activeLinked, l => versionCaps(l?.version ?? '', l?.custom ?? false))

export const activeTheme = writable<string>('')

export const activeServerId = writable<string>('')

// The Launch page's Get Started guide, opened by hand. Kept here so a remount on a new server exe keeps it open.
export const guideRequested = writable<boolean>(false)
// Get Started's game data copy. Kept here so closing and reopening the guide can't start a second copy.
export const gameDataCopying = writable<boolean>(false)

// -- Tauri invoke helpers --

function applyTheme(theme: string) {
  if (theme) {
    document.documentElement.setAttribute('data-theme', theme)
  } else {
    document.documentElement.removeAttribute('data-theme')
  }
}

function applyTextSize(size: number) {
  // Zoom scales text and layout together, so nothing wraps that would not at 100%.
  try { getCurrentWebview().setZoom(size).catch(() => {}) } catch { /* outside Tauri (tests) */ }
}

export async function loadConfig(): Promise<void> {
  const config = await invoke<AppConfig>('get_config')
  appConfig.set(config)
  activeTheme.set(config.theme ?? '')
  applyTheme(config.theme ?? '')
  applyTextSize(config.text_size ?? 1)
  if (config.active_server_id) {
    activeServerId.set(config.active_server_id)
  } else if (config.servers.length > 0) {
    activeServerId.set(config.servers[0].id)
  }
  // Not awaited, so callers don't wait on the dialog.
  try {
    Promise.resolve(invoke<string | null>('take_config_notice'))
      .then(notice => {
        if (notice) askConfirm({ title: "Settings file couldn't be read", message: notice, okLabel: 'OK', cancelLabel: 'Close' })
      })
      .catch(() => {})
  } catch {}
}

export async function upsertServer(server: Server, password: string): Promise<void> {
  const isNew = !get(appConfig).servers.some(s => s.id === server.id)
  const updated = await invoke<AppConfig>('upsert_server', { server, password })
  appConfig.set(updated)
  if (isNew) await selectServer(server.id)
}

/** Returns the error, or '' when deleted. */
export async function deleteServer(serverId: string): Promise<string> {
  try {
    const updated = await invoke<AppConfig>('delete_server', { serverId })
    appConfig.set(updated)
    activeServerId.update(id => {
      if (id === serverId) {
        return updated.servers[0]?.id ?? ''
      }
      return id
    })
    return ''
  } catch (e) {
    return String(e)
  }
}

/** The config on disk after a failed save, or `fallback` if that can't be read either. */
async function reloadAfterFailure(fallback: AppConfig): Promise<AppConfig> {
  try {
    return await invoke<AppConfig>('get_config')
  } catch {
    return fallback
  }
}

/** Returns the error, or '' when saved. A failure puts back the saved selection. */
export async function selectServer(serverId: string): Promise<string> {
  const previousId = get(activeServerId)
  const previous = get(appConfig)
  activeServerId.set(serverId)
  try {
    await invoke('set_active_server', { serverId })
    return ''
  } catch (e) {
    const disk = await reloadAfterFailure(previous)
    appConfig.set(disk)
    activeServerId.set(disk.active_server_id || previousId)
    return String(e)
  }
}

/// Optimistically applies `updater` to `appConfig`, then invokes `invokeName`.
/// On failure, sets `appConfig` from the config on disk (not a snapshot, which
/// could undo a later change that did save) and returns the error. '' means saved.
async function updateConfigOptimistic(
  updater: (c: AppConfig) => AppConfig,
  invokeName: string,
  invokeArgs: Record<string, unknown>,
): Promise<string> {
  const previous = get(appConfig)
  appConfig.update(updater)
  try {
    const saved = await invoke<AppConfig | null>(invokeName, invokeArgs)
    if (saved) appConfig.set(saved)
    return ''
  } catch (e) {
    appConfig.set(await reloadAfterFailure(previous))
    return String(e)
  }
}

// Not optimistic: a new server exe remounts the tab, which would drop the caller's error.
// They return the saved config, since new exes detect the active entry's version again.
async function saveExe(command: string, path: string): Promise<string> {
  try {
    appConfig.set(await invoke<AppConfig>(command, { path }))
    return ''
  } catch (e) {
    return String(e)
  }
}

export function setGameExe(path: string): Promise<string> {
  return saveExe('set_game_exe', path)
}

export function setServerExe(path: string): Promise<string> {
  return saveExe('set_server_exe', path)
}

// -- Nightly update (kept here so leaving the Ops tab doesn't lose it) --

export interface UpdateProgress {
  stage: 'backing_up' | 'downloading' | 'extracting' | 'installing' | 'done'
  pct: number
  message?: string
}

export interface UpdateOutcome {
  /** The server it ran against. The outcome only shows for that server. */
  serverExe: string
  error: string
  /** Stock files the update shipped switched the other way, kept as the user had them. */
  kept: { file: string; enabled: boolean }[]
  /** Files saved in the app that the update replaced. Their edited copies are in the backup. */
  edited: string[]
  /** The pre-update backup, for `put_back_edits`. Empty after an error. */
  backup_id: string
}

export const updateRunning = writable<boolean>(false)
export const updateProgress = writable<UpdateProgress | null>(null)
export const updateOutcome = writable<UpdateOutcome | null>(null)

/** Runs a nightly update on `serverExe`. Resolves once it ends, whatever the outcome. */
export async function runServerUpdate(serverExe: string, backupTargets: string[]) {
  if (get(updateRunning)) return
  updateRunning.set(true)
  updateOutcome.set(null)
  updateProgress.set(null)
  const unlisten = await listen<UpdateProgress>('update-progress', e => updateProgress.set(e.payload))
  try {
    const result = await invoke<Pick<UpdateOutcome, 'kept' | 'edited' | 'backup_id'>>('run_update', { serverExe, backupTargets })
    updateOutcome.set({ serverExe, error: '', ...result })
  } catch (e) {
    updateOutcome.set({ serverExe, error: String(e), kept: [], edited: [], backup_id: '' })
  } finally {
    unlisten()
    updateProgress.set(null)
    updateRunning.set(false)
  }
}

let closing = false

/** Answers the window's close button: unsaved edits, a running update and the server come first. */
export async function handleCloseRequest(id: number) {
  invoke('close_ack', { id }).catch(() => {})
  if (closing) return
  closing = true
  let exiting = false
  try {
    if (get(updateRunning)) {
      await askConfirm({ title: 'Update running', message: 'Close MH Multiverse after the update finishes.', okLabel: '', cancelLabel: 'OK' })
      return
    }
    if (!(await confirmLeave())) return
    if (get(serverRunning) && get(appConfig).shutdown.ask_before_close) {
      const ok = await askConfirm({
        title: 'Stop the server?',
        message: 'The server is running. Closing stops it and saves player progress first.',
        okLabel: 'Stop and close',
        cancelLabel: 'Cancel',
      })
      if (!ok) return
    }
    await invoke('exit_app')
    exiting = true
    if (get(serverRunning)) serverStopping.set(true)
  } catch (e) {
    setServerError(String(e))
  } finally {
    if (!exiting) closing = false
  }
}

/** Why the active version can't change right now, or null when it can. */
export const switchBlocked = derived([serverRunning, gameRunning, updateRunning], ([server, game, update]) =>
  update ? 'Wait for the update to finish'
  : server || game ? 'Stop the server and the game first'
  : null)

/** Switching is refused while the server, the game or an update runs. Throws the reason.
 *  Open editors hold the old version's data, so unsaved edits are confirmed first. */
export async function activateLinked(id: string): Promise<void> {
  if (id === get(appConfig).active_linked_id) return
  const blocked = get(switchBlocked)
  if (blocked) throw blocked
  if (!(await confirmLeave())) return
  appConfig.set(await invoke<AppConfig>('activate_linked', { id }))
}

export async function upsertLinked(install: LinkedInstall): Promise<void> {
  appConfig.set(await invoke<AppConfig>('upsert_linked', { install }))
}

export async function deleteLinked(id: string): Promise<void> {
  appConfig.set(await invoke<AppConfig>('delete_linked', { id }))
}

export async function setTheme(theme: string): Promise<string> {
  const previous = get(appConfig)
  activeTheme.set(theme)
  applyTheme(theme)
  appConfig.update(c => ({ ...c, theme }))
  try {
    await invoke('set_theme', { theme })
    return ''
  } catch (e) {
    const disk = await reloadAfterFailure(previous)
    appConfig.set(disk)
    activeTheme.set(disk.theme ?? '')
    applyTheme(disk.theme ?? '')
    return String(e)
  }
}

export async function setTextSize(size: number): Promise<string> {
  applyTextSize(size)
  const error = await updateConfigOptimistic(c => ({ ...c, text_size: size }), 'set_text_size', { size })
  // A failed save reloaded the config, so this puts its zoom back.
  applyTextSize(get(appConfig).text_size)
  return error
}

export function setLaunchOptions(options: LaunchOptions): Promise<string> {
  return updateConfigOptimistic(c => syncActiveLinked({ ...c, launch_options: options }), 'set_launch_options', { options })
}

export function setShutdownConfig(shutdown: ShutdownConfig): Promise<string> {
  return updateConfigOptimistic(c => ({ ...c, shutdown }), 'set_shutdown_config', { shutdown })
}

export function setBackupTargets(targets: string[]): Promise<string> {
  return updateConfigOptimistic(c => ({ ...c, backup_targets: targets }), 'set_backup_targets', { targets })
}

export function setKeepAutoBackups(keep: number): Promise<string> {
  return updateConfigOptimistic(c => ({ ...c, keep_auto_backups: keep }), 'set_keep_auto_backups', { keep })
}

export function setConsolePresets(presets: string[]): Promise<string> {
  return updateConfigOptimistic(c => ({ ...c, console_presets: presets }), 'set_console_presets', { presets })
}

export function setApacheAutoStart(on: boolean): Promise<string> {
  return updateConfigOptimistic(c => ({ ...c, apache_auto_start: on }), 'set_apache_auto_start', { on })
}

export async function setAdvancedData(advanced: boolean): Promise<string> {
  // Switching view unmounts the open editor, so it asks like a tab change.
  if (!(await confirmLeave())) return ''
  return updateConfigOptimistic(c => ({ ...c, advanced_data: advanced }), 'set_advanced_data', { advanced })
}

export function setSchedulerNow(dt: Date) {
  schedulerNow.set(dt)

  const utcNow = new Date()
  const offsetHours = Math.round(
    (dt.getTime() - utcNow.getTime()) / (1000 * 60 * 60)
  )

  eventTimezoneOffset.set(offsetHours)
}

// -- App self-update --

export interface ReleaseNote {
  version: string
  body: string
  published_at: string | null
  html_url: string
  breaking: boolean
}

export interface AppUpdateInfo {
  current: string
  latest: string
  up_to_date: boolean
  html_url: string
  notes: ReleaseNote[]
}

export const appUpdate = writable<AppUpdateInfo | null>(null)
export const appUpdateError = writable<string>('')
export const appUpdateCheckedAt = writable<Date | null>(null)
// Session-only: the badge comes back on next launch.
export const updateRemindLater = writable<boolean>(false)

const DISMISSED_UPDATE_KEY = 'app-update-dismissed'

function loadDismissedUpdate(): string {
  try { return localStorage.getItem(DISMISSED_UPDATE_KEY) ?? '' } catch { return '' }
}

// The version the user chose to ignore; a newer release shows the badge again.
export const dismissedUpdate = writable<string>(loadDismissedUpdate())

dismissedUpdate.subscribe(value => {
  try { localStorage.setItem(DISMISSED_UPDATE_KEY, value) } catch {}
})

export const showUpdateBadge = derived(
  [appUpdate, dismissedUpdate, updateRemindLater],
  ([$u, $dismissed, $later]) => !!$u && !$u.up_to_date && $u.latest !== $dismissed && !$later
)

export async function checkAppUpdate(): Promise<void> {
  try {
    appUpdate.set(await invoke<AppUpdateInfo>('check_app_update'))
    appUpdateError.set('')
  } catch (e) {
    appUpdateError.set(String(e))
  }
  appUpdateCheckedAt.set(new Date())
}
