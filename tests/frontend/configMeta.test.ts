import { describe, it, expect } from 'vitest'
import { otherSettings } from '../../src/lib/configMeta'

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
