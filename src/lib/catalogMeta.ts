// ─── Interfaces ───────────────────────────────────────────────────────────────
//
// Field names are PascalCase because the Rust store.rs types use
// `serde(rename_all = "PascalCase")`. The two meta fields on CatalogEntryWithMeta
// (`source_file`, `from_modified`) are snake_case because they are added by Rust
// outside the PascalCase struct.

export interface GuidItem {
  PrototypeGuid: number
  /** Decimal string — preserves full u64 precision across the JS boundary. */
  ItemPrototypeRuntimeIdForClient: string
  Quantity: number
}

export interface LocalizedEntry {
  LanguageId: string
  Description: string
  Title: string
  ReleaseDate: string
  ItemPrice: number
}

export interface UrlEntry {
  LanguageId: string
  Url: string
  ImageData: string
}

export interface NamedItem {
  Name: string
  /**
   * Per-entry display sort order. NOT a fixed value per type name.
   * Type.Order and every TypeModifier.Order on the same entry are always
   * identical — it is one logical value stored redundantly in both places.
   *
   * Observed values in stock catalog files:
   *   0   — fully hidden items (NoDisplay + NoDisplayStore); login rewards, internal grants
   *   1   — hidden-from-store items or standard purchasable boosts
   *   5   — regular visible store items
   *   999 — crafting ingredients (sorted to end)
   */
  Order: number
}

export interface CatalogEntry {
  SkuId: number
  GuidItems: GuidItem[]
  AdditionalGuidItems: GuidItem[]
  LocalizedEntries: LocalizedEntry[]
  InfoUrls: UrlEntry[]
  ContentData: UrlEntry[]
  Type: NamedItem
  TypeModifiers: NamedItem[]
}

export interface CatalogEntryWithMeta extends CatalogEntry {
  /** Base catalog filename, e.g. "CatalogBundle.json". */
  source_file: string
  /** true when the effective entry came from the *MODIFIED.json sibling. */
  from_modified: boolean
}

// ─── Item types ───────────────────────────────────────────────────────────────

/**
 * Known catalog item type names.
 *
 * Order is intentionally absent here — it is a per-entry value, not a
 * property of the type. New entries receive an explicit order argument;
 * existing entries always round-trip their own Order value unchanged.
 */
export const ITEM_TYPE_NAMES = [
  'Boost',
  'Bundle',
  'Chest',
  'Costume',
  'Hero',
  'Service',
  'TeamUp',
] as const

export type ItemTypeName = (typeof ITEM_TYPE_NAMES)[number]

export function typeNames(): string[] {
  return [...ITEM_TYPE_NAMES]
}

// ─── Offer type ───────────────────────────────────────────────────────────────

/**
 * Logical offer structure for the editor modal.
 *
 *  single — any non-Bundle type; GuidItems holds the item(s), AdditionalGuidItems is unused
 *  bundle — Type.Name === "Bundle"; GuidItems holds all bundle items
 *  bogo   — Type.Name === "Bundle"; GuidItems = purchase items, AdditionalGuidItems = bonus items
 *
 * BOGO is mechanically a Bundle entry that has a non-empty AdditionalGuidItems array.
 */
export type OfferType = 'single' | 'bundle' | 'bogo'

/**
 * Infer the offer type from an existing catalog entry.
 * Used to pre-select the offer type radio when opening an edit modal.
 */
export function inferOfferType(entry: CatalogEntry): OfferType {
  if (entry.Type.Name !== 'Bundle') return 'single'
  if (entry.AdditionalGuidItems.length > 0) return 'bogo'
  return 'bundle'
}

// ─── Item categories ──────────────────────────────────────────────────────────

/**
 * A browseable prototype category for the item picker.
 *
 * `Path` is a Calligraphy blueprint path prefix used as `blueprintHint` in
 * `search_prototypes`. Multiple paths may be pipe-separated (|) for categories
 * that span more than one prototype subtree (e.g. Test Gear).
 *
 * `IsInventoryType` is informational — it mirrors CatalogManager's flag and
 * may be used in the future for display or filtering purposes.
 */
export interface ItemCategory {
  DisplayName: string
  Path: string
  IsInventoryType: boolean
}

/**
 * Browseable item categories, sourced from CatalogManager's categories.json.
 * Each category maps to one or more Calligraphy prototype path prefixes.
 */
export const ITEM_CATEGORIES: ItemCategory[] = [
  {
    DisplayName: 'Consumables',
    Path: 'Entity/Items/Consumables',
    IsInventoryType: false,
  },
  {
    DisplayName: 'Character Tokens',
    Path: 'Entity/Items/CharacterTokens',
    IsInventoryType: false,
  },
  {
    DisplayName: 'Costumes',
    Path: 'Entity/Items/Costumes',
    IsInventoryType: false,
  },
  {
    DisplayName: 'Currency Items',
    Path: 'Entity/Items/CurrencyItems',
    IsInventoryType: false,
  },
  {
    DisplayName: 'Pets',
    Path: 'Entity/Items/Pets',
    IsInventoryType: false,
  },
  {
    DisplayName: 'Crafting',
    Path: 'Entity/Items/Crafting',
    IsInventoryType: false,
  },
  {
    DisplayName: 'Stash Tabs',
    Path: 'Entity/Inventory/PlayerInventories/StashInventories/PageProtos/AvatarGear',
    IsInventoryType: true,
  },
  {
    DisplayName: 'Test Gear',
    // Pipe-separated — picker will run a search per segment and merge results.
    Path: 'Entity/Items/Test|Entity/Items/Artifacts/Prototypes/Tier1Artifacts/RaidTest|Entity/Items/Medals/MedalBlueprints/Endgame/TestMedals',
    IsInventoryType: false,
  },
]

