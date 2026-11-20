import { describe, it, expect, beforeEach, vi } from 'vitest'
import { get } from 'svelte/store'

const invokeMock = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))
const zoomMock = vi.hoisted(() => vi.fn(() => Promise.resolve()))
vi.mock('@tauri-apps/api/webview', () => ({ getCurrentWebview: () => ({ setZoom: zoomMock }) }))

import {
  appendLog,
  appendLogBatch,
  clearLog,
  serverLog,
  setSchedulerNow,
  eventTimezoneOffset,
  unsavedSources,
  setUnsaved,
  confirmLeave,
  goTab,
  goDataTab,
  activeTab,
  activeDataTab,
  confirmRequest,
  appConfig,
  activeServerId,
  upsertServer,
  setAdvancedData,
  setTextSize,
  setTheme,
  setBackupTargets,
  setConsolePresets,
  selectServer,
  deleteServer,
  setServerExe,
  activeTheme,
  activateLinked,
  switchBlocked,
  updateRunning,
  serverRunning,
  eventsRestartPending,
  markEventsSaved,
  gameRunning,
  type AppConfig,
  type Server,
} from '../../src/lib/store'

beforeEach(() => {
  clearLog()
  vi.useRealTimers()
})

describe('appendLog', () => {
  it('adds an auto-incrementing id', () => {
    appendLog({ time: '', level: 'info', msg: 'a' })
    appendLog({ time: '', level: 'info', msg: 'b' })
    const lines = get(serverLog)
    expect(lines.length).toBe(2)
    expect(lines[1].id).toBeGreaterThan(lines[0].id)
  })
})

describe('appendLogBatch', () => {
  it('caps stored lines at 2000', () => {
    const batch = Array.from({ length: 2500 }, (_, i) => ({ time: '', level: 'info' as const, msg: `line ${i}` }))
    appendLogBatch(batch)
    expect(get(serverLog).length).toBe(2000)
  })

})

describe('setSchedulerNow', () => {
  it('computes the timezone offset against the current time', () => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-01-01T00:00:00.000Z'))

    setSchedulerNow(new Date('2026-01-01T05:00:00.000Z'))

    expect(get(eventTimezoneOffset)).toBe(5)
    vi.useRealTimers()
  })
})

