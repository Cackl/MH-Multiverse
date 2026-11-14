import { describe, it, expect } from 'vitest'
import {
  inferOfferType,
  additionalItemsToSave,
  usesGeneratedPage,
  modifiersForType,
  buildModifiers,
  entryTitle,
  entryPrice,
  defaultLocalizedEntry,
  removeKind,
  stockVersion,
  type CatalogEntry,
  type CatalogEntryWithMeta,
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
    expect(inferOfferType(entryWithType('Bundle', [{ PrototypeGuid: '1', ItemPrototypeRuntimeIdForClient: '1', Quantity: 1 }]))).toBe('bogo')
  })
})

describe('usesGeneratedPage', () => {
  const info = 'http://cdn/x_1_en_bundle.html', thumb = 'http://cdn/images/MTXStore_x_1.png'
  const withUrls = (type: string, i: string, t: string): CatalogEntry => ({
    ...entryWithType(type),
    InfoUrls: [{ LanguageId: 'en_us', Url: i, ImageData: '' }],
    ContentData: [{ LanguageId: 'en_us', Url: t, ImageData: '' }],
  })
  it.each([
    ['our bundle', withUrls('Bundle', info, thumb), true],
    ['stock bundle image', withUrls('Bundle', info, 'http://cdn/images/MTXStore_Bundle_Thumb_X.png'), false],
    ['stock bundle page', withUrls('Bundle', 'http://cdn/x_bundle.html', thumb), false],
    ['not a bundle', withUrls('Boost', info, thumb), false],
    ['no urls', entryWithType('Bundle'), false],
  ] as const)('%s', (_, entry, want) => {
    expect(usesGeneratedPage(entry, info, thumb)).toBe(want)
  })
})

describe('additionalItemsToSave', () => {
  const bonus = [{ PrototypeGuid: '1', ItemPrototypeRuntimeIdForClient: '2', Quantity: 1 }]
  it.each([
    ['single', bonus],
    ['bogo', bonus],
    ['bundle', []],
  ] as const)('%s saves %j', (offerType, expected) => {
    expect(additionalItemsToSave(offerType, bonus)).toEqual(expected)
  })
})

describe('modifiersForType', () => {
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

describe('removeKind / stockVersion', () => {
  const stock = entryWithType('Hero')
  const changed = { ...stock, TypeModifiers: [{ Name: 'NoDisplay', Order: 5 }] }
  const row = (from_modified: boolean, hasStock: boolean): CatalogEntryWithMeta =>
    ({ ...changed, source_file: 'Catalog.json', from_modified, stock: hasStock ? stock : null })

  it.each([
    ['stock row', false, true, null],
    ['changed stock entry', true, true, 'revert'],
    ['entry only MODIFIED has', true, false, 'delete'],
  ] as const)('%s', (_, fromModified, hasStock, kind) => {
    expect(removeKind(row(fromModified, hasStock))).toBe(kind)
  })

  it('a reverted entry shows stock and no longer counts as modified', () => {
    const r = stockVersion(row(true, true))
    expect(r.TypeModifiers).toEqual([])
    expect(r.from_modified).toBe(false)
    expect(r.source_file).toBe('Catalog.json')
    expect(removeKind(r)).toBeNull()
  })
})
