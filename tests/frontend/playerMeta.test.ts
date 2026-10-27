import { describe, it, expect } from 'vitest'
import { isBanned, isWhitelisted, userLevelLabel, formatLastSeen } from '../../src/lib/playerMeta'

describe('isBanned', () => {
  it('flag value 2', () => expect(isBanned(2)).toBe(true))
  it('flag value 0', () => expect(isBanned(0)).toBe(false))
  it('combined flags (banned + whitelisted)', () => expect(isBanned(18)).toBe(true))
  it('undefined', () => expect(isBanned(undefined)).toBe(false))
})

describe('isWhitelisted', () => {
  it('flag value 16', () => expect(isWhitelisted(16)).toBe(true))
  it('combined flags', () => expect(isWhitelisted(18)).toBe(true))
  it('undefined', () => expect(isWhitelisted(undefined)).toBe(false))
})

describe('userLevelLabel', () => {
  it('known levels', () => {
    expect(userLevelLabel(0)).toBe('Player')
    expect(userLevelLabel(1)).toBe('Moderator')
    expect(userLevelLabel(2)).toBe('Admin')
  })
  it('undefined defaults to Player', () => expect(userLevelLabel(undefined)).toBe('Player'))
})

describe('formatLastSeen', () => {
  it('zero is Never', () => expect(formatLastSeen(0)).toBe('Never'))
  it('undefined is Never', () => expect(formatLastSeen(undefined)).toBe('Never'))

  it('.NET ticks, Unix ms, and Unix seconds for the same instant all agree', () => {
    // timestampToDate() isn't exported, so this pins its three unit
    // classifications indirectly: encode one instant three ways and check
    // they render identically, rather than asserting a locale-dependent string.
    const unixMs = 1700000000000 // 2023-11-14T22:13:20.000Z
    const unixSeconds = unixMs / 1000
    const dotnetTicks = (unixMs + 62135596800000) * 10000

    const fromSeconds = formatLastSeen(unixSeconds)
    const fromMs = formatLastSeen(unixMs)
    const fromTicks = formatLastSeen(dotnetTicks)

    expect(fromSeconds).not.toBe('Invalid date')
    expect(fromSeconds).toBe(fromMs)
    expect(fromMs).toBe(fromTicks)
  })
})