describe('unsaved-changes guard', () => {
  // Answers the in-app confirmation currently showing; returns its message.
  function answerNext(ok: boolean): string | null {
    const req = get(confirmRequest)
    if (!req) return null
    req.resolve(ok)
    return req.message
  }
  const asked = () => get(confirmRequest) !== null

  beforeEach(() => {
    unsavedSources.set({})
    confirmRequest.set(null)
    activeTab.set('launch')
    activeDataTab.set('events')
  })

  it('setUnsaved adds and removes a source', () => {
    setUnsaved('patches', 'Patches')
    expect(get(unsavedSources)).toEqual({ patches: 'Patches' })
    setUnsaved('patches', null)
    expect(get(unsavedSources)).toEqual({})
  })

  it('confirmLeave is true without asking when nothing is unsaved', async () => {
    expect(await confirmLeave()).toBe(true)
    expect(asked()).toBe(false)
  })

  it('confirmLeave asks, names the sources and clears them on accept', async () => {
    setUnsaved('patches', 'Patches')
    setUnsaved('config', 'Config')
    const result = confirmLeave()
    expect(answerNext(true)).toContain('Patches, Config')
    expect(await result).toBe(true)
    expect(get(unsavedSources)).toEqual({})
    expect(asked()).toBe(false)
  })

  it('confirmLeave keeps the sources when the user stays', async () => {
    setUnsaved('patches', 'Patches')
    const result = confirmLeave()
    answerNext(false)
    expect(await result).toBe(false)
    expect(get(unsavedSources)).toEqual({ patches: 'Patches' })
  })

  it('a newer question cancels an unanswered one', async () => {
    setUnsaved('patches', 'Patches')
    const first = confirmLeave()
    const second = confirmLeave()
    expect(await first).toBe(false)
    answerNext(true)
    expect(await second).toBe(true)
  })

  it('switching Simple/Advanced with unsaved edits asks, and staying keeps the view', async () => {
    invokeMock.mockReset().mockResolvedValue(undefined)
    setUnsaved('tuning', 'Live Tuning')
    const stay = setAdvancedData(false)
    expect(answerNext(false)).toContain('Live Tuning')
    await stay
    expect(invokeMock).not.toHaveBeenCalled()

    const leave = setAdvancedData(false)
    answerNext(true)
    await leave
    expect(invokeMock).toHaveBeenCalledWith('set_advanced_data', { advanced: false })
  })

  it('switching Simple/Advanced with nothing unsaved never asks', async () => {
    invokeMock.mockReset().mockResolvedValue(undefined)
    await setAdvancedData(true)
    expect(asked()).toBe(false)
    expect(invokeMock).toHaveBeenCalledWith('set_advanced_data', { advanced: true })
  })

  it('goTab to the current tab never asks', async () => {
    setUnsaved('patches', 'Patches')
    await goTab('launch')
    expect(asked()).toBe(false)
  })

  it('goTab stays put when the user declines', async () => {
    setUnsaved('config', 'Config')
    const nav = goTab('server')
    answerNext(false)
    await nav
    expect(get(activeTab)).toBe('launch')
  })

  it('goTab moves when nothing is unsaved', async () => {
    await goTab('server')
    expect(get(activeTab)).toBe('server')
  })

  it('goDataTab to the open sub-tab never asks, other sub-tabs do', async () => {
    activeTab.set('data')
    setUnsaved('patches', 'Patches')
    await goDataTab('events')
    expect(asked()).toBe(false)
    const nav = goDataTab('store')
    expect(asked()).toBe(true)
    answerNext(false)
    await nav
    expect(get(activeDataTab)).toBe('events')
  })

  it('switching version with unsaved edits asks, and staying keeps the version', async () => {
    invokeMock.mockReset().mockResolvedValue({ ...get(appConfig), active_linked_id: 'b' })
    appConfig.update(c => ({ ...c, active_linked_id: 'a' }))
    setUnsaved('events', 'Events')
    const stay = activateLinked('b')
    expect(answerNext(false)).toContain('Events')
    await stay
    expect(invokeMock).not.toHaveBeenCalled()
    expect(get(appConfig).active_linked_id).toBe('a')

    const leave = activateLinked('b')
    answerNext(true)
    await leave
    expect(invokeMock).toHaveBeenCalledWith('activate_linked', { id: 'b' })
    expect(get(appConfig).active_linked_id).toBe('b')
  })

  it('a switch while the server, the game or an update runs is refused before asking', async () => {
    invokeMock.mockReset()
    appConfig.update(c => ({ ...c, active_linked_id: 'a' }))
    setUnsaved('events', 'Events')
    for (const [flag, reason] of [
      [updateRunning, 'Wait for the update to finish'],
      [serverRunning, 'Stop the server and the game first'],
      [gameRunning, 'Stop the server and the game first'],
    ] as const) {
      flag.set(true)
      expect(get(switchBlocked)).toBe(reason)
      await expect(activateLinked('b')).rejects.toBe(reason)
      expect(asked()).toBe(false)
      flag.set(false)
    }
    expect(get(switchBlocked)).toBeNull()
    expect(invokeMock).not.toHaveBeenCalled()
    expect(get(unsavedSources)).toEqual({ events: 'Events' })
  })

  it('picking the active version again never asks', async () => {
    invokeMock.mockReset()
    appConfig.update(c => ({ ...c, active_linked_id: 'a' }))
    setUnsaved('events', 'Events')
    await activateLinked('a')
    expect(asked()).toBe(false)
    expect(invokeMock).not.toHaveBeenCalled()
  })
})

describe('upsertServer selection', () => {
  const srv = (id: string): Server => ({ id, name: id, host: '', email: '', is_local: true, use_https: false })
  const withServers = (...ids: string[]): AppConfig => ({ ...get(appConfig), servers: ids.map(srv) })

  beforeEach(() => {
    invokeMock.mockReset()
    appConfig.set(withServers('a', 'b'))
    activeServerId.set('a')
  })

  // upsert_server returns the updated config; set_active_server returns nothing.
  function backendReturns(servers: AppConfig) {
    invokeMock.mockImplementation(async (cmd: string) => (cmd === 'upsert_server' ? servers : undefined))
  }

  it.each([
    ['a new server is selected', 'c', 'c'],
    ['an edited server keeps the current selection', 'b', 'a'],
  ])('%s', async (_name, id, expected) => {
    backendReturns(withServers('a', 'b', 'c'))
    await upsertServer(srv(id), 'pw')
    expect(get(activeServerId)).toBe(expected)
  })

  it.each([
    ['persists the selection of a new server', 'c', true],
    ['does not persist anything for an edit', 'b', false],
  ])('%s', async (_name, id, persisted) => {
    backendReturns(withServers('a', 'b', 'c'))
    await upsertServer(srv(id), 'pw')
    const calls = invokeMock.mock.calls.filter(([cmd]) => cmd === 'set_active_server')
    expect(calls).toEqual(persisted ? [['set_active_server', { serverId: id }]] : [])
  })
})

