import { describe, it, expect } from 'vitest'
import { CONFIG_SCHEMA, changedValues, fieldError, fieldsFor, otherSettings, type Field } from '../../src/lib/configMeta'

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

describe('changedValues', () => {
  const saved = { Frontend: { Port: '4306', BindIP: '127.0.0.1' }, Logging: { EnableFile: 'false' } }

  it('is empty when nothing changed', () => {
    expect(changedValues(structuredClone(saved), saved)).toEqual({})
  })

  it('keeps an edit on one page while another page is edited', () => {
    const values = structuredClone(saved)
    values.Frontend.Port = '4307'
    values.Logging.EnableFile = 'true'
    expect(changedValues(values, saved)).toEqual({ Frontend: { Port: '4307' }, Logging: { EnableFile: 'true' } })
  })

  it('treats a key missing from the saved file as blank', () => {
    expect(changedValues({ Leaderboards: { AutoSave: '' } }, saved)).toEqual({})
    expect(changedValues({ Leaderboards: { AutoSave: '5' } }, saved)).toEqual({ Leaderboards: { AutoSave: '5' } })
  })
})

describe('fieldError', () => {
  const all = CONFIG_SCHEMA.flatMap(p => [...(p.fields ?? []), ...(p.subsections ?? []).flatMap(s => s.fields)])
  const get = (id: string) => all.find(f => `${f.section}.${f.key}` === id)!
  const burst = get('WebFrontend.LoginRateLimitBurst')
  const port = get('Frontend.Port')
  const loot = get('CustomGameOptions.LootSpawnGridCellRadius')
  const bindIp = get('Frontend.BindIP')

  it('takes whole numbers for int fields and rejects decimals and blanks', () => {
    expect(fieldError(burst, '5')).toBeNull()
    expect(fieldError(burst, '12.5')).toBe('Enter a whole number')
    expect(fieldError(burst, '')).toBe('Enter a whole number')
    expect(fieldError(burst, '1')).toBe('Enter a number of at least 2')
  })

  it('takes decimals for float fields but not blanks', () => {
    expect(fieldError(loot, '20.5')).toBeNull()
    expect(fieldError(loot, '')).toBe('Enter a number')
    expect(fieldError(loot, '200')).toBe('Enter a number from 16 to 128')
  })

  it('keeps ports in 1 to 65535', () => {
    expect(fieldError(port, '4306')).toBeNull()
    expect(fieldError(port, '0')).toBe('Enter a number from 1 to 65535')
    expect(fieldError(port, '65536')).toBe('Enter a number from 1 to 65535')
  })

  it('takes only an IP address for Bind IP', () => {
    for (const ok of ['127.0.0.1', '0.0.0.0', '::', '::1', 'fe80::1']) expect(fieldError(bindIp, ok)).toBeNull()
    for (const bad of ['localhost', '256.1.1.1', '1.2.3', '']) expect(fieldError(bindIp, bad)).toBe('Enter an IP address like 127.0.0.1')
  })

  it('leaves text fields alone', () => {
    expect(fieldError({ key: 'X', section: 'S', type: 'string', label: 'X', description: '' } as Field, '')).toBeNull()
  })

  it('marks every integer the server reads as int', () => {
    const ints = [
      'Frontend.Port', 'WebFrontend.Port', 'Frontend.ReceiveTimeoutMS',
      'WebFrontend.LoginRateLimitCostMS', 'WebFrontend.LoginRateLimitBurst',
      'PlayerManager.ServerCapacity', 'PlayerManager.MaxLoginQueueClients', 'GameInstance.NumWorkerThreads',
      'SQLiteDBManager.MaxBackupNumber', 'SQLiteDBManager.BackupIntervalMinutes',
      'JsonDBManager.MaxBackupNumber', 'JsonDBManager.BackupIntervalMinutes',
      'Leaderboards.AutoSaveIntervalMinutes', 'CustomGameOptions.AutosaveIntervalMinutes',
      'GameOptions.GiftingAccountAgeInDaysRequired', 'GameOptions.GiftingAvatarLevelRequired', 'GameOptions.GiftingLoginCountRequired',
      'MTXStore.GazillioniteBalanceForNewAccounts', 'MTXStore.ESToGazillioniteConversionStep',
      'MTXStore.GiftingOmegaLevelRequired', 'MTXStore.GiftingInfinityLevelRequired',
    ]
    for (const id of ints) expect(get(id)?.int, id).toBe(true)
  })
})