// ─── Type modifiers ───────────────────────────────────────────────────────────

/**
 * Available type modifiers per item type.
 * Sourced from PredefinedModifiers in CatalogService.cs.
 *
 * The runtime Order value of each modifier always matches the entry's own
 * Order — set via buildModifiers().
 */
const TYPE_MODIFIERS: Readonly<Record<string, ReadonlyArray<string>>> = {
  Bundle:  ['Giftable', 'NoDisplay', 'NoDisplayStore'],
  Hero:    ['Giftable', 'NoDisplay', 'NoDisplayStore', 'Special'],
  Costume: ['Giftable', 'NoDisplay', 'NoDisplayStore', 'Special'],
  TeamUp:  ['Giftable', 'NoDisplay', 'NoDisplayStore', 'Special'],
  Boost:   ['Giftable', 'NoDisplay', 'NoDisplayStore', 'Special'],
  Chest:   ['Giftable', 'NoDisplay', 'NoDisplayStore', 'Special'],
  Service: ['StashPage', 'PowerSpecPanel'],
}

/** Returns the list of available modifier names for a given item type. */
export function modifiersForType(typeName: string): string[] {
  return [...(TYPE_MODIFIERS[typeName] ?? [])]
}

/**
 * Build a TypeModifiers array for a catalog entry.
 * Each modifier's Order is set to match the entry's own order value,
 * consistent with the convention used throughout the stock catalog files.
 */
export function buildModifiers(modifierNames: string[], order: number): NamedItem[] {
  return modifierNames.map(name => ({ Name: name, Order: order }))
}

// ─── Prototype search hints ───────────────────────────────────────────────────

/**
 * Maps item type names to a Calligraphy blueprint path substring.
 * Passed as blueprint_hint to search_prototypes in the GuidItem prototype
 * search dropdown to scope results to a relevant category.
 */
const BLUEPRINT_HINTS: Partial<Record<string, string>> = {
  Hero:    'Avatar',
  Costume: 'Costume',
  TeamUp:  'TeamUp',
  Boost:   'Consumable',
  Chest:   'Consumable',
  Service: 'Inventory',
  // Bundle has no single blueprint scope — leave undefined to search all types
}

/** Returns a blueprint hint substring for the prototype search, or undefined for Bundle. */
export function blueprintHintForType(typeName: string): string | undefined {
  return BLUEPRINT_HINTS[typeName]
}

// ─── Entry factories ──────────────────────────────────────────────────────────

export function defaultLocalizedEntry(): LocalizedEntry {
  return {
    LanguageId: 'en_us',
    Description: '',
    Title: '',
    ReleaseDate: '',
    ItemPrice: 1,
  }
}

export function defaultUrlEntry(): UrlEntry {
  return { LanguageId: 'en_us', Url: '', ImageData: '' }
}

/**
 * Build a skeleton CatalogEntry for a new item ready to be filled in by
 * the editor modal.
 *
 * @param typeName - Item type (e.g. "Boost", "Bundle")
 * @param skuId    - From get_next_sku_id
 * @param order    - Per-entry display sort order. Defaults to 5 (standard visible
 *                   store item). Use 0 for fully hidden, 1 for hidden-from-store.
 */
export function newCatalogEntry(
  typeName: string,
  skuId: number,
  order: number = 5,
): CatalogEntry {
  const isBundle = typeName === 'Bundle'
  return {
    SkuId: skuId,
    GuidItems: [],
    AdditionalGuidItems: [],
    LocalizedEntries: [defaultLocalizedEntry()],
    InfoUrls: isBundle ? [defaultUrlEntry()] : [],
    ContentData: isBundle ? [defaultUrlEntry()] : [],
    Type: { Name: typeName, Order: order },
    TypeModifiers: [],
  }
}

// ─── Display helpers ──────────────────────────────────────────────────────────

/** Return the title from the en_us localized entry, falling back to the first available. */
export function entryTitle(entry: CatalogEntry): string {
  const loc =
    entry.LocalizedEntries.find(e => e.LanguageId === 'en_us') ??
    entry.LocalizedEntries[0]
  return loc?.Title ?? `SKU ${entry.SkuId}`
}

/** Return the price from the en_us localized entry, falling back to the first available. */
export function entryPrice(entry: CatalogEntry): number {
  const loc =
    entry.LocalizedEntries.find(e => e.LanguageId === 'en_us') ??
    entry.LocalizedEntries[0]
  return loc?.ItemPrice ?? 0
}