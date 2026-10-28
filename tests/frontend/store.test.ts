import { describe, it, expect, beforeEach, vi } from 'vitest'
import { get } from 'svelte/store'

const invokeMock = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))

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
