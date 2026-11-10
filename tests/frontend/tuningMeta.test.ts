import { describe, it, expect } from 'vitest'
import { categoryForSetting, blueprintHintForSetting, parseTuningValue, ruleDateError, newTuningFileName, tuningDisplayName, CATEGORY_PREFIXES,
  RATE_ROWS, SERVER_RATES_FILE, rateRows, formatRate, toShown, fromShown, resolveRate, planServerRatesWrite, reloadResult,
  type GlobalTuning, type RateRow, type TuningEntry } from '../../src/lib/tuningMeta'

describe('categoryForSetting', () => {
  it('all prefixes map to their declared category', () => {
    for (const [prefix, label] of CATEGORY_PREFIXES) {
      expect(categoryForSetting(`${prefix}Foo`)).toBe(label)
    }
  })

  it('unknown prefix falls back to Other', () => {
    expect(categoryForSetting('eXYZTV_Nonsense')).toBe('Other')
  })

  it('eRTV_ is not shadowed by the shorter eRT_ prefix', () => {
    expect(categoryForSetting('eRTV_PlayerLimit')).toBe('Regions')
    expect(categoryForSetting('eRT_BonusXPPct')).toBe('Regions')
  })
})

describe('blueprintHintForSetting', () => {
  it('Global settings need no prototype', () => {
    expect(blueprintHintForSetting('eGTV_XPGain')).toBeNull()
  })
  it('unknown prefix returns null', () => {
    expect(blueprintHintForSetting('eXYZTV_Nonsense')).toBeNull()
  })
})

describe('blueprintHintForSetting path shape', () => {
  const prefixes = ['eWETV_', 'ePTV_', 'eRTV_', 'eRT_', 'eLTTV_', 'eMTV_', 'eCTV_', 'eAETV_', 'eATV_', 'ePOTV_', 'ePETV_']
  it('returns null or a path prefix ending in / for every targeted prefix', () => {
    for (const p of prefixes) {
      const hint = blueprintHintForSetting(p + 'X')
      if (hint !== null) expect(hint, p).toMatch(/^[A-Za-z]+(\/[A-Za-z]+)*\/$/)
    }
  })
})

describe('parseTuningValue', () => {
  it('keeps 0 as 0', () => {
    expect(parseTuningValue('0')).toBe(0)
    expect(parseTuningValue(0)).toBe(0)
  })
  it('parses decimals and negatives', () => {
    expect(parseTuningValue('1.5')).toBe(1.5)
    expect(parseTuningValue(-2)).toBe(-2)
  })
  it('returns null for blank or non-numeric input', () => {
    for (const bad of ['', '  ', null, undefined, 'abc', '1abc', 'NaN', 'Infinity']) {
      expect(parseTuningValue(bad as never), String(bad)).toBeNull()
    }
  })
})

describe('ruleDateError', () => {
  const ok = ''
  it.each([
    // type,              month, day, duration, expected
    ['SpecialDate',       1,    31,  7,    ok],
    ['SpecialDate',       2,    28,  1,    ok],
    ['SpecialDate',       2,    29,  7,    'Day must be 1 to 28 for this month.'],
    ['SpecialDate',       2,    30,  7,    'Day must be 1 to 28 for this month.'],
    ['SpecialDate',       4,    30,  7,    ok],
    ['SpecialDate',       4,    31,  7,    'Day must be 1 to 30 for this month.'],
    ['SpecialDate',       12,   31,  7,    ok],
    ['SpecialDate',       3,    0,   7,    'Day must be 1 to 31 for this month.'],
    ['SpecialDate',       3,    null, 7,   'Day must be 1 to 31 for this month.'],
    ['SpecialDate',       3,    1.5, 7,    'Day must be 1 to 31 for this month.'],
    ['SpecialDate',       0,    1,   7,    'Month must be 1 to 12.'],
    ['SpecialDate',       13,   1,   7,    'Month must be 1 to 12.'],
    ['SpecialDate',       null, 1,   7,    'Month must be 1 to 12.'],
    ['SpecialDate',       1,    1,   0,    'Duration must be at least 1 day.'],
    ['SpecialDate',       1,    1,   null, 'Duration must be at least 1 day.'],
    // Lunar months have up to 30 days and the server converts them safely.
    ['SpecialDateLunar',  2,    30,  7,    ok],
    ['SpecialDateLunar',  2,    30,  null, 'Duration must be at least 1 day.'],
    // Undated rules ignore the date fields.
    ['AlwaysOn',          null, null, null, ok],
    ['DayOfWeek',         2,    31,  0,    ok],
  ] as const)('%s %s/%s for %s days', (type, month, day, duration, expected) => {
    expect(ruleDateError(type, month, day, duration)).toBe(expected)
  })
})

