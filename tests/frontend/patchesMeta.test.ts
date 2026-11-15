import { describe, it, expect } from 'vitest'
import { patchPack, STOCK_PACKS, idsOf, namedValue, pickedValue, type PatchEntry } from '../../src/lib/patchesMeta'

describe('pickedValue', () => {
  const a = { prototype_id: '101', prototype_guid: '201' }
  const b = { prototype_id: '102', prototype_guid: '202' }

  it('fills an empty value with the picked id, GUID or a one-item list', () => {
    expect(pickedValue('', 'PrototypeId', a, null)).toBe('101')
    expect(pickedValue('0', 'PrototypeDataRef', a, null)).toBe('101')
    expect(pickedValue(' ', 'PrototypeGuid', a, null)).toBe('201')
    expect(pickedValue('[]', 'PrototypeId[]', a, null)).toBe('["101"]')
    expect(pickedValue('', 'PrototypeDataRef[]', a, null)).toBe('["101"]')
  })

  it('keeps a value the entry already had', () => {
    expect(pickedValue('555', 'PrototypeId', a, null)).toBe('555')
    expect(pickedValue('["7","8"]', 'PrototypeId[]', a, null)).toBe('["7","8"]')
  })

  it('replaces what an earlier pick filled in', () => {
    expect(pickedValue('101', 'PrototypeId', b, a)).toBe('102')
    expect(pickedValue('201', 'PrototypeGuid', b, a)).toBe('202')
    expect(pickedValue('["101"]', 'PrototypeId[]', b, a)).toBe('["102"]')
    expect(pickedValue('555', 'PrototypeId', b, a)).toBe('555')
  })

  it('leaves other value types alone', () => {
    expect(pickedValue('', 'Float', a, null)).toBe('')
    expect(pickedValue('0', 'Integer', a, null)).toBe('0')
  })
})

const entry = (ValueType: string, Value: unknown): PatchEntry =>
  ({ Enabled: true, Prototype: 'P', Path: 'F', Description: '', ValueType, Value })

describe('idsOf', () => {
  it('takes single ids and id arrays', () => {
    expect(idsOf(entry('PrototypeId', '11'))).toEqual(['11'])
    expect(idsOf(entry('PrototypeGuid', '12'))).toEqual(['12'])
    expect(idsOf(entry('PrototypeDataRef[]', ['13', '14']))).toEqual(['13', '14'])
  })

  it('ignores types that hold no prototype ids', () => {
    expect(idsOf(entry('Integer', 5))).toEqual([])
    expect(idsOf(entry('LocaleStringId', '15'))).toEqual([])
    expect(idsOf(entry('Prototype', '{"Item":16}'))).toEqual([])
  })

  it('skips values that are not id strings', () => {
    expect(idsOf(entry('PrototypeId', null))).toEqual([])
    expect(idsOf(entry('PrototypeId[]', ['17', 18]))).toEqual(['17'])
  })
})

describe('namedValue', () => {
  const ids = { '11': 'Doom Bot', '13': 'Kadoomer' }

  it('names the ids it knows and keeps the rest', () => {
    expect(namedValue(entry('PrototypeId', '11'), ids)).toBe('Doom Bot')
    expect(namedValue(entry('PrototypeId[]', ['13', '99']), ids)).toBe('Kadoomer, 99')
  })

  it('is null when no id has a name', () => {
    expect(namedValue(entry('PrototypeId', '99'), ids)).toBeNull()
    expect(namedValue(entry('Integer', 11), ids)).toBeNull()
  })
})

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
