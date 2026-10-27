import { describe, it, expect } from 'vitest'
import { isBanned, isWhitelisted, userLevelLabel, formatLastSeen } from '../../src/lib/playerMeta'

describe('account flags', () => {
  it('isBanned reads bit 2, also when combined with other flags', () => {
    expect(isBanned(2)).toBe(true)
    expect(isBanned(18)).toBe(true)
    expect(isBanned(16)).toBe(false)
    expect(isBanned(undefined)).toBe(false)
  })
  it('isWhitelisted reads bit 16, also when combined with other flags', () => {
    expect(isWhitelisted(16)).toBe(true)
    expect(isWhitelisted(18)).toBe(true)
    expect(isWhitelisted(2)).toBe(false)
    expect(isWhitelisted(undefined)).toBe(false)
  })
})

describe('userLevelLabel', () => {
  it('missing level is a Player, an unlisted level is Unknown', () => {
    expect(userLevelLabel(undefined)).toBe('Player')
    expect(userLevelLabel(99)).toBe('Unknown')
  })
})

describe('formatLastSeen', () => {
  it('zero and missing timestamps read Never', () => {
    expect(formatLastSeen(0)).toBe('Never')
    expect(formatLastSeen(undefined)).toBe('Never')
  })
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
