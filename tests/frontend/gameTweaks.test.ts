import { describe, it, expect } from 'vitest'
import {
  clampCooldown, readCooldowns, withCooldowns, COOLDOWN_DEFAULT,
  clampSummon, summonCapEntry, readSummonCaps, withSummonCaps, SUMMON_DEFAULT, shadowedCooldowns,
} from '../../src/lib/gameTweaks'
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

const golem = 'Powers/Player/Magik/SummonGolem.prototype'

describe('summonCapEntry', () => {
  it('replaces the whole eval with a whole-number one and an exact parent id', () => {
    expect(summonCapEntry({ path: golem, max: 8 })).toEqual({
      Enabled: true,
      Prototype: golem,
      Path: 'SummonMaxSimultaneous',
      Description: '[MV:summons max=8] Up to 8 at once',
      ValueType: 'Prototype',
      Value: `{"ParentDataRef":${BIG},"Value":8,"Op":"LoadInt"}`,
    })
  })
})

describe('withSummonCaps and readSummonCaps', () => {
  it('reads back what was written and keeps other entries', () => {
    const caps = [{ path: golem, max: 8 }, { path: 'Powers/Player/X.prototype', max: 128 }]
    const out = withSummonCaps(withCooldowns([mine], heroes, { all: 25, powers: [] }), caps)
    expect(out.slice(0, 3).map(e => e.Path)).toEqual(['Weight', 'Properties', 'Properties'])
    expect(readSummonCaps(out)).toEqual(caps)
    expect(readCooldowns(out).all).toBe(25)
  })

  it('replaces its own entries', () => {
    const once = withSummonCaps([mine], [{ path: golem, max: 8 }])
    expect(withSummonCaps(once, [])).toEqual([mine])
    expect(readSummonCaps([mine])).toEqual([])
  })
})

describe('clampSummon', () => {
  it('rounds and stays in range', () => {
    expect(clampSummon(3.6)).toBe(4)
    expect(clampSummon(0)).toBe(1)
    expect(clampSummon(500)).toBe(128)
    expect(clampSummon(NaN)).toBe(SUMMON_DEFAULT)
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

describe('shadowedCooldowns', () => {
  const T = 'PatchDataZMultiverseTweaks.json'
  const tweak = withCooldowns([], heroes, { all: 25, powers: [] })
  // Another Properties entry for Angela, with the path cased differently (the server ignores case).
  const props = (extra: Partial<PatchEntry> = {}): PatchEntry => ({
    Enabled: true, Prototype: heroes[0].toUpperCase(), Path: 'Properties', Description: 'Hand-made',
    ValueType: 'Properties', Value: '{"HealthMaxOther":1}', ...extra,
  })

  it('names an earlier file that sets Properties first', () => {
    const files = [{ file_name: 'PatchDataA.json', entries: [props()] }, { file_name: T, entries: tweak }]
    expect(shadowedCooldowns(files, T, heroes)).toEqual({ [heroes[0]]: 'PatchDataA.json' })
  })

  it('names the tweaks file when a hand-made entry comes before the cooldowns', () => {
    expect(shadowedCooldowns([{ file_name: T, entries: [props(), ...tweak] }], T, heroes)).toEqual({ [heroes[0]]: T })
  })

  it('ignores disabled entries and files after the tweaks file', () => {
    const files = [
      { file_name: 'PatchDataA.json', entries: [props({ Enabled: false })] },
      { file_name: T, entries: tweak },
      { file_name: 'PatchDataZZ.json', entries: [props()] },
    ]
    expect(shadowedCooldowns(files, T, heroes)).toEqual({})
  })

  it('finds nothing without another Properties entry', () => {
    const files = [{ file_name: 'PatchDataA.json', entries: [mine, props({ ValueType: 'Integer', Path: 'Properties.X' })] }, { file_name: T, entries: tweak }]
    expect(shadowedCooldowns(files, T, heroes)).toEqual({})
  })
})
