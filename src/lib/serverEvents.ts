import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import {
  appendLog,
  appendLogBatch,
  apacheRunning,
  clearServerError,
  setServerError,
  serverRunning,
  startUptime,
  stopUptime,
  setSchedulerNow,
  type LogLevel,
  type LogLine,
} from './store'

type ServerStoppedPayload = {
  running: boolean
  exit_code: number | null
}

type RawLogEventPayload = {
  time: string
  level: string
  msg: string
}[]

type BridgeState = {
  initialized: boolean
  unlistenLog: UnlistenFn | null
  unlistenStarted: UnlistenFn | null
  unlistenStopped: UnlistenFn | null
}

declare global {
  interface Window {
    __mhmServerBridge?: BridgeState
  }
}

function getBridgeState(): BridgeState {
  if (!window.__mhmServerBridge) {
    window.__mhmServerBridge = {
      initialized: false,
      unlistenLog: null,
      unlistenStarted: null,
      unlistenStopped: null,
    }
  }
  return window.__mhmServerBridge
}

function toLogLevel(level: string): LogLevel {
  switch (level) {
    case 'trace':
    case 'debug':
    case 'info':
    case 'ok':
    case 'warn':
    case 'err':
    case 'fatal':
      return level
    default:
      return 'info'
  }
}

function normalizeLogBatch(lines: RawLogEventPayload): Omit<LogLine, 'id'>[] {
  return lines.map((line) => ({
    time: line.time ?? '',
    level: toLogLevel(line.level),
    msg: line.msg ?? '',
  }))
}

const NOW_REGEX = /Checking Live Tuning events \(now=\[(.*?)\]\)/

function extractSchedulerNow(msg: string): Date | null {
  const match = msg.match(NOW_REGEX)
  if (!match) return null

  const parsed = new Date(match[1])
  return Number.isNaN(parsed.getTime()) ? null : parsed
}

async function syncInitialState() {
  const running = await invoke<boolean>('server_is_running')
  serverRunning.set(running)

  if (running) {
    startUptime()
    const apache = await invoke<boolean>('apache_is_running')
    apacheRunning.set(apache)
    clearServerError()
  } else {
    apacheRunning.set(false)
    stopUptime()
  }
}

export async function initServerEventBridge(): Promise<void> {
  const bridge = getBridgeState()
  if (bridge.initialized) return
  bridge.initialized = true

  await syncInitialState()

  bridge.unlistenLog = await listen<RawLogEventPayload>('server-log', (event) => {
    const batch = normalizeLogBatch(event.payload)

    for (const line of batch) {
      const dt = extractSchedulerNow(line.msg)
      if (dt) {
        setSchedulerNow(dt)
      }
    }

    appendLogBatch(batch)
  })

  bridge.unlistenStarted = await listen('server-started', async () => {
    serverRunning.set(true)
    clearServerError()
    startUptime()
    appendLog({ time: '', level: 'ok', msg: '-- Server started --' })

    try {
      const apache = await invoke<boolean>('apache_is_running')
      apacheRunning.set(apache)
    } catch {
      apacheRunning.set(false)
    }
  })

  bridge.unlistenStopped = await listen<ServerStoppedPayload>('server-stopped', (event) => {
    serverRunning.set(false)
    apacheRunning.set(false)
    stopUptime()

    if (event.payload.exit_code !== null && event.payload.exit_code !== 0) {
      const message = `Server exited unexpectedly (code ${event.payload.exit_code})`
      setServerError(message)
      appendLog({ time: '', level: 'err', msg: `-- ${message} --` })
    } else {
      clearServerError()
      appendLog({ time: '', level: 'info', msg: '-- Server stopped --' })
    }
  })
}