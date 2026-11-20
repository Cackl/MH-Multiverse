// Store Simple view logic: sections, hide, price, sale, and what to write.
// Pure functions so Vitest can reach them; the page only calls these and apply_store_changes.

import { buildModifiers, entryPrice, type CatalogEntry, type CatalogEntryWithMeta } from './catalogMeta'

/** The store tabs Simple shows, in the game's order. Service and untyped entries stay in Advanced. */
export const STORE_SECTIONS = [
  { type: 'Hero', label: 'Heroes' },
  { type: 'Costume', label: 'Costumes' },
  { type: 'TeamUp', label: 'Team-Ups' },
  { type: 'Boost', label: 'Items' },
  { type: 'Chest', label: 'Cards' },
  { type: 'Bundle', label: 'Bundles' },
] as const

/** 0 makes an item free. */
export const MIN_PRICE = 0
export const MAX_PRICE = 1_000_000

const HIDE = ['NoDisplay', 'NoDisplayStore']

export function isHidden(e: CatalogEntry): boolean {
  return e.TypeModifiers.some(m => HIDE.includes(m.Name))
}

/** Hide adds both of stock's hide modifiers; Show removes both. Other modifiers are kept. */
export function setHidden<T extends CatalogEntry>(e: T, hidden: boolean): T {
  const kept = e.TypeModifiers.map(m => m.Name).filter(n => !HIDE.includes(n))
  return { ...e, TypeModifiers: buildModifiers(hidden ? [...kept, ...HIDE] : kept, e.Type.Order) }
}

/** Every locale gets the same price, because the server charges the first one. */
export function setPrice<T extends CatalogEntry>(e: T, price: number): T {
  return { ...e, LocalizedEntries: e.LocalizedEntries.map(l => ({ ...l, ItemPrice: price })) }
}

const strip = ({ SkuId, GuidItems, AdditionalGuidItems, LocalizedEntries, InfoUrls, ContentData, Type, TypeModifiers }: CatalogEntry): CatalogEntry =>
  ({ SkuId, GuidItems, AdditionalGuidItems, LocalizedEntries, InfoUrls, ContentData, Type, TypeModifiers })

export function sameEntry(a: CatalogEntry, b: CatalogEntry): boolean {
  return JSON.stringify(strip(a)) === JSON.stringify(strip(b))
}

export interface StoreUpsert { entry: CatalogEntry; target_file: string }
export interface StoreRemoval { sku_id: string; source_file: string }
export interface StoreWrites { upserts: StoreUpsert[]; removes: StoreRemoval[] }

/** What to write for edited entries: an overlay row, or none at all when the edit matches stock. */
export function planWrites(edited: CatalogEntryWithMeta[]): StoreWrites {
  const out: StoreWrites = { upserts: [], removes: [] }
  for (const e of edited) {
    if (e.stock && sameEntry(e, e.stock)) {
      if (e.from_modified) out.removes.push({ sku_id: e.SkuId, source_file: e.source_file })
    } else {
      out.upserts.push({ entry: strip(e), target_file: e.source_file })
    }
  }
  return out
}

/** The entry as stock has it, keeping where it loads from. Null for an entry stock doesn't have. */
export function reverted(e: CatalogEntryWithMeta): CatalogEntryWithMeta | null {
  return e.stock ? { ...e, ...e.stock } : null
}

/** A one-word CamelCase title (raw ids, as in CatalogLimitedCostumes.json) gets spaces between words. */
export function displayTitle(title: string): string {
  return /\s/.test(title) ? title : title.replace(/([a-z0-9])([A-Z])/g, '$1 $2').replace(/([A-Z])([A-Z][a-z])/g, '$1 $2')
}

// ── Add item ──────────────────────────────────────────────────────────────────

/** The launcher's own catalog file for items added in Simple. Rows land in its MODIFIED file. */
export const ADDED_ITEMS_FILE = 'CatalogMultiverse.json'

export interface NewItem {
  /** Prototype runtime id, decimal string. */
  itemId: string
  name: string
  /** A STORE_SECTIONS type. */
  section: string
  price: number
  quantity: number
  description: string
}