describe('tuning file names', () => {
  it('new files sort after LiveTuningDataGlobal.json, as the server loads them', () => {
    // The server's culture-sensitive sort ranks '_' before letters; localeCompare does the same.
    const name = newTuningFileName('  Rates ')
    expect(name).toBe('LiveTuningDataUserRates.json')
    expect(name.localeCompare('LiveTuningDataGlobal.json', 'en')).toBeGreaterThan(0)
    expect('LiveTuningData_Rates.json'.localeCompare('LiveTuningDataGlobal.json', 'en')).toBeLessThan(0)
  })

  it.each([
    ['LiveTuningDataUserRates.json', 'Rates'],
    ['LiveTuningData_TEST.json',     'TEST'],
    ['LiveTuningDataGlobal.json',    'Global'],
    ['LiveTuningData.json',          'LiveTuningData'],
    ['LiveTuningDataUser.json',      'User'],
  ])('%s shows as %s', (file, label) => {
    expect(tuningDisplayName(file)).toBe(label)
  })
})

// ── Server Rates ───────────────────────────────────────────────────────────────

const row = (setting: string): RateRow => RATE_ROWS.find(r => r.setting === setting)!
const XP = row('eGTV_XPGain')

describe('rateRows', () => {
  it.each([
    [true,  'eGTV_InfinityXPPct', 'eGTV_OmegaXPPct'],
    [false, 'eGTV_OmegaXPPct',    'eGTV_InfinityXPPct'],
  ])('InfinitySystemEnabled=%s shows %s and hides %s', (infinity, shown, hidden) => {
    const settings = rateRows(infinity).map(r => r.setting)
    expect(settings).toContain(shown)
    expect(settings).not.toContain(hidden)
    expect(settings).toHaveLength(14)
  })
})

describe('formatRate', () => {
  it.each([
    ['eGTV_XPGain', 2, '2×'],
    ['eGTV_XPGain', 0.25, '0.25×'],
    ['eGTV_PartyXPBonusPct', 1, '+0%'],
    ['eGTV_PartyXPBonusPct', 1.5, '+50%'],
    ['eGTV_CosmicPrestigeXPPct', 0.08, '8%'],
    ['eGTV_CosmicPrestigeXPPct', 1, 'Game default'],
    ['eGTV_PVPEnabled', 0, 'Off'],
    ['eGTV_PVPEnabled', 1, 'On'],
    ['eGTV_PVPEnabled', 2, 'On'],
  ])('%s = %s shows as %s', (setting, value, text) => {
    expect(formatRate(row(setting), value)).toBe(text)
  })
})

describe('toShown', () => {
  it.each([
    ['eGTV_XPGain', 2, 2],
    ['eGTV_PartyXPBonusPct', 1.5, 50],
    ['eGTV_PartyXPBonusPct', 1.1, 10],
    ['eGTV_CosmicPrestigeXPPct', 0.07, 7],
    ['eGTV_CosmicPrestigeXPPct', 1, 100],
  ])('%s = %s shows %s in the box', (setting, value, shown) => {
    expect(toShown(row(setting), value)).toBe(shown)
  })
})

