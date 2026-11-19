import { describe, it, expect } from 'vitest'
import { CONFIG_SCHEMA, fieldsFor, otherSettings } from '../../src/lib/configMeta'

describe('fieldsFor', () => {
  const progression = CONFIG_SCHEMA.find(p => p.id === 'gameplay')!.subsections!.flatMap(s => s.fields)
  const keys = (hasInfinity: boolean) => fieldsFor(progression, hasInfinity).map(f => f.key)

  it('hides Infinity and Difficulty choice without the Infinity system', () => {
    expect(keys(false)).not.toContain('InfinitySystemEnabled')
    expect(keys(false)).not.toContain('IsDifficultySliderEnabled')
    expect(keys(false)).toContain('TeamUpSystemEnabled')
    expect(keys(false)).toHaveLength(progression.length - 2)
  })

  it('shows every field with it', () => {
    expect(keys(true)).toEqual(progression.map(f => f.key))
  })
})

describe('otherSettings', () => {
  it('lists keys no page shows, sorted and grouped by section, with switches for true/false', () => {
    const defaults = {
      GameOptions: { OrbisTrophiesEnabled: 'true', TeamUpSystemEnabled: 'true' },
      Frontend: { Port: '4306' },
    }
    const values = {
      GameOptions: { OrbisTrophiesEnabled: 'false', TeamUpSystemEnabled: 'true' },
      Frontend: { Port: '4306' },
      Zeta: { NewKey: '1.0', AFlag: 'False' },
    }
    const descriptions = { GameOptions: { OrbisTrophiesEnabled: 'PS4 trophies.' } }

    expect(otherSettings(values, defaults, descriptions)).toEqual([
      { title: 'GameOptions', fields: [
        { key: 'OrbisTrophiesEnabled', section: 'GameOptions', label: 'OrbisTrophiesEnabled', type: 'bool', description: 'PS4 trophies.' },
      ] },
      { title: 'Zeta', fields: [
        { key: 'AFlag', section: 'Zeta', label: 'AFlag', type: 'bool', description: '' },
        { key: 'NewKey', section: 'Zeta', label: 'NewKey', type: 'string', description: '' },
      ] },
    ])
  })
})
