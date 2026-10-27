import { describe, it, expect } from 'vitest'
import { categoryForSetting, blueprintHintForSetting, CATEGORY_PREFIXES, KNOWN_CORE } from '../../src/lib/tuningMeta'

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
    expect(blueprintHintForSetting('eWETV_Enabled')).toBe('WorldEntity')
  })
  it('unknown prefix returns null', () => {
    expect(blueprintHintForSetting('eXYZTV_Nonsense')).toBeNull()
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
