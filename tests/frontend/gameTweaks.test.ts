import { describe, it, expect } from 'vitest'
import { clampCooldown, readCooldowns, withCooldowns, COOLDOWN_DEFAULT } from '../../src/lib/gameTweaks'
import type { PatchEntry } from '../../src/lib/patchesMeta'

const heroes = ['Entity/Characters/Avatars/Shipping/Angela.prototype', 'Entity/Characters/Avatars/Shipping/Blade.prototype']
const mine: PatchEntry = { Enabled: true, Prototype: 'Loot/X.prototype', Path: 'Weight', Description: 'My own change', ValueType: 'Integer', Value: 5 }
// Above 2^53, so a Number would round it.
const BIG = '17436925256515063008'

describe('withCooldowns', () => {
  it('writes -0.99 at 99%', () => {
    expect(withCooldowns([], heroes, { all: 99, powers: [] })[0].Value).toBe('{"CooldownModifierPctGlobal":-0.99}')
  })

  it('writes one Properties entry per hero', () => {
    const out = withCooldowns([], heroes, { all: 25, powers: [] })
    expect(out).toEqual(heroes.map(Prototype => ({
      Enabled: true,
      Prototype,
      Path: 'Properties',
      Description: '[MV:cooldowns all=25] All cooldowns -25%',
      ValueType: 'Properties',
      Value: '{"CooldownModifierPctGlobal":-0.25}',
    })))
  })

  it('repeats the per-power key with exact ids in the same entry', () => {
    const out = withCooldowns([], heroes, { all: 25, powers: [{ id: BIG, pct: 50 }, { id: '42', pct: 10 }] })
    expect(out).toHaveLength(2)
    expect(out[0].Value).toBe(`{"CooldownModifierPctGlobal":-0.25,"CooldownModifierPctForPower":[${BIG},-0.5],"CooldownModifierPctForPower":[42,-0.1]}`)
    expect(out[0].Description).toBe(`[MV:cooldowns all=25 powers=${BIG}:50,42:10] All cooldowns -25%, 2 power cooldowns`)
  })

  it('writes per-power reductions without a global one', () => {
    const out = withCooldowns([], heroes, { all: null, powers: [{ id: '42', pct: 30 }] })
    expect(out[0].Value).toBe('{"CooldownModifierPctForPower":[42,-0.3]}')
    expect(out[0].Description).toBe('[MV:cooldowns powers=42:30] 1 power cooldown')
  })

  it('cuts a power so its total stays at 99%', () => {
    const out = withCooldowns([], heroes, { all: 90, powers: [{ id: '42', pct: 50 }] })
    expect(out[0].Value).toBe('{"CooldownModifierPctGlobal":-0.9,"CooldownModifierPctForPower":[42,-0.09]}')
    // At 99 globally there is nothing left for the power.
    expect(withCooldowns([], heroes, { all: 99, powers: [{ id: '42', pct: 50 }] })[0].Value).toBe('{"CooldownModifierPctGlobal":-0.99}')
  })

  it('replaces its own entries and keeps everything else', () => {
    const once = withCooldowns([mine], heroes, { all: 25, powers: [] })
    const again = withCooldowns(once, heroes, { all: 40, powers: [] })
    expect(again).toHaveLength(3)
    expect(again[0]).toBe(mine)
    expect(again[1].Value).toBe('{"CooldownModifierPctGlobal":-0.4}')
  })

  it('replaces entries with the older tag', () => {
    const old = { ...mine, Prototype: heroes[0], Description: '[MV:cooldowns:all pct=25] All cooldowns -25%' }
    expect(withCooldowns([mine, old], heroes, { all: null, powers: [] })).toEqual([mine])
  })

  it('removes its entries when nothing is set', () => {
    expect(withCooldowns(withCooldowns([mine], heroes, { all: 25, powers: [] }), heroes, { all: null, powers: [] })).toEqual([mine])
  })
})

describe('readCooldowns', () => {
  it('reads back what withCooldowns wrote', () => {
    const c = { all: 35, powers: [{ id: BIG, pct: 50 }, { id: '42', pct: 10 }] }
    expect(readCooldowns(withCooldowns([mine], heroes, c))).toEqual(c)
    const powersOnly = { all: null, powers: [{ id: '42', pct: 30 }] }
    expect(readCooldowns(withCooldowns([mine], heroes, powersOnly))).toEqual(powersOnly)
  })

  it('reads the older tag', () => {
    expect(readCooldowns([{ ...mine, Description: '[MV:cooldowns:all pct=25] All cooldowns -25%' }])).toEqual({ all: 25, powers: [] })
  })

  it('is empty without a cooldown entry', () => {
    expect(readCooldowns([mine])).toEqual({ all: null, powers: [] })
    expect(readCooldowns([])).toEqual({ all: null, powers: [] })
  })
})

describe('clampCooldown', () => {
  it('rounds to the step and stays in range', () => {
    expect(clampCooldown(23.4)).toBe(23)
    expect(clampCooldown(1)).toBe(5)
    expect(clampCooldown(100)).toBe(99)
    expect(clampCooldown(150)).toBe(99)
    expect(clampCooldown(NaN)).toBe(COOLDOWN_DEFAULT)
  })
})