describe('setTextSize', () => {
  it('zooms the window and saves the size', async () => {
    invokeMock.mockReset().mockResolvedValue(undefined)
    zoomMock.mockClear()
    await setTextSize(1.25)
    expect(invokeMock).toHaveBeenCalledWith('set_text_size', { size: 1.25 })
    expect(get(appConfig).text_size).toBe(1.25)
    expect(zoomMock).toHaveBeenLastCalledWith(1.25)
  })

  it('puts the previous zoom back when the save fails', async () => {
    invokeMock.mockReset().mockResolvedValue(undefined)
    await setTextSize(1)
    invokeMock.mockReset().mockRejectedValue('disk full')
    zoomMock.mockClear()
    await setTextSize(1.1)
    expect(get(appConfig).text_size).toBe(1)
    expect(zoomMock).toHaveBeenLastCalledWith(1)
  })
})

describe('failed settings saves', () => {
  it('leaves the server exe alone until the save succeeds, so a refusal does not remount the tab', async () => {
    appConfig.set({ ...get(appConfig), server_exe: 'old.exe' })
    let seen = ''
    invokeMock.mockReset().mockImplementation(async () => {
      seen = get(appConfig).server_exe
      throw 'Stop the server and the game first'
    })
    expect(await setServerExe('new.exe')).toBe('Stop the server and the game first')
    expect(seen).toBe('old.exe')
    expect(get(appConfig).server_exe).toBe('old.exe')
  })

  it('returns the error and shows the config on disk, not the snapshot', async () => {
    vi.stubGlobal('document', { documentElement: { setAttribute() {}, removeAttribute() {} } })
    invokeMock.mockReset().mockResolvedValue(undefined)
    await setTheme('old')
    const disk = { ...get(appConfig), theme: 'disk' }
    invokeMock.mockReset().mockImplementation(async (cmd: string) => {
      if (cmd === 'get_config') return disk
      throw 'disk full'
    })
    expect(await setTheme('new')).toBe('disk full')
    expect(get(appConfig).theme).toBe('disk')
    expect(get(activeTheme)).toBe('disk')
    vi.unstubAllGlobals()
  })

  it('keeps a later change that saved when an earlier one fails', async () => {
    invokeMock.mockReset()
    appConfig.set({ ...get(appConfig), backup_targets: ['old'], console_presets: ['old'] })
    let rejectTargets: (e: unknown) => void = () => {}
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'set_backup_targets') return new Promise((_, reject) => { rejectTargets = reject })
      if (cmd === 'get_config') return Promise.resolve({ ...get(appConfig), backup_targets: ['old'], console_presets: ['B'] })
      return Promise.resolve(undefined)
    })
    const targets = setBackupTargets(['A'])
    expect(await setConsolePresets(['B'])).toBe('')
    rejectTargets('locked')
    expect(await targets).toBe('locked')
    expect(get(appConfig).console_presets).toEqual(['B'])
    expect(get(appConfig).backup_targets).toEqual(['old'])
  })

  it('selectServer puts the saved selection back and returns the error', async () => {
    invokeMock.mockReset().mockImplementation(async (cmd: string) => {
      if (cmd === 'get_config') return { ...get(appConfig), active_server_id: 'a' }
      throw 'no write'
    })
    activeServerId.set('a')
    expect(await selectServer('b')).toBe('no write')
    expect(get(activeServerId)).toBe('a')
  })

  it('deleteServer returns the error instead of throwing', async () => {
    invokeMock.mockReset().mockRejectedValue('no write')
    expect(await deleteServer('a')).toBe('no write')
  })
})

describe('eventsRestartPending', () => {
  it('stays until the server starts after the save', () => {
    serverRunning.set(true)
    markEventsSaved()
    expect(get(eventsRestartPending)).toBe(true)
    serverRunning.set(true)
    expect(get(eventsRestartPending)).toBe(true)
    serverRunning.set(false)
    expect(get(eventsRestartPending)).toBe(true)
    serverRunning.set(true)
    expect(get(eventsRestartPending)).toBe(false)
  })
  it('clears on the next start when saved while stopped', () => {
    serverRunning.set(false)
    markEventsSaved()
    expect(get(eventsRestartPending)).toBe(true)
    serverRunning.set(true)
    expect(get(eventsRestartPending)).toBe(false)
  })
})
