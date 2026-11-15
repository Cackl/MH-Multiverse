// Game Tweaks: recipes the Simple Patches view writes into the launcher's own patch file.
// Each generated entry's Description starts with a [MV:<template>...] tag so the
// view can read its settings back and replace only its own entries.

import type { PatchEntry } from './patchesMeta'

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
