import { describe, it, expect } from 'vitest'
import {
  inferOfferType,
  modifiersForType,
  buildModifiers,
  newCatalogEntry,
  entryTitle,
  entryPrice,
  defaultLocalizedEntry,
  type CatalogEntry,
} from '../../src/lib/catalogMeta'

function entryWithType(name: string, additional: unknown[] = []): CatalogEntry {
  return {
    SkuId: '1',
    GuidItems: [],
    AdditionalGuidItems: additional as CatalogEntry['AdditionalGuidItems'],
    LocalizedEntries: [defaultLocalizedEntry()],
    InfoUrls: [],
    ContentData: [],
    Type: { Name: name, Order: 5 },
    TypeModifiers: [],
  }
}

describe('inferOfferType', () => {
  it('non-bundle type is single', () => {
    expect(inferOfferType(entryWithType('Hero'))).toBe('single')
  })
  it('Bundle with no additional items is bundle', () => {
    expect(inferOfferType(entryWithType('Bundle'))).toBe('bundle')
  })
  it('Bundle with additional items is bogo', () => {
    expect(inferOfferType(entryWithType('Bundle', [{ PrototypeGuid: 1, ItemPrototypeRuntimeIdForClient: '1', Quantity: 1 }]))).toBe('bogo')
  })
})

describe('modifiersForType', () => {
  it('known type returns its modifiers', () => {
    expect(modifiersForType('Bundle')).toContain('Giftable')
  })
  it('unknown type returns empty array', () => {
    expect(modifiersForType('NotAType')).toEqual([])
  })
})

describe('buildModifiers', () => {
  it('every modifier gets the same order as the entry', () => {
    const mods = buildModifiers(['Giftable', 'NoDisplay'], 5)
    expect(mods).toEqual([{ Name: 'Giftable', Order: 5 }, { Name: 'NoDisplay', Order: 5 }])
  })
})

describe('newCatalogEntry', () => {
  it('defaults to order 5', () => {
    const entry = newCatalogEntry('Boost', '1001')
    expect(entry.Type.Order).toBe(5)
  })
  it('Bundle type gets non-empty InfoUrls and ContentData', () => {
    const entry = newCatalogEntry('Bundle', '1001')
    expect(entry.InfoUrls.length).toBeGreaterThan(0)
    expect(entry.ContentData.length).toBeGreaterThan(0)
  })
  it('non-bundle type gets empty InfoUrls and ContentData', () => {
    const entry = newCatalogEntry('Boost', '1001')
    expect(entry.InfoUrls).toEqual([])
    expect(entry.ContentData).toEqual([])
  })
})

describe('entryTitle / entryPrice', () => {
  it('prefers the en_us localized entry', () => {
    const entry = entryWithType('Hero')
    entry.LocalizedEntries = [
      { LanguageId: 'de_de', Description: '', Title: 'Titel', ReleaseDate: '', ItemPrice: 200 },
      { LanguageId: 'en_us', Description: '', Title: 'Title', ReleaseDate: '', ItemPrice: 100 },
    ]
    expect(entryTitle(entry)).toBe('Title')
    expect(entryPrice(entry)).toBe(100)
  })

  it('falls back to the first entry when there is no en_us', () => {
    const entry = entryWithType('Hero')
    entry.LocalizedEntries = [{ LanguageId: 'de_de', Description: '', Title: 'Titel', ReleaseDate: '', ItemPrice: 200 }]
    expect(entryTitle(entry)).toBe('Titel')
    expect(entryPrice(entry)).toBe(200)
  })
})
