import { describe, it, expect } from 'vitest'
import { clampCooldown, cooldownPct, withCooldown, COOLDOWN_DEFAULT } from '../../src/lib/gameTweaks'
import type { PatchEntry } from '../../src/lib/patchesMeta'

const heroes = ['Entity/Characters/Avatars/Shipping/Angela.prototype', 'Entity/Characters/Avatars/Shipping/Blade.prototype']
const mine: PatchEntry = { Enabled: true, Prototype: 'Loot/X.prototype', Path: 'Weight', Description: 'My own change', ValueType: 'Integer', Value: 5 }

describe('withCooldown', () => {
  it('writes -0.99 at 99%', () => {
    expect(withCooldown([], heroes, 99)[0].Value).toBe('{"CooldownModifierPctGlobal":-0.99}')
  })

  it('writes one Properties entry per hero', () => {
    const out = withCooldown([], heroes, 25)
    expect(out).toEqual(heroes.map(Prototype => ({
      Enabled: true,
      Prototype,
      Path: 'Properties',
      Description: '[MV:cooldowns:all pct=25] All cooldowns -25%',
      ValueType: 'Properties',
      Value: '{"CooldownModifierPctGlobal":-0.25}',
    })))
  })

  it('replaces its own entries and keeps everything else', () => {
    const once = withCooldown([mine], heroes, 25)
    const again = withCooldown(once, heroes, 40)
    expect(again).toHaveLength(3)
    expect(again[0]).toBe(mine)
    expect(again[1].Value).toBe('{"CooldownModifierPctGlobal":-0.4}')
  })

  it('removes its entries for null', () => {
    expect(withCooldown(withCooldown([mine], heroes, 25), heroes, null)).toEqual([mine])
  })
})

describe('cooldownPct', () => {
  it('reads back what withCooldown wrote', () => {
    expect(cooldownPct(withCooldown([mine], heroes, 35))).toBe(35)
  })

  it('is null without a cooldown entry', () => {
    expect(cooldownPct([mine])).toBeNull()
    expect(cooldownPct([])).toBeNull()
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
