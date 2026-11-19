import { describe, it, expect } from 'vitest'
import { serverFolder, withExes } from '../../src/lib/linkedVersions'
import type { LinkedInstall } from '../../src/lib/store'

const entry = (name: string, version: string): LinkedInstall =>
  ({ id: 'a', name, version, game_exe: 'old.exe', server_exe: 'olds.exe', custom: false, no_steam: false })

describe('withExes', () => {
  it('takes the client version first, then the server version', () => {
    expect(withExes(entry('Mine', '1.52'), 'g', 's', '1.48', '1.52').version).toBe('1.48')
    expect(withExes(entry('Mine', '1.52'), 'g', 's', null, '1.53').version).toBe('1.53')
  })

  it('keeps a picked version when neither is detected', () => {
    expect(withExes(entry('Mine', '1.52'), 'g', 's', null, null)).toEqual(
      { ...entry('Mine', '1.52'), game_exe: 'g', server_exe: 's' })
  })

  it('a name that was empty or only the version follows it, a chosen one stays', () => {
    expect(withExes(entry('', ''), 'g', 's', '1.48', null).name).toBe('1.48')
    expect(withExes(entry('1.52', '1.52'), 'g', 's', '1.48', null).name).toBe('1.48')
    expect(withExes(entry('Mine', '1.52'), 'g', 's', '1.48', null).name).toBe('Mine')
  })
})

describe('serverFolder', () => {
  it('is the folder holding the exe', () => {
    expect(serverFolder('D:\\Games\\MHServerEmu-v48\\MHServerEmu.exe')).toBe('MHServerEmu-v48')
    expect(serverFolder('')).toBe('')
  })
})
