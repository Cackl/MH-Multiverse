import { writable } from 'svelte/store'
import { invoke } from '@tauri-apps/api/core'

export type DataTab = 'events' | 'tuning' | 'store' | 'patches'
export type Tab = 'launch' | 'server' | 'config' | 'data' | 'ops' | 'settings'
export type LogLevel = 'all' | 'trace' | 'debug' | 'info' | 'ok' | 'warn' | 'err' | 'fatal'

export const activeTab = writable<Tab>('launch')
export const activeDataTab = writable<DataTab>('events')
export const serverRunning = writable<boolean>(false)
export const gameRunning = writable<boolean>(false)
export const apacheRunning = writable<boolean>(false)
export const uptimeSec = writable<number>(0)
export const serverError = writable<string>('')
export const tuningFocusFile = writable<string | null>(null)
export const schedulerNow = writable<Date | null>(null)
export const eventTimezoneOffset = writable<number>(0)


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
  robocopy: boolean
  no_steam: boolean
  custom_resolution: boolean
  resolution_width: number
  resolution_height: number
}


export interface ShutdownConfig {
  delay_minutes: number
  broadcast_message: string
}

export interface Server {
  id: string
  name: string
  host: string
  email: string
  is_local?: boolean
  use_https?: boolean
}

export interface AppConfig {
  game_exe: string
  server_exe: string
  active_server_id: string
  servers: Server[]
  theme: string
  launch_options: LaunchOptions
  shutdown: ShutdownConfig
  console_presets: string[]
  tuning_tags: Record<string, string>
  tuning_favourites: string[]
  backup_targets: string[]
  store_html_output_dir: string
}

const defaultLaunchOptions: LaunchOptions = {
  auto_login: true,
  patched_client: false,
  skip_startup_movies: false,
  skip_motion_comics: false,
  no_sound: false,
  enable_client_log: false,
  robocopy: true,
  no_steam: true,
  custom_resolution: false,
  resolution_width: 0,
  resolution_height: 0,
}

const defaultShutdownConfig: ShutdownConfig = {
  delay_minutes: 0,
  broadcast_message: 'Server is shutting down in {minutes} minute(s).',
}

export const appConfig = writable<AppConfig>({
  game_exe: '',
  server_exe: '',
  active_server_id: '',
  servers: [],
  theme: '',
  launch_options: defaultLaunchOptions,
  shutdown: defaultShutdownConfig,
  console_presets: ["!commands","!account userlevel","!server status","!server broadcast","!server shutdown","!server reloadcatalog","!server reloadlivetuning"],
  tuning_tags: {},
  tuning_favourites: [],
  backup_targets: ['Config.ini', 'ConfigOverride.ini', 'Data/Game/LiveTuning', 'Data/Account.db'],
  store_html_output_dir: ''
})

export const activeTheme = writable<string>('')

export const activeServerId = writable<string>('')

// -- Tauri invoke helpers --

function applyTheme(theme: string) {
  if (theme) {
    document.documentElement.setAttribute('data-theme', theme)
  } else {
    document.documentElement.removeAttribute('data-theme')
  }
}

export async function loadConfig(): Promise<void> {
  const config = await invoke<AppConfig>('get_config')
  appConfig.set(config)
  activeTheme.set(config.theme ?? '')
  applyTheme(config.theme ?? '')
  if (config.active_server_id) {
    activeServerId.set(config.active_server_id)
  } else if (config.servers.length > 0) {
    activeServerId.set(config.servers[0].id)
  }
}

export async function upsertServer(server: Server, password: string): Promise<void> {
  const updated = await invoke<AppConfig>('upsert_server', { server, password })
  appConfig.set(updated)
}

export async function deleteServer(serverId: string): Promise<void> {
  const updated = await invoke<AppConfig>('delete_server', { serverId })
  appConfig.set(updated)
  activeServerId.update(id => {
    if (id === serverId) {
      return updated.servers[0]?.id ?? ''
    }
    return id
  })
}

export async function selectServer(serverId: string): Promise<void> {
  activeServerId.set(serverId)
  await invoke('set_active_server', { serverId })
}

export async function setGameExe(path: string): Promise<void> {
  appConfig.update(c => ({ ...c, game_exe: path }))
  await invoke('set_game_exe', { path })
}

export async function setServerExe(path: string): Promise<void> {
  appConfig.update(c => ({ ...c, server_exe: path }))
  await invoke('set_server_exe', { path })
}

export async function setTheme(theme: string): Promise<void> {
  activeTheme.set(theme)
  applyTheme(theme)
  appConfig.update(c => ({ ...c, theme }))
  await invoke('set_theme', { theme })
}

export async function setLaunchOptions(options: LaunchOptions): Promise<void> {
  appConfig.update(c => ({ ...c, launch_options: options }))
  await invoke('set_launch_options', { options })
}

export async function setShutdownConfig(shutdown: ShutdownConfig): Promise<void> {
  appConfig.update(c => ({ ...c, shutdown }))
  await invoke('set_shutdown_config', { shutdown })
}

export async function setTuningTags(tags: Record<string, string>): Promise<void> {
  appConfig.update(c => ({ ...c, tuning_tags: tags }))
  await invoke('set_tuning_tags', { tags })
}

export async function setTuningFavourites(favourites: string[]): Promise<void> {
  appConfig.update(c => ({ ...c, tuning_favourites: favourites }))
  await invoke('set_tuning_favourites', { favourites })
}

export async function setBackupTargets(targets: string[]): Promise<void> {
  appConfig.update(c => ({ ...c, backup_targets: targets }))
  await invoke('set_backup_targets', { targets })
}

export async function setStoreHtmlOutputDir(dir: string): Promise<void> {
  appConfig.update(c => ({ ...c, store_html_output_dir: dir }))
  await invoke('set_store_html_output_dir', { dir })
}

export async function setConsolePresets(presets: string[]): Promise<void> {
  appConfig.update(c => ({ ...c, console_presets: presets }))
  await invoke('set_console_presets', { presets })
}

export function setSchedulerNow(dt: Date) {
  schedulerNow.set(dt)

  const utcNow = new Date()
  const offsetHours = Math.round(
    (dt.getTime() - utcNow.getTime()) / (1000 * 60 * 60)
  )

  eventTimezoneOffset.set(offsetHours)
}