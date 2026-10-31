import { describe, it, expect } from 'vitest'
import { normalizeLogBatch, extractSchedulerNow, parseServerNowAsUtc } from '../../src/lib/serverEvents'

describe('normalizeLogBatch', () => {
  it('passes known levels through unchanged', () => {
    const out = normalizeLogBatch([
      { time: 't1', level: 'err', msg: 'm1' },
      { time: 't2', level: 'trace', msg: 'm2' },
    ])
    expect(out[0].level).toBe('err')
    expect(out[1].level).toBe('trace')
  })

  it('maps an unrecognised level to info', () => {
    // toLogLevel's switch has no "error" case (the server already sends
    // "err") — an unmapped string falls through to the default, "info".
    const out = normalizeLogBatch([{ time: 't', level: 'unknown', msg: 'm' }])
    expect(out[0].level).toBe('info')
  })

  it('defaults missing time/msg to empty string', () => {
    const out = normalizeLogBatch([{ time: undefined as unknown as string, level: 'info', msg: undefined as unknown as string }])
    expect(out[0].time).toBe('')
    expect(out[0].msg).toBe('')
  })
})

describe('log passwords', () => {
  const cases: [string, string][] = [
    ['!account create a@b.c Player hunter2', '!account create a@b.c Player ****'],
    ['!account password a@b.c hunter2', '!account password a@b.c ****'],
    ['!account verify a@b.c hunter2', '!account verify a@b.c ****'],
    ['[Info] Console: !ACCOUNT  CREATE a@b.c Player hunter2 extra', '[Info] Console: !ACCOUNT  CREATE a@b.c Player **** extra'],
    [
      '[CommandManager] Command invoked: invoker=[ServerConsole], command=[account], parameters=[create me me3 pass]',
      '[CommandManager] Command invoked: invoker=[ServerConsole], command=[account], parameters=[create me me3 ****]',
    ],
    [
      'invoker=[ServerConsole], command=[account], parameters=[password a@b.co pw]x]',
      'invoker=[ServerConsole], command=[account], parameters=[password a@b.co ****]',
    ],
    [
      'invoker=[ServerConsole], command=[account], parameters=[verify a@b.co secret]',
      'invoker=[ServerConsole], command=[account], parameters=[verify a@b.co ****]',
    ],
    [
      'invoker=[ServerConsole], command=[account], parameters=[userlevel a@b.co 2]',
      'invoker=[ServerConsole], command=[account], parameters=[userlevel a@b.co 2]',
    ],
    ['> !account password a@b.c', '> !account password a@b.c'],
    ['!account info', '!account info'],
    ['!account userlevel a@b.c 2', '!account userlevel a@b.c 2'],
    ['Account created: a@b.c', 'Account created: a@b.c'],
  ]
  it.each(cases)('masks %s', (raw, shown) => {
    expect(normalizeLogBatch([{ time: 't', level: 'info', msg: raw }])[0].msg).toBe(shown)
  })
})

describe('parseServerNowAsUtc', () => {
  it('parses the MHServerEmu date/time format as UTC', () => {
    const date = parseServerNowAsUtc('04/10/2026 03:28:24')
    expect(date?.toISOString()).toBe('2026-04-10T03:28:24.000Z')
  })

  it('returns null for a non-matching string', () => {
    expect(parseServerNowAsUtc('not a date')).toBeNull()
  })
})

describe('extractSchedulerNow', () => {
  it('extracts and parses a known timestamp from a log line', () => {
    const date = extractSchedulerNow('Checking Live Tuning events (now=[04/10/2026 03:28:24])')
    expect(date?.toISOString()).toBe('2026-04-10T03:28:24.000Z')
  })

  it('returns null when the line has no match', () => {
    expect(extractSchedulerNow('Some unrelated log line')).toBeNull()
  })
})
