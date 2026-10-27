import { describe, it, expect } from 'vitest'
import { categoryForSetting, blueprintHintForSetting, parseTuningValue, CATEGORY_PREFIXES, KNOWN_CORE } from '../../src/lib/tuningMeta'

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
  it('World Entity settings hint WorldEntity', () => {
    expect(blueprintHintForSetting('eWETV_Enabled')).toBe('Entity/')
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

describe('KNOWN_CORE', () => {
  it('is a Set containing the expected core filenames', () => {
    expect(KNOWN_CORE).toBeInstanceOf(Set)
    for (const name of ['LiveTuningData.json', 'LiveTuningDataBugFixes.json', 'LiveTuningDataGlobal.json', 'LiveTuningDataPvP.json']) {
      expect(KNOWN_CORE.has(name)).toBe(true)
    }
  })
})
