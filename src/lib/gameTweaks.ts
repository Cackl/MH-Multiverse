// Game Tweaks: recipes the Simple Patches view writes into the launcher's own patch file.
// Each generated entry's Description starts with a [MV:<template>...] tag so the
// view can read its settings back and replace only its own entries.

import type { PatchEntry } from './patchesMeta'
import { LOAD_INT } from './patchEntryEdit'

/** Every shipping hero is a non-abstract AvatarPrototype in this folder. */
export const AVATAR_FOLDER = 'Entity/Characters/Avatars/Shipping/'
/** Player powers, for the power pickers. */
export const PLAYER_POWERS = 'Powers/Player/'

export const COOLDOWN_MIN = 5
// A 0 cooldown never schedules a charge refill (Power.cs StartCooldown), so stop short of 100.
export const COOLDOWN_MAX = 99
export const COOLDOWN_STEP = 1
export const COOLDOWN_DEFAULT = 25

// Matches the older `[MV:cooldowns:all pct=N]` tag too, so saving replaces it.
const COOLDOWN_TAG = '[MV:cooldowns'

/** A power's own reduction. `id` is the decimal prototype id. */
export interface PowerCooldown { id: string; pct: number }
/** Everything the cooldown recipe writes: the reduction for every power (null = none) and per-power ones. */
export interface Cooldowns { all: number | null; powers: PowerCooldown[] }

export const SUMMON_MIN = 1
// The shared summon inventory holds 128 (AgentSummonedEntities Capacity).
export const SUMMON_MAX = 128
export const SUMMON_DEFAULT = 4
export { SUMMON_TIP } from './patchEntryEdit'
const SUMMON_TAG = '[MV:summons'

/** How many a summon power can keep out at once. `path` is the power's prototype path. */
export interface SummonCap { path: string; max: number }

/** A typed number as a whole number between the minimum and maximum. */
export function clampSummon(n: number): number {
  if (!Number.isFinite(n)) return SUMMON_DEFAULT
  return Math.min(SUMMON_MAX, Math.max(SUMMON_MIN, Math.round(n)))
}

/**
 * The entry that sets a summon power's cap. It replaces the whole SummonMaxSimultaneous eval:
 * editing `.Value` on an inherited eval would change every power sharing it.
 */
export function summonCapEntry(cap: SummonCap): PatchEntry {
  return {
    Enabled: true,
    Prototype: cap.path,
    Path: 'SummonMaxSimultaneous',
    Description: `${SUMMON_TAG} max=${cap.max}] Up to ${cap.max} at once`,
    ValueType: 'Prototype',
    // Text, so the 64-bit id stays exact (see patches.rs RAW_TYPES).
    Value: `{"ParentDataRef":${LOAD_INT},"Value":${cap.max},"Op":"LoadInt"}`,
  }
}

/** The summon caps saved in the file. */
export function readSummonCaps(entries: PatchEntry[]): SummonCap[] {
  return entries.flatMap(e => {
    const max = e.Description.startsWith(SUMMON_TAG) ? e.Description.match(/max=(\d+)/)?.[1] : undefined
    return max === undefined ? [] : [{ path: e.Prototype, max: Number(max) }]
  })
}

/** The file's entries with the summon caps replaced. Other entries are kept. */
export function withSummonCaps(entries: PatchEntry[], caps: SummonCap[]): PatchEntry[] {
  return [...entries.filter(e => !e.Description.startsWith(SUMMON_TAG)), ...caps.map(summonCapEntry)]
}

/** A typed percent as the nearest allowed step, kept between the minimum and maximum. */
export function clampCooldown(pct: number): number {
  if (!Number.isFinite(pct)) return COOLDOWN_DEFAULT
  const stepped = Math.round(pct / COOLDOWN_STEP) * COOLDOWN_STEP
  return Math.min(COOLDOWN_MAX, Math.max(COOLDOWN_MIN, stepped))
}

/** The cooldown settings saved in the file. */
export function readCooldowns(entries: PatchEntry[]): Cooldowns {
  const tag = entries.find(e => e.Description.startsWith(COOLDOWN_TAG))?.Description.match(/^\[[^\]]*\]/)?.[0] ?? ''
  const all = tag.match(/(?:all|pct)=(\d+)/)?.[1]
  const powers = (tag.match(/powers=([\d:,]+)/)?.[1] ?? '')
    .split(',')
    .filter(Boolean)
    .map(p => { const [id, pct] = p.split(':'); return { id, pct: Number(pct) } })
  return { all: all === undefined ? null : Number(all), powers }
}

/**
 * The file's entries with the cooldown settings replaced. One `Properties` entry per hero
 * (the server reads only the first per prototype, so both kinds share it) sets
 * CooldownModifierPctGlobal and one CooldownModifierPctForPower per power. The server adds
 * them up, so a power's own value is cut to keep its total at COOLDOWN_MAX.
 * Entries from other recipes and hand-made ones are kept.
 */
export function withCooldowns(entries: PatchEntry[], avatarPaths: string[], c: Cooldowns): PatchEntry[] {
  const kept = entries.filter(e => !e.Description.startsWith(COOLDOWN_TAG))
  if (c.all === null && !c.powers.length) return kept
  const all = c.all ?? 0
  // The key repeats once per power, so this is built as text (see patches.rs RAW_TYPES).
  // Ids go in unquoted: they are 64-bit and must stay exact.
  const parts = [
    ...(c.all === null ? [] : [`"CooldownModifierPctGlobal":${-all / 100}`]),
    ...c.powers
      .map(p => ({ id: p.id, pct: Math.min(p.pct, COOLDOWN_MAX - all) }))
      .filter(p => p.pct > 0)
      .map(p => `"CooldownModifierPctForPower":[${p.id},${-p.pct / 100}]`),
  ]
  const settings = [
    ...(c.all === null ? [] : [`all=${c.all}`]),
    ...(c.powers.length ? [`powers=${c.powers.map(p => `${p.id}:${p.pct}`).join(',')}`] : []),
  ]
  const summary = [
    ...(c.all === null ? [] : [`All cooldowns -${c.all}%`]),
    ...(c.powers.length ? [`${c.powers.length} power cooldown${c.powers.length === 1 ? '' : 's'}`] : []),
  ]
  return [
    ...kept,
    ...avatarPaths.map(path => ({
      Enabled: true,
      Prototype: path,
      Path: 'Properties',
      Description: `${COOLDOWN_TAG} ${settings.join(' ')}] ${summary.join(', ')}`,
      ValueType: 'Properties',
      Value: `{${parts.join(',')}}`,
    })),
  ]
}
