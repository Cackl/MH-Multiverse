import { describe, it, expect } from 'vitest'
import { categoryForSetting, blueprintHintForSetting, parseTuningValue, ruleDateError, CATEGORY_PREFIXES } from '../../src/lib/tuningMeta'

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
