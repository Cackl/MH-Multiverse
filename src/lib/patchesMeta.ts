// ── Patches payload types ────────────────────────────────────────────────────
// Mirror the Rust structs in patches.rs. Previously re-declared independently
// in PatchesPanel.svelte and PatchEditorModal.svelte.

export interface PatchFileInfo {
  file_name: string
  enabled: boolean
}

export interface PatchPack {
  name: string
  description: string
}

// Curated names for the stock v52 patch files, keyed by file name.
export const STOCK_PACKS: Record<string, PatchPack> = {
  'PatchDataBugFixes.json':        { name: 'Bug fixes',        description: 'Fixes broken missions, chests, T-poses and boost cooldowns' },
  'PatchDataMissions.json':        { name: 'Missions',         description: 'Restores the intro movie, Norway ambushes and discovery orb rewards' },
  'PatchDataRestoredContent.json': { name: 'Restored content', description: 'Lets the Fantastic Four insignias drop and restores the artifact drop rate' },
  'PatchDataSpecialDrops.json':    { name: 'Special drops',    description: 'Adds costumes, Fortune Cards and hero tokens to event drops' },
  'PatchDataSpecialEvents.json':   { name: 'Special events',   description: 'Loot and vendor items for the weekly and holiday events' },
  'PatchDataMysticMayhem.json':    { name: 'Mystic Mayhem',    description: 'Adds the Mystic Mayhem bosses to Midtown Patrol' },
  'PatchDataCarnival.json':        { name: 'Carnival',         description: 'Carnival event missions and items' },
  'PatchDataCowpocalypse.json':    { name: 'Cowpocalypse',     description: 'Adds a cow wave to Industry City Patrol' },
  'PatchDataGameBalance.json':     { name: 'Game balance',     description: 'Eternity Splinters drop without the boost active' },
  'PatchDataCustom.json':          { name: 'Custom items',     description: 'Example patches that add a dagger and a cloak' },
  'PatchDataTestOverrideLoot.json':{ name: 'Test loot',        description: 'Developer test that changes a mini boss loot table' },
}

/** Curated name and description, or the file name without PatchData and .json. */
export function patchPack(fileName: string): PatchPack {
  return STOCK_PACKS[fileName]
    ?? { name: fileName.replace(/^PatchData/, '').replace(/\.json$/, '') || fileName, description: '' }
}

/** What `name_patch_targets` returns: prototype path → name, decimal id or GUID → name. */
export interface PatchNames {
  paths: Record<string, string>
  ids: Record<string, string>
}

// Value types that hold prototype ids or GUIDs. They arrive as decimal strings (see patches.rs).
const ID_VALUE_TYPES = ['PrototypeId', 'PrototypeDataRef', 'PrototypeGuid', 'PrototypeId[]', 'PrototypeDataRef[]']

/** The id strings in an entry's value, or none when its type holds no prototype ids. */
export function idsOf(entry: PatchEntry): string[] {
  if (!ID_VALUE_TYPES.includes(entry.ValueType)) return []
  const values = Array.isArray(entry.Value) ? entry.Value : [entry.Value]
  return values.filter((v): v is string => typeof v === 'string')
}

/** The value as names, e.g. "Doom Bot, Kadoomer", or null when none of its ids has a name. */
export function namedValue(entry: PatchEntry, ids: Record<string, string>): string | null {
  const list = idsOf(entry)
  if (!list.some(id => id in ids)) return null
  return list.map(id => ids[id] ?? id).join(', ')
}

export interface PatchEntry {
  Enabled: boolean
  Prototype: string
  Path: string
  Description: string
  ValueType: string
  Value: unknown
}