describe('fromShown', () => {
  it.each([
    ['eGTV_XPGain', 3.5, 3.5],
    ['eGTV_XPGain', 250, 100],
    ['eGTV_XPGain', 0.1, 0.25],
    ['eGTV_PartyXPBonusPct', 1000, 11],
    ['eGTV_PartyXPBonusPct', 1200, 11],
    ['eGTV_PartyXPBonusPct', -5, 1],
    ['eGTV_PartyXPBonusPct', 10, 1.1],
    ['eGTV_PartyXPBonusPct', 14, 1.14], // 14 / 100 + 1 is 1.1400000000000001 unrounded
    ['eGTV_CosmicPrestigeXPPct', 7, 0.07],
    ['eGTV_CosmicPrestigeXPPct', 100, 1],
    ['eGTV_CosmicPrestigeXPPct', 5000, 10],
    ['eGTV_CosmicPrestigeXPPct', 0, 0.01],
    ['eGTV_VendorBuyPrice', 0, 0],
  ])('%s typed as %s is %s', (setting, shown, value) => {
    expect(fromShown(row(setting), shown)).toBe(value)
  })
})

type Files = [string, [string, number][]][]
const tuning = (files: Files): GlobalTuning => ({
  files: files.map(([file, values]) => ({ file, values: values.map(([setting, value]) => ({ setting, value })), error: null })),
  infinity_system: true,
})
const G = 'LiveTuningDataGlobal.json'
const OWN = SERVER_RATES_FILE
const OLD = 'LiveTuningData_Old.json'
const USER = 'LiveTuningDataUserMine.json'
const HOLIDAY = 'LiveTuningDataHoliday.json' // after Global, before the Server Rates file

describe('resolveRate', () => {
  it.each<[string, Files, number, number, object, string[]]>([
    ['nothing sets it', [[OWN, []]], 1, 1, { kind: 'stock' }, []],
    ['Global sets it', [[G, [['eGTV_XPGain', 1]]], [OWN, []]], 1, 1, { kind: 'stock' }, []],
    ['the Server Rates file wins', [[G, [['eGTV_XPGain', 1]]], [OWN, [['eGTV_XPGain', 2]]]], 2, 1, { kind: 'yours' }, []],
    ['the last entry in a file wins', [[G, [['eGTV_XPGain', 1]]], [OWN, [['eGTV_XPGain', 2], ['eGTV_XPGain', 3]]]], 3, 1, { kind: 'yours' }, []],
    ['a file after it overrides it', [[G, [['eGTV_XPGain', 1]]], [OWN, [['eGTV_XPGain', 2]]], [USER, [['eGTV_XPGain', 3]]]],
      3, 1, { kind: 'overriddenBy', file: USER }, []],
    ['a file after it overrides it before it exists', [[G, [['eGTV_XPGain', 1]]], [OWN, []], [USER, [['eGTV_XPGain', 3]]]],
      3, 1, { kind: 'overriddenBy', file: USER }, []],
    ['a file between Global and it sets it', [[G, [['eGTV_XPGain', 1]]], [HOLIDAY, [['eGTV_XPGain', 4]]], [OWN, []]],
      4, 4, { kind: 'setIn', file: HOLIDAY }, []],
    ['an _ file Global resets has no effect', [[OLD, [['eGTV_XPGain', 3]]], [G, [['eGTV_XPGain', 1]]], [OWN, []]],
      1, 1, { kind: 'stock' }, [`${OLD} (3×) is ignored. Global loads after it.`]],
    ['an _ file Global does not reset applies', [[OLD, [['eGTV_XPGain', 3]]], [G, [['eGTV_LootRarity', 1]]], [OWN, []]],
      3, 3, { kind: 'setIn', file: OLD }, []],
    ['other settings are ignored', [[G, [['eGTV_LootRarity', 5]]], [OWN, [['eGTV_LootDropRate', 2]]]], 1, 1, { kind: 'stock' }, []],
  ])('%s', (_name, files, value, below, badge, notes) => {
    expect(resolveRate(tuning(files), XP)).toEqual({ value, below, badge, notes })
  })
})

