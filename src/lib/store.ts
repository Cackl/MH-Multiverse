import { writable } from 'svelte/store'
import { invoke } from '@tauri-apps/api/core'

export type Tab = 'launch' | 'server' | 'events' | 'config' | 'ops' | 'settings'

export const activeTab = writable<Tab>('launch')
export const serverRunning = writable<boolean>(false)
export const gameRunning = writable<boolean>(false)
export const apacheRunning = writable<boolean>(false)
export const uptimeSec = writable<number>(0)

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

export type LogLevel = 'trace' | 'debug' | 'info' | 'ok' | 'warn' | 'err' | 'fatal'

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

export interface LaunchOptions {
  auto_login: boolean
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

export interface Server {
  id: string
  name: string
  host: string
  email: string
}

export interface AppConfig {
  game_exe: string
  server_exe: string
  active_server_id: string
  servers: Server[]
  theme: string
  launch_options: LaunchOptions
}

const defaultLaunchOptions: LaunchOptions = {
  auto_login: true,
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

export const appConfig = writable<AppConfig>({
  game_exe: '',
  server_exe: '',
  active_server_id: '',
  servers: [],
  theme: '',
  launch_options: defaultLaunchOptions,
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