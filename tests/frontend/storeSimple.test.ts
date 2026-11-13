import { describe, it, expect } from 'vitest'
import { entryPrice, type CatalogEntry, type CatalogEntryWithMeta } from '../../src/lib/catalogMeta'
import {
  isHidden, setHidden, setPrice, planWrites, reverted, salePrice, saleLabel, startSale, endSale, catalogReloadResult,
} from '../../src/lib/storeSimple'

function entry(sku: string, type: string, price: number, mods: string[] = ['Giftable']): CatalogEntry {
  return {
    SkuId: sku,
    GuidItems: [{ PrototypeGuid: '0', ItemPrototypeRuntimeIdForClient: '7', Quantity: 1 }],
    AdditionalGuidItems: [],
    LocalizedEntries: [
      { LanguageId: 'en_us', Description: '', Title: `Item ${sku}`, ReleaseDate: '', ItemPrice: price },
      { LanguageId: 'fr_fr', Description: '', Title: `Item ${sku}`, ReleaseDate: '', ItemPrice: price },
    ],
    InfoUrls: [],
    ContentData: [],
    Type: { Name: type, Order: 1 },
    TypeModifiers: mods.map(Name => ({ Name, Order: 1 })),
  }
}

/** A stock entry with no overlay. */
const stock = (e: CatalogEntry): CatalogEntryWithMeta => ({ ...e, source_file: 'Catalog.json', from_modified: false, stock: e })
/** An overlay over `base`; a null base means an entry that only exists in the MODIFIED file. */
const overlay = (e: CatalogEntry, base: CatalogEntry | null): CatalogEntryWithMeta =>
  ({ ...e, source_file: 'Catalog.json', from_modified: true, stock: base })
/** An unsaved edit `e` of a stock entry `base` that has no overlay yet. */
const editOf = (e: CatalogEntry, base: CatalogEntry): CatalogEntryWithMeta => ({ ...stock(base), ...e })

describe('hide', () => {
  it.each([
    [['Giftable'], false],
    [['NoDisplayStore'], true],
    [['NoDisplay'], true],
    [['Giftable', 'NoDisplay', 'NoDisplayStore'], true],
  ])('isHidden(%j) is %s', (mods, hidden) => {
    expect(isHidden(entry('1', 'Costume', 10, mods))).toBe(hidden)
  })

  it('adds both stock modifiers with the entry Order and keeps the rest; Show removes both', () => {
    const hidden = setHidden(entry('1', 'Costume', 10, ['Giftable', 'Special']), true)
    expect(hidden.TypeModifiers).toEqual(['Giftable', 'Special', 'NoDisplay', 'NoDisplayStore'].map(Name => ({ Name, Order: 1 })))
    expect(setHidden(hidden, false).TypeModifiers.map(m => m.Name)).toEqual(['Giftable', 'Special'])
  })
})

describe('planWrites', () => {
  const base = entry('1', 'Costume', 950)

  it.each([
    ['an untouched stock entry writes nothing', stock(base), [], []],
    ['a stock entry with a new price gets an overlay', editOf(setPrice(base, 710), base), ['1'], []],
    ['an overlay edited back to stock is removed', overlay(base, base), [], ['1']],
    ['an overlay that still differs is rewritten', overlay(setHidden(base, true), base), ['1'], []],
    ['an entry stock lacks is always written', overlay(base, null), ['1'], []],
  ])('%s', (_, e, upserts, removes) => {
    const out = planWrites([e])
    expect(out.upserts.map(u => u.entry.SkuId)).toEqual(upserts)
    expect(out.removes.map(r => r.sku_id)).toEqual(removes)
  })

  it('writes the plain entry to the file it loads from', () => {
    const { upserts } = planWrites([{ ...editOf(setPrice(base, 5), base), source_file: 'CatalogRestoredHeroes.json' }])
    expect(upserts[0].target_file).toBe('CatalogRestoredHeroes.json')
    expect(Object.keys(upserts[0].entry)).not.toContain('stock')
    expect(Object.keys(upserts[0].entry)).not.toContain('source_file')
  })

  it('setPrice sets every locale', () => {
    expect(setPrice(base, 5).LocalizedEntries.map(l => l.ItemPrice)).toEqual([5, 5])
  })

  it('reverted gives the stock entry, and saving it removes the overlay', () => {
    const r = reverted(overlay(setPrice(base, 1), base))!
    expect(entryPrice(r)).toBe(950)
    expect(planWrites([r]).removes.map(x => x.sku_id)).toEqual(['1'])
    expect(reverted(overlay(base, null))).toBeNull()
  })
})

describe('sale', () => {
  it.each([
    [950, 25, 715], [100, 25, 75], [50, 25, 40], [1325, 25, 995], [950, 90, 95], [50, 100, 1], [1, 50, 1],
  ])('salePrice(%i, %i%%) = %i', (before, pct, after) => {
    expect(salePrice(before, pct)).toBe(after)
  })

  it.each([
    [['Costume'], '25% off Costumes'],
    [['Hero', 'Costume'], '25% off Heroes, Costumes'],
    [['Hero', 'Costume', 'TeamUp', 'Boost', 'Chest', 'Bundle'], '25% off everything'],
  ])('saleLabel(%j)', (sections, label) => {
    expect(saleLabel(25, sections)).toBe(label)
  })

  it('prices only visible entries in the chosen sections and records what they were', () => {
    const entries = [
      stock(entry('1', 'Costume', 950)),
      stock(entry('2', 'Costume', 950, ['NoDisplay', 'NoDisplayStore'])),
      stock(entry('3', 'Hero', 900)),
      overlay(entry('4', 'Costume', 800), entry('4', 'Costume', 950)),
      stock(entry('5', 'Costume', 1)),
    ]
    const { edited, sale } = startSale(entries, 25, ['Costume'], 'T')
    expect(edited.map(e => [e.SkuId, entryPrice(e)])).toEqual([['1', 715], ['4', 600]])
    expect(sale.prices).toEqual({
      1: { before: 950, after: 715, hadOverlay: false },
      4: { before: 800, after: 600, hadOverlay: true },
    })
    expect(sale.label).toBe('25% off Costumes')
  })

  it('ending a sale restores prices exactly and leaves no overlay behind', () => {
    const base = entry('1', 'Costume', 950)
    const { edited, sale } = startSale([stock(base)], 25, ['Costume'], 'T')
    const onSale = overlay(edited[0], base)
    const end = endSale([onSale], sale)
    expect(end.kept).toEqual([])
    expect(planWrites(end.edited)).toEqual({ upserts: [], removes: [{ sku_id: '1', source_file: 'Catalog.json' }] })
  })

  it('an earlier overlay gets its own price back', () => {
    const base = entry('4', 'Costume', 950)
    const before = overlay(entry('4', 'Costume', 800), base)
    const { edited, sale } = startSale([before], 25, ['Costume'], 'T')
    const end = endSale([overlay(edited[0], base)], sale)
    expect(entryPrice(end.edited[0])).toBe(800)
    expect(planWrites(end.edited).upserts.map(u => u.entry.SkuId)).toEqual(['4'])
  })

  it('a price changed during the sale is kept and reported', () => {
    const base = entry('1', 'Costume', 950)
    const { sale } = startSale([stock(base)], 25, ['Costume'], 'T')
    const end = endSale([overlay(entry('1', 'Costume', 500), base)], sale)
    expect(end).toEqual({ edited: [], kept: ['1'] })
  })

  it("a stock price change during the sale shows through, and other edits made during it stay", () => {
    const { edited, sale } = startSale([stock(entry('1', 'Costume', 950))], 25, ['Costume'], 'T')
    const newStock = entry('1', 'Costume', 1000)
    const hiddenDuringSale = overlay(setHidden(edited[0], true), newStock)
    const end = endSale([hiddenDuringSale], sale)
    expect(entryPrice(end.edited[0])).toBe(1000)
    expect(isHidden(end.edited[0])).toBe(true)
  })
})

describe('catalogReloadResult', () => {
  it.each([
    [[], null],
    [['Parsed catalog entries from Catalog.json'], null],
    [['Loaded 1345 store catalog entries'], { count: 1345, skipped: 0 }],
    [[
      'Failed to deserialize C:/srv/Data/Game/MTXStore/CatalogMODIFIED.json as CatalogEntry[]',
      'Loaded 1259 store catalog entries',
    ], { count: 1259, skipped: 1 }],
    [['Failed to deserialize C:/srv/Data/Game/LiveTuning/X.json as TuningEntry[]', 'Loaded 3 store catalog entries'], { count: 3, skipped: 0 }],
  ])('%j', (lines, result) => {
    expect(catalogReloadResult(lines)).toEqual(result)
  })
})
