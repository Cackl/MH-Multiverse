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

/** The launcher's own patch file for Game Tweaks. Z so it loads after the stock files. */
export const TWEAKS_FILE = 'PatchDataZMultiverseTweaks.json'
const TWEAKS_PACK: PatchPack = { name: 'Multiverse tweaks', description: 'The tweaks you set above' }

/** Curated name and description, or the file name without PatchData and .json. The curated
 *  ones describe 1.52's stock files, so other versions pass labels false. */
export function patchPack(fileName: string, labels = true): PatchPack {
  if (fileName === TWEAKS_FILE) return TWEAKS_PACK
  return (labels ? STOCK_PACKS[fileName] : undefined)
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

/** The ids a prototype pick offers, as the picker returns them. */
export interface PickedIds {
  prototype_id: string
  prototype_guid: string
}

/**
 * The editor's Value text after picking a prototype. For id and GUID types it becomes the
 * picked prototype's own id, but only over an empty value or one an earlier pick (`filledBy`)
 * put there: in most patches the value names a different prototype than the target.
 */
export function pickedValue(current: string, valueType: string, pick: PickedIds, filledBy: PickedIds | null): string {
  const text = current.trim()
  const fromEarlierPick = !!filledBy && [filledBy.prototype_id, filledBy.prototype_guid, JSON.stringify([filledBy.prototype_id])].includes(text)
  if (!['', '0', '[]'].includes(text) && !fromEarlierPick) return current
  switch (valueType) {
    case 'PrototypeId':
    case 'PrototypeDataRef':     return pick.prototype_id
    case 'PrototypeGuid':        return pick.prototype_guid
    case 'PrototypeId[]':
    case 'PrototypeDataRef[]':   return JSON.stringify([pick.prototype_id])
    default:                     return current
  }
}

export interface PatchEntry {
  Enabled: boolean
  Prototype: string
  Path: string
  Description: string
  ValueType: string
  Value: unknown
}