/** A single-item entry with no modifiers or page links, ready for planWrites. */
export function newStoreItem(sku: string, f: NewItem): CatalogEntryWithMeta {
  return {
    SkuId: sku,
    GuidItems: [{ PrototypeGuid: '0', ItemPrototypeRuntimeIdForClient: f.itemId, Quantity: f.quantity }],
    AdditionalGuidItems: [],
    LocalizedEntries: [{ LanguageId: 'en_us', Description: f.description, Title: f.name.trim(), ReleaseDate: '', ItemPrice: f.price }],
    InfoUrls: [],
    ContentData: [],
    Type: { Name: f.section, Order: 5 },
    TypeModifiers: [],
    source_file: ADDED_ITEMS_FILE,
    from_modified: true,
    stock: null,
  }
}

// ── Sale ──────────────────────────────────────────────────────────────────────

export interface SalePrice { before: number; after: number; hadOverlay: boolean }
export interface Sale {
  label: string
  percent: number
  sections: string[]
  started: string
  prices: Record<string, SalePrice>
}
export interface StoreState { version: 1; sale?: Sale }

/**
 * Percent off, rounded to the nearest 5 G like stock prices. 100% makes it free (the server
 * accepts 0 G); any smaller discount stays at 1 G or above, so it never rounds to free.
 */
export function salePrice(before: number, percent: number): number {
  if (percent >= 100) return 0
  return Math.max(1, Math.round(before * (100 - percent) / 100 / 5) * 5)
}

export function saleLabel(percent: number, sections: string[]): string {
  const names = STORE_SECTIONS.filter(s => sections.includes(s.type)).map(s => s.label)
  return `${percent}% off ${names.length === STORE_SECTIONS.length ? 'everything' : names.join(', ')}`
}

/** Prices every visible entry in `sections` down by `percent`; entries already at the floor are left out. */
export function startSale(entries: CatalogEntryWithMeta[], percent: number, sections: string[], started: string) {
  const prices: Record<string, SalePrice> = {}
  const edited: CatalogEntryWithMeta[] = []
  for (const e of entries) {
    if (!sections.includes(e.Type.Name) || isHidden(e)) continue
    const before = entryPrice(e)
    const after = salePrice(before, percent)
    if (after >= before) continue
    prices[e.SkuId] = { before, after, hadOverlay: e.from_modified }
    edited.push(setPrice(e, after))
  }
  const sale: Sale = { label: saleLabel(percent, sections), percent, sections, started, prices }
  return { edited, sale }
}

/**
 * Undoes a sale. An entry whose price was changed during the sale is left alone and listed in
 * `kept`. An entry that had no overlay before the sale goes back to stock's current price, so a
 * stock update during the sale shows through; any other edit made to it during the sale stays.
 * Entries that aren't loaded, because their file is switched off, stay on sale: `remaining` is the
 * sale record for just those, or null when there are none.
 */
export function endSale(entries: CatalogEntryWithMeta[], sale: Sale) {
  const edited: CatalogEntryWithMeta[] = []
  const kept: string[] = []
  const loaded = new Set(entries.map(e => e.SkuId))
  const missing = Object.entries(sale.prices).filter(([sku]) => !loaded.has(sku))
  const remaining: Sale | null = missing.length ? { ...sale, prices: Object.fromEntries(missing) } : null
  for (const e of entries) {
    const p = sale.prices[e.SkuId]
    if (!p) continue
    if (entryPrice(e) !== p.after) {
      kept.push(e.SkuId)
      continue
    }
    edited.push(setPrice(e, !p.hadOverlay && e.stock ? entryPrice(e.stock) : p.before))
  }
  return { edited, kept, remaining }
}

// ── Reload ────────────────────────────────────────────────────────────────────

/**
 * The outcome of `!server reloadcatalog` from the server log lines that follow it, or null
 * while the summary line hasn't arrived. A file that fails to parse is skipped by the server.
 */
export function catalogReloadResult(lines: string[]): { count: number; skipped: number } | null {
  let skipped = 0
  for (const line of lines) {
    const loaded = /Loaded (\d+) store catalog entries/.exec(line)
    if (loaded) return { count: Number(loaded[1]), skipped }
    if (/Failed to deserialize .* as CatalogEntry/.test(line)) skipped++
  }
  return null
}
