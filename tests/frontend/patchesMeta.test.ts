import { describe, it, expect } from 'vitest'
import { patchPack, STOCK_PACKS } from '../../src/lib/patchesMeta'

describe('patchPack', () => {
  it('stock files get their curated name and description', () => {
    expect(patchPack('PatchDataBugFixes.json')).toEqual(STOCK_PACKS['PatchDataBugFixes.json'])
    expect(patchPack('PatchDataBugFixes.json').name).toBe('Bug fixes')
  })

  it('unknown files fall back to the name without PatchData and .json', () => {
    expect(patchPack('PatchDataMyTweaks.json')).toEqual({ name: 'MyTweaks', description: '' })
  })

  it('a name that strips to nothing keeps the file name', () => {
    expect(patchPack('PatchData.json').name).toBe('PatchData.json')
  })

  it('every curated pack has a name and a description', () => {
    for (const pack of Object.values(STOCK_PACKS)) {
      expect(pack.name).not.toBe('')
      expect(pack.description).not.toBe('')
    }
  })
})
