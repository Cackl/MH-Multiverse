// Game Tweaks: recipes the Simple Patches view writes into the launcher's own patch file.
// Each generated entry's Description starts with a [MV:<template>:<key> <inputs>] tag so the
// view can read its settings back and replace only its own entries.

import type { PatchEntry } from './patchesMeta'

/** Every shipping hero is a non-abstract AvatarPrototype in this folder. */
export const AVATAR_FOLDER = 'Entity/Characters/Avatars/Shipping/'

export const COOLDOWN_MIN = 5
// A 0 cooldown never schedules a charge refill (Power.cs StartCooldown), so stop short of 100.
export const COOLDOWN_MAX = 99
export const COOLDOWN_STEP = 1
export const COOLDOWN_DEFAULT = 25

const COOLDOWN_TAG = '[MV:cooldowns:all'

/** A typed percent as the nearest allowed step, kept between the minimum and maximum. */
export function clampCooldown(pct: number): number {
  if (!Number.isFinite(pct)) return COOLDOWN_DEFAULT
  const stepped = Math.round(pct / COOLDOWN_STEP) * COOLDOWN_STEP
  return Math.min(COOLDOWN_MAX, Math.max(COOLDOWN_MIN, stepped))
}

/** The global cooldown reduction in the file, as a percent, or null when it has none. */
export function cooldownPct(entries: PatchEntry[]): number | null {
  const tagged = entries.find(e => e.Description.startsWith(COOLDOWN_TAG))
  const pct = tagged?.Description.match(/^\[MV:cooldowns:all pct=(\d+)\]/)?.[1]
  return pct === undefined ? null : Number(pct)
}

/**
 * The file's entries with the global cooldown reduction set to `pct` (null removes it).
 * One `Properties` entry per hero sets CooldownModifierPctGlobal, which the server applies to
 * every power that hero uses. Entries from other recipes and hand-made ones are kept.
 */
export function withCooldown(entries: PatchEntry[], avatarPaths: string[], pct: number | null): PatchEntry[] {
  const kept = entries.filter(e => !e.Description.startsWith(COOLDOWN_TAG))
  if (pct === null) return kept
  return [
    ...kept,
    ...avatarPaths.map(path => ({
      Enabled: true,
      Prototype: path,
      Path: 'Properties',
      Description: `${COOLDOWN_TAG} pct=${pct}] All cooldowns -${pct}%`,
      ValueType: 'Properties',
      // Properties values travel as JSON text (see patches.rs RAW_TYPES).
      Value: JSON.stringify({ CooldownModifierPctGlobal: -pct / 100 }),
    })),
  ]
}