describe('planServerRatesWrite', () => {
  const e = (setting: string, value: number, prototype = ''): TuningEntry => ({ prototype, setting, value })
  const mob = e('eWETV_MobHealth', 2, 'Entity/Mobs/Brute.prototype')

  it.each<[string, TuningEntry[], { setting: string; value: number; below: number }[], TuningEntry[]]>([
    ['adds a new value', [], [{ setting: 'eGTV_XPGain', value: 2, below: 1 }], [e('eGTV_XPGain', 2)]],
    ['changes a value in place and keeps other entries',
      [e('eGTV_LootRarity', 2), e('eGTV_XPGain', 2), mob], [{ setting: 'eGTV_XPGain', value: 3, below: 1 }],
      [e('eGTV_LootRarity', 2), e('eGTV_XPGain', 3), mob]],
    ['removes a value back at what it would be without the file',
      [e('eGTV_XPGain', 2), mob], [{ setting: 'eGTV_XPGain', value: 1, below: 1 }], [mob]],
    ['removes a value equal to it after float drift',
      [e('eGTV_XPGain', 2)], [{ setting: 'eGTV_XPGain', value: 0.1 + 0.2, below: 0.3 }], []],
    ['writes Off as 0 when the value without the file is On',
      [], [{ setting: 'eGTV_PlayerTradeEnabled', value: 0, below: 1 }], [e('eGTV_PlayerTradeEnabled', 0)]],
    ['writes a value that differs from the value without the file, even 1',
      [], [{ setting: 'eGTV_XPGain', value: 1, below: 3 }], [e('eGTV_XPGain', 1)]],
    ['keeps one entry when the file repeats a setting',
      [e('eGTV_XPGain', 2), mob, e('eGTV_XPGain', 5)], [{ setting: 'eGTV_XPGain', value: 3, below: 1 }],
      [e('eGTV_XPGain', 3), mob]],
    ['leaves a per-prototype entry with the same setting name alone',
      [e('eGTV_XPGain', 9, 'Entity/Mobs/Brute.prototype')], [{ setting: 'eGTV_XPGain', value: 2, below: 1 }],
      [e('eGTV_XPGain', 9, 'Entity/Mobs/Brute.prototype'), e('eGTV_XPGain', 2)]],
    ['does nothing without changes', [e('eGTV_XPGain', 2), mob], [], [e('eGTV_XPGain', 2), mob]],
  ])('%s', (_name, existing, changes, expected) => {
    const before = structuredClone(existing)
    expect(planServerRatesWrite(existing, changes)).toEqual(expected)
    expect(existing).toEqual(before) // the input is not modified
  })
})

describe('reloadResult', () => {
  const loaded = '[LiveTuningManager] Loaded 42 Live Tuning settings'
  it.each<[string, string[], object | null]>([
    ['nothing yet', [], null],
    ['unrelated lines only', ['[CommandManager] Command invoked: invoker=[ServerConsole], command=[server], parameters=[reloadlivetuning]'], null],
    ['rejections before the load ends are not a result yet', ['[General] Verify failed: Invalid setting eGTV_Foo for prototype '], null],
    ['the load line', [loaded], { count: 42, rejected: 0 }],
    ['rejections before the load line', [
      '[General] Verify failed: Invalid setting eGTV_Foo for prototype ',
      '[FileHelper] Failed to deserialize C:\\x\\LiveTuningDataBad.json as LiveTuningUpdateValue[]',
      '[General] Verify failed: Failed to parse LiveTuningDataBad.json',
      loaded,
    ], { count: 42, rejected: 2 }],
    ['lines after the load line are ignored', [loaded, '[General] Verify failed: later'], { count: 42, rejected: 0 }],
  ])('%s', (_name, lines, expected) => {
    expect(reloadResult(lines)).toEqual(expected)
  })
})
