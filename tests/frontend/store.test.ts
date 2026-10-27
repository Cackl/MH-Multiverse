import { describe, it, expect, beforeEach, vi } from 'vitest'
import { get } from 'svelte/store'
import {
  appendLog,
  appendLogBatch,
  clearLog,
  serverLog,
  LOG_LEVEL_SEVERITY,
  setSchedulerNow,
  eventTimezoneOffset,
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

  it('no-ops on an empty batch', () => {
    appendLog({ time: '', level: 'info', msg: 'kept' })
    appendLogBatch([])
    expect(get(serverLog).length).toBe(1)
  })
})

describe('clearLog', () => {
  it('empties the log', () => {
    appendLog({ time: '', level: 'info', msg: 'a' })
    clearLog()
    expect(get(serverLog)).toEqual([])
  })
})

describe('LOG_LEVEL_SEVERITY', () => {
  it('orders levels trace < debug < info < ok < warn < err < fatal', () => {
    const order: (keyof typeof LOG_LEVEL_SEVERITY)[] = ['trace', 'debug', 'info', 'ok', 'warn', 'err', 'fatal']
    for (let i = 1; i < order.length; i++) {
      expect(LOG_LEVEL_SEVERITY[order[i]]).toBeGreaterThan(LOG_LEVEL_SEVERITY[order[i - 1]])
    }
  })

  it('"all" sorts below every real level', () => {
    expect(LOG_LEVEL_SEVERITY.all).toBeLessThan(LOG_LEVEL_SEVERITY.trace)
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
