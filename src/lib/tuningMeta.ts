// ── Prefix-ordering safety net ───────────────────────────────────────────────
// Both maps below do a first-match scan over a prefix list. If a prefix is
// itself a prefix of a later, more specific one in the same list, the later
// one would never be reached. Verified at load time rather than left to
// convention alone.

function assertPrefixOrder<T>(entries: [string, T][], listName: string): void {
  for (let i = 0; i < entries.length; i++) {
    for (let j = i + 1; j < entries.length; j++) {
      const [earlier] = entries[i]
      const [later] = entries[j]
      if (later.startsWith(earlier)) {
        throw new Error(
          `${listName}: '${earlier}' (index ${i}) is a prefix of '${later}' (index ${j}) ` +
          `but appears first, so '${later}' can never match. Move '${later}' before '${earlier}'.`
        )
      }
    }
  }
}

// ── Category prefix map ────────────────────────────────────────────────────────

export const CATEGORY_PREFIXES: [string, string][] = [
  ['eGTV_',  'Global'],
  ['eWETV_', 'World Entity'],
  ['ePTV_',  'Powers'],
  ['eRTV_',  'Regions'],
  ['eRT_',   'Regions'],
  ['eLTTV_', 'Loot'],
  ['eMTV_',  'Mission'],
  ['eCTV_',  'Condition'],
  ['eAETV_', 'Avatar Entity'],
  ['eATV_',  'Area'],
  ['ePOTV_', 'Population Object'],
  ['eMFTV_', 'Metrics Frequency'],
  ['ePETV_', 'Public Events'],
]

assertPrefixOrder(CATEGORY_PREFIXES, 'CATEGORY_PREFIXES')

export function categoryForSetting(setting: string): string {
  for (const [prefix, label] of CATEGORY_PREFIXES) {
    if (setting.startsWith(prefix)) return label
  }
  return 'Other'
}

// Maps a setting prefix to a Calligraphy prototype path prefix (pipe-separated
// for several), used as `blueprintHint` to scope prototype searches; the
// backend prefix-matches it, case-insensitively, against paths in
// Calligraphy.sip. Maps to null for Global settings (no prototype required) and
// where the target's location is unverified (search all prototypes).
const BLUEPRINT_HINT_PREFIXES: [string, string | null][] = [
  ['eGTV_',  null], // Global — no prototype
  ['eWETV_', 'Entity/'],
  ['ePTV_',  'Powers/'],
  ['eRTV_',  'Regions/'],
  ['eRT_',   'Regions/'],
  ['eLTTV_', 'Loot/Tables/'],
  ['eMTV_',  'Missions/'],
  ['eCTV_',  null], // Condition — no standalone prototype folder found
  ['eAETV_', 'Entity/Characters/Avatars/'],
  ['eATV_',  'Regions/'],
  ['ePOTV_', 'Populations/'],
  ['ePETV_', 'Events/PublicEvents/'],
]

assertPrefixOrder(BLUEPRINT_HINT_PREFIXES, 'BLUEPRINT_HINT_PREFIXES')

// Parses a tuning value from an <input type="number"> (a number, or null/''
// when empty) or a plain string. Returns null for blank or non-numeric input,
// so a real 0 is never confused with "no value".
export function parseTuningValue(raw: string | number | null | undefined): number | null {
  if (raw === null || raw === undefined) return null
  const text = String(raw).trim()
  if (text === '') return null
  const n = Number(text)
  return Number.isFinite(n) ? n : null
}

export function blueprintHintForSetting(setting: string): string | null {
  for (const [prefix, hint] of BLUEPRINT_HINT_PREFIXES) {
    if (setting.startsWith(prefix)) return hint
  }
  return null
}

// ── Tuning/Events payload types ─────────────────────────────────────────────
// Mirror the Rust structs in tuning.rs and events.rs. Previously re-declared
// independently in TuningPanel.svelte, EventsPanel.svelte,
// EventDefinitionEditorModal.svelte, and EventRuleEditorModal.svelte.

export interface TuningFileInfo {
  canonical_name: string
  enabled: boolean
  toggleable: boolean
  relative_path: string
  event_id: string | null
  was_auto_enabled: boolean
}

export interface EventDefinition {
  id: string
  display_name: string
  file_path: string
  daily_gift: string | null
  instanced_missions: string[] | null
  is_hidden: boolean | null
}

export interface EventsData {
  definitions: EventDefinition[]
  using_override: boolean
}

export interface ScheduleRule {
  name: string
  is_enabled: boolean
  rule_type: string
  start_day_of_week: string | null
  start_month: number | null
  start_day: number | null
  duration_days: number | null
  events: string[]
}

/**
 * Why a dated rule can't be saved, or '' if it can. The server builds a SpecialDate rule's
 * start with `new DateTime(year, month, day)` and no guard, so an impossible date (31 Apr,
 * or 29 Feb in a non-leap year) takes it down. Lunar dates are converted inside a try, so
 * only their duration is checked; a missing duration makes the server drop the rule.
 */
export function ruleDateError(
  ruleType: string, month: number | null, day: number | null, durationDays: number | null,
): string {
  if (ruleType !== 'SpecialDate' && ruleType !== 'SpecialDateLunar') return ''
  if (ruleType === 'SpecialDate') {
    if (!Number.isInteger(month) || month! < 1 || month! > 12) return 'Month must be 1 to 12.'
    const max = new Date(Date.UTC(2023, month!, 0)).getUTCDate() // 2023: not a leap year
    if (!Number.isInteger(day) || day! < 1 || day! > max) return `Day must be 1 to ${max} for this month.`
  }
  if (!Number.isInteger(durationDays) || durationDays! < 1) return 'Duration must be at least 1 day.'
  return ''
}

export interface ScheduleData {
  rules: ScheduleRule[]
  using_override: boolean
}

// ── File names ─────────────────────────────────────────────────────────────────

// The server loads LiveTuningData*.json in culture-sensitive sorted order and the last value
// wins. "_" sorts before every letter, so a "LiveTuningData_" file loads before
// LiveTuningDataGlobal.json, which then resets every global it sets. "User" sorts after "Global".
export const NEW_TUNING_FILE_PREFIX = 'LiveTuningDataUser'

export function newTuningFileName(suffix: string): string {
  return `${NEW_TUNING_FILE_PREFIX}${suffix.trim()}.json`
}

/** Short label for a file: the name without "LiveTuningData", "_" or "User", and ".json". */
export function tuningDisplayName(fileName: string): string {
  return fileName.replace(/^LiveTuningData(?:_|User(?=.+\.json$))?/, '').replace(/\.json$/, '') || 'LiveTuningData'
}

// ── Known file sets ────────────────────────────────────────────────────────────

export const KNOWN_CORE = new Set([
  'LiveTuningData.json',
  'LiveTuningDataBugFixes.json',
  'LiveTuningDataGlobal.json',
  'LiveTuningDataPvP.json',
])

export const KNOWN_EVENTS = new Set([
  'LiveTuningData_CosmicChaos.json',
  'LiveTuningData_MidtownMadness.json',
  'LiveTuningData_ArmorIncursion.json',
  'LiveTuningData_OdinsBounty.json',
  'LiveTuningData_Defenders&FriendsXP.json',
  'LiveTuningData_AvengersXP.json',
  'LiveTuningData_FantasticFourXP.json',
  'LiveTuningData_Guardians&CosmicXP.json',
  'LiveTuningData_Scoundrels&VillainsXP.json',
  'LiveTuningData_XMenXP.json',
  'LiveTuningData_PandemoniumProtocol.json',
])

/** Sidebar groups of the Advanced Live Tuning page, in display order. */
export const TUNING_GROUPS = ['Your files', 'Core', 'Events', 'Other'] as const
export type TuningGroup = typeof TUNING_GROUPS[number]

/**
 * Which sidebar group a file belongs to. `eventFiles` names the event-owned files (from
 * Events.json, else KNOWN_EVENTS on older servers). Other is the files in subfolders.
 */
export function tuningGroup(f: Pick<TuningFileInfo, 'canonical_name' | 'toggleable' | 'event_id'>, eventFiles: Set<string>): TuningGroup {
  if (f.event_id !== null || eventFiles.has(f.canonical_name)) return 'Events'
  if (!f.toggleable) return 'Other'
  return KNOWN_CORE.has(f.canonical_name) ? 'Core' : 'Your files'
}

// ── Known settings ─────────────────────────────────────────────────────────────
// Source: MHServerEmu docs/Game/LiveTuning.md and SimpleView-LiveTuning.md, checked against the
// server source (v52). requiresPrototype: false only for Global (eGTV_).

export interface KnownSetting {
  setting: string
  category: string
  description: string
  requiresPrototype: boolean
}

const STOCK = 'No effect on the stock server.'
const UNKNOWN = 'Unknown. The server never reads it.'

const g = (setting: string, description: string): KnownSetting =>
  ({ setting, category: 'Global', description, requiresPrototype: false })
const p = (category: string) => (setting: string, description: string): KnownSetting =>
  ({ setting, category, description, requiresPrototype: true })

const area = p('Area'), we = p('World Entity'), av = p('Avatar Entity'), po = p('Population Object')
const pw = p('Powers'), rg = p('Regions'), lt = p('Loot'), ms = p('Mission'), cd = p('Condition')
const pe = p('Public Events'), mf = p('Metrics Frequency')

export const KNOWN_SETTINGS: KnownSetting[] = [
  g('eGTV_VendorBuyPrice',                 'Multiplier for vendor buy prices. 0 is free.'),
  g('eGTV_VendorSellPrice',                'Multiplier for vendor sell prices and donation XP.'),
  g('eGTV_VendorXPGain',                   'Multiplier for vendor XP from donations.'),
  g('eGTV_PVPEnabled',                     'Blocks PvP when 0.'),
  g('eGTV_XPGain',                         'Multiplier for kill and orb XP. Not mission rewards.'),
  g('eGTV_LootDropRate',                   'Multiplier for drop chance from mobs and chests.'),
  g('eGTV_LootSpecialDropRate',            'Multiplier for special item find. Mostly event and bonus tables.'),
  g('eGTV_LootRarity',                     'Multiplier for rare item find. Levels off around 10.'),
  g('eGTV_PartyXPBonusPct',                'Adds (value - 1) to party XP. Below 1 does nothing.'),
  g('eGTV_PlayerTradeEnabled',             'Blocks player trading when 0.'),
  g('eGTV_CosmicPrestigeXPPct',            'XP factor at max prestige. 1 uses the game data (0.04).'),
  g('eGTV_LootVaporizationEnabled',        'Blocks loot vaporising when 0.'),
  g('eGTV_XPBuffDisplay',                  'Shows an XP bonus buff icon. Visual only.'),
  g('eGTV_SIFBuffDisplay',                 'Shows a special item find buff icon. Visual only.'),
  g('eGTV_RIFBuffDisplay',                 'Shows a rare item find buff icon. Visual only.'),
  g('eGTV_OmegaXPPct',                     'Multiplier for Omega XP. Infinity system off only.'),
  g('eGTV_RespectLevelForGlobalXP',        `0 skips the level gate for global XP. ${STOCK}`),
  g('eGTV_RespectLevelForGlobalRIF',       `0 skips the level gate for global rare item find. ${STOCK}`),
  g('eGTV_RespectLevelForGlobalSIF',       `0 skips the level gate for global special item find. ${STOCK}`),
  g('eGTV_RespectLevelForOmegaXP',         `0 skips the level gate for Omega XP. ${STOCK}`),
  g('eGTV_RespectLevelForAvatarXP',        `0 skips the level gate for hero XP. ${STOCK}`),
  g('eGTV_RespectLevelForRegionXP',        `0 skips the level gate for region XP. ${STOCK}`),
  g('eGTV_ServerBonusUnlockLevelOverride', 'Minimum level for bonuses. 1 uses the game data (60). Stock sets 0.'),
  g('eGTV_BoostTimersRunning',             'Pauses boost timers everywhere when 0.'),
  g('eGTV_InfinityXPPct',                  'Multiplier for Infinity XP. Infinity system on only.'),
  g('eGTV_RespectLevelForInfinityXP',      `0 skips the level gate for Infinity XP. ${STOCK}`),
  g('eGTV_SuperVerboseMetricsEnabled',     UNKNOWN),
  g('eGTV_HighVolumeMetricsEnabled',       UNKNOWN),
  g('eGTV_MediumVolumeMetricsEnabled',     UNKNOWN),
  g('eGTV_LowVolumeMetricsEnabled',        UNKNOWN),

  area('eATV_AreaMobSpawnHeat',            'Multiplier for spawn heat. Capped at the area\'s maximum.'),
  area('eATV_AreaMobSpawnHeatReturn',      'Multiplier for spawn heat return. Capped at the area\'s maximum.'),

  we('eWETV_MobPowerDamage',               'Multiplier for damage from this entity and its summons.'),
  we('eWETV_MobHealth',                    'Multiplier for health. New spawns only.'),
  we('eWETV_MobXP',                        'Multiplier for kill XP.'),
  we('eWETV_MobDropRate',                  'Multiplier for drop chance.'),
  we('eWETV_MobSpecialDropRate',           'Multiplier for special item find.'),
  we('eWETV_Enabled',                      '0 off, 1 default, 2 force on.'),
  we('eWETV_MobDropRarity',                'Multiplier for rare item find.'),
  we('eWETV_VendorEnabled',                'This item can\'t be bought from vendors when 0.'),
  we('eWETV_Unused1',                      'No effect.'),
  we('eWETV_EternitySplinterPrice',        'Eternity Splinter price. 1 uses the game data.'),
  we('eWETV_LootGroupNum',                 'Loot group number. Whole numbers. 1 means none.'),
  we('eWETV_LootNoDropPercent',            'No-drop chance as a loot group member. 1 skips the check.'),
  we('eWETV_Visible',                      'Hides this entity when 0. Ignored on heroes.'),

  av('eAETV_BonusXPPct',                   'Multiplier for this hero\'s XP, missions included.'),
  av('eAETV_XPBuffDisplay',                'Shows a hero XP buff icon. Visual only.'),
  av('eAETV_EternitySplinterPrice',        'Eternity Splinter price. Client only, and the roster panel ignores it.'),
  av('eAETV_Enabled',                      'Disables this hero in the client only. The server does not block it.'),

  po('ePOTV_PopulationObjectWeight',       'Multiplier for spawn group weight, rounded down. 0 or less excludes it.'),

  pw('ePTV_PowerCost',                     'Multiplier for power cost.'),
  pw('ePTV_PowerDamagePVE',                'Multiplier for PvE damage.'),
  pw('ePTV_PowerDamagePVP',                'Multiplier for PvP damage.'),
  pw('ePTV_PowerEnabled',                  'Disables this power when 0.'),

  rg('eRTV_PlayerLimit',                   'Multiplier for the player cap and spawn density. Never 0.'),
  rg('eRTV_Enabled',                       'Blocks teleporting here when 0.'),
  rg('eRT_BonusXPPct',                     'Multiplier for XP here, missions included.'),
  rg('eRT_XPBuffDisplay',                  'Shows a region XP buff icon. Visual only.'),
  rg('eRT_BonusItemFindMultiplier',        'Multiplier for bonus item find. Whole numbers only.'),

  lt('eLTTV_Enabled',                      '0 off, 1 default, 2 force on.'),
  lt('eLTTV_Weight',                       'Multiplier for pick weight, rounded down. 0 or less excludes it.'),
  lt('eLTTV_Rolls',                        'Number of rolls. Minimum 1.'),
  lt('eLTTV_NoDropPercent',                'No-drop chance. 1 uses the table\'s own.'),
  lt('eLTTV_GroupNum',                     'Also rolls this loot group. Whole numbers. 1 means none.'),

  ms('eMTV_Enabled',                       'Suspends this mission when 0.'),
  ms('eMTV_EventInstance',                 'Changing it resets event mission progress.'),

  cd('eCTV_Enabled',                       'Blocks this condition when 0. Other values force it on if the globals list it.'),

  pe('ePETV_Enabled',                      'Disables this public event in the client only. The server does not stop it.'),
  pe('ePETV_EventInstance',                'Keys team assignment. Whole numbers only.'),

  mf('eMFTV_SampleRate',                   'No effect.'),
]

/** Known settings that 1.48's tuning enums don't have (upstream Gazillion/1.48.0.1712/CommonMessages.cs). */
const MISSING_IN_1_48 = new Set([
  'eGTV_InfinityXPPct', 'eGTV_RespectLevelForInfinityXP',
  'eGTV_SuperVerboseMetricsEnabled', 'eGTV_HighVolumeMetricsEnabled',
  'eGTV_MediumVolumeMetricsEnabled', 'eGTV_LowVolumeMetricsEnabled',
  'eMFTV_SampleRate', 'eRT_BonusItemFindMultiplier',
])

/** The settings to offer for a game version. */
export function knownSettingsFor(version: string): KnownSetting[] {
  return version === '1.48' ? KNOWN_SETTINGS.filter(s => !MISSING_IN_1_48.has(s.setting)) : KNOWN_SETTINGS
}

const KNOWN_BY_ID = new Map(KNOWN_SETTINGS.map(s => [s.setting, s]))

/** Hover text for a setting: its description, or nothing when it isn't known. */
export function settingTip(setting: string): string {
  return KNOWN_BY_ID.get(setting)?.description ?? ''
}

// ── Server Rates (the Simple view of Live Tuning) ───────────────────────────────

/** The file the Server Rates form owns; mirrors SERVER_RATES_FILE in tuning.rs. */
export const SERVER_RATES_FILE = 'LiveTuningDataMultiverse.json'
const GLOBAL_FILE = 'LiveTuningDataGlobal.json'

export interface TuningEntry {
  prototype: string
  setting: string
  value: number
}

// Mirror read_global_tuning in tuning.rs.
export interface GlobalValue { setting: string; value: number }
export interface GlobalTuningFile { file: string; values: GlobalValue[]; error: string | null }
export interface GlobalTuning {
  files: GlobalTuningFile[]
  infinity_system: boolean
}

export type RateGroup = 'XP' | 'Loot' | 'Vendors' | 'Rules'

export interface RateRow {
  setting: string
  label: string
  group: RateGroup
  /** times: 2×; bonus: +50% (1 = +0%); pct: 8%; toggle: 0 is Off, anything else On. */
  format: 'times' | 'bonus' | 'pct' | 'toggle'
  /** The slider's range and step. */
  min: number
  max: number
  step: number
  /** The highest value the number box takes; it can go past the slider. */
  limit: number
  /** Shown as a ? tooltip beside the label. */
  tip?: string
  /** A value that means "use the game data", shown as Game default. */
  gameDefault?: number
  /** Only one of Infinity XP and Omega XP is live, set by [GameOptions] InfinitySystemEnabled. */
  infinity?: boolean
}

// Slider ranges from SimpleView-LiveTuning.md §5.3, with higher limits for the number box. The
// limits stop runaway values: 1e39 becomes Infinity, which the server turns into a huge negative
// number, and drop rates in the hundreds load the server heavily. Stock Global sets all of these.
export const RATE_ROWS: RateRow[] = [
  { setting: 'eGTV_XPGain',                  label: 'XP from kills and orbs', group: 'XP',      format: 'times',  min: 0.25, max: 10, step: 0.25, limit: 1000, tip: 'Mission reward XP is not affected.' },
  { setting: 'eGTV_InfinityXPPct',           label: 'Infinity XP',            group: 'XP',      format: 'times',  min: 0,    max: 10, step: 0.5,  limit: 1000, infinity: true },
  { setting: 'eGTV_OmegaXPPct',              label: 'Omega XP',               group: 'XP',      format: 'times',  min: 0,    max: 10, step: 0.5,  limit: 1000, infinity: false },
  { setting: 'eGTV_PartyXPBonusPct',         label: 'Party XP bonus',         group: 'XP',      format: 'bonus',  min: 1,    max: 5,  step: 0.5,  limit: 11 },
  { setting: 'eGTV_CosmicPrestigeXPPct',     label: 'XP at max prestige',     group: 'XP',      format: 'pct',    min: 0.01, max: 2,  step: 0.01, limit: 10, gameDefault: 1 },
  { setting: 'eGTV_LootDropRate',            label: 'Drop rate',              group: 'Loot',    format: 'times',  min: 0.25, max: 10, step: 0.25, limit: 1000 },
  { setting: 'eGTV_LootRarity',              label: 'Rare item find',         group: 'Loot',    format: 'times',  min: 0.5,  max: 10, step: 0.5,  limit: 1000 },
  { setting: 'eGTV_LootSpecialDropRate',     label: 'Special item find',      group: 'Loot',    format: 'times',  min: 0.5,  max: 10, step: 0.5,  limit: 1000 },
  { setting: 'eGTV_LootVaporizationEnabled', label: 'Enable Loot Vaporising', group: 'Loot',    format: 'toggle', min: 0,    max: 1,  step: 1,    limit: 1 },
  { setting: 'eGTV_VendorBuyPrice',          label: 'Buy prices',             group: 'Vendors', format: 'times',  min: 0,    max: 3,  step: 0.25, limit: 1000 },
  { setting: 'eGTV_VendorSellPrice',         label: 'Sell prices',            group: 'Vendors', format: 'times',  min: 0,    max: 3,  step: 0.25, limit: 1000 },
  { setting: 'eGTV_VendorXPGain',            label: 'Vendor XP',              group: 'Vendors', format: 'times',  min: 0,    max: 10, step: 0.5,  limit: 1000 },
  { setting: 'eGTV_PVPEnabled',              label: 'PvP',                    group: 'Rules',   format: 'toggle', min: 0,    max: 1,  step: 1,    limit: 1 },
  { setting: 'eGTV_PlayerTradeEnabled',      label: 'Player trading',         group: 'Rules',   format: 'toggle', min: 0,    max: 1,  step: 1,    limit: 1 },
  { setting: 'eGTV_BoostTimersRunning',      label: 'Boost timers run',       group: 'Rules',   format: 'toggle', min: 0,    max: 1,  step: 1,    limit: 1, tip: 'Off pauses all boost timers everywhere.' },
]

/** The rows to show: all but the dead one of Infinity XP and Omega XP. A version without the
 *  Infinity system (1.48) ignores the switch, so Omega XP is the live one there. */
export function rateRows(infinitySystem: boolean, hasInfinity = true): RateRow[] {
  const infinity = infinitySystem && hasInfinity
  return RATE_ROWS.filter(r => r.infinity === undefined || r.infinity === infinity)
}

const trim = (n: number) => String(+n.toFixed(2))

export function formatRate(row: RateRow, value: number): string {
  switch (row.format) {
    case 'toggle': return value !== 0 ? 'On' : 'Off'
    case 'bonus':  return `+${trim((value - 1) * 100)}%`
    case 'pct':    return value === row.gameDefault ? 'Game default' : `${trim(value * 100)}%`
    default:       return `${trim(value)}×`
  }
}

const round = (n: number) => Math.round(n * 1e6) / 1e6

/** The value as the number box shows it: 2 for 2×, 50 for a +50% bonus, 8 for 8%. */
export function toShown(row: RateRow, value: number): number {
  if (row.format === 'bonus') return round((value - 1) * 100)
  if (row.format === 'pct') return round(value * 100)
  return value
}

/** The value for a number typed in the box, kept between the row's min and limit. */
export function fromShown(row: RateRow, shown: number): number {
  const value = row.format === 'bonus' ? shown / 100 + 1 : row.format === 'pct' ? shown / 100 : shown
  return round(Math.min(row.limit, Math.max(row.min, value)))
}

export type RateBadge =
  | { kind: 'stock' }
  | { kind: 'yours' }
  | { kind: 'setIn'; file: string }
  | { kind: 'overriddenBy'; file: string }

export interface RateState {
  /** What the server uses now: the last base file that sets it, else the engine default 1. */
  value: number
  /** What it would be without the Server Rates file. */
  below: number
  badge: RateBadge
  notes: string[]
}

const lastValue = (values: GlobalValue[], setting: string) =>
  values.filter(v => v.setting === setting).at(-1)?.value

/** Where a global's value comes from. Base files apply in load order and the last one wins. */
export function resolveRate(tuning: GlobalTuning, row: RateRow): RateState {
  const { setting } = row
  const ownedAt = tuning.files.findIndex(f => f.file === SERVER_RATES_FILE)
  const globalAt = tuning.files.findIndex(f => f.file === GLOBAL_FILE)
  const globalSets = globalAt >= 0 && lastValue(tuning.files[globalAt].values, setting) !== undefined
  let below = 1
  let belowFile: string | null = null
  let owned: number | undefined
  let after: { file: string; value: number } | null = null
  const noEffect: string[] = []

  tuning.files.forEach((f, i) => {
    const v = lastValue(f.values, setting)
    if (v === undefined) return
    if (i === ownedAt) owned = v
    else if (i < ownedAt) { below = v; belowFile = f.file }
    else after = { file: f.file, value: v }
    if (globalSets && i < globalAt) {
      noEffect.push(`${f.file} (${formatRate(row, v)}) is ignored. Global loads after it.`)
    }
  })

  const last = after as { file: string; value: number } | null
  const badge: RateBadge =
    last ? { kind: 'overriddenBy', file: last.file }
    : owned !== undefined ? { kind: 'yours' }
    : belowFile !== null && belowFile !== GLOBAL_FILE ? { kind: 'setIn', file: belowFile }
    : { kind: 'stock' }

  return { value: last ? last.value : owned ?? below, below, badge, notes: noEffect }
}

const sameValue = (a: number, b: number) => Math.abs(a - b) < 1e-9

/**
 * The Server Rates file after saving `changes` (each with the value the setting would have
 * without the file). A value equal to that is removed, anything else is upserted in place.
 * Every other entry, including per-prototype ones, is kept as it was.
 */
export function planServerRatesWrite(
  existing: TuningEntry[],
  changes: { setting: string; value: number; below: number }[],
): TuningEntry[] {
  let out = existing
  for (const { setting, value, below } of changes) {
    const isIt = (e: TuningEntry) => e.prototype === '' && e.setting === setting
    const at = out.findIndex(isIt)
    out = out.filter((e, i) => !isIt(e) || i === at)
    if (sameValue(value, below)) out = out.filter(e => !isIt(e))
    else if (at >= 0) out[at] = { prototype: '', setting, value }
    else out.push({ prototype: '', setting, value })
  }
  return out
}

export interface ReloadResult {
  count: number
  /** How many entries or files the server rejected while loading (its `Verify failed:` lines). */
  rejected: number
}

/**
 * The outcome of `!server reloadlivetuning` from the log lines printed after it was sent, or
 * null while the load hasn't finished. The command prints nothing itself; the load ends with
 * `Loaded N Live Tuning settings`, even when a file in it was skipped. A file that won't parse
 * logs `Failed to deserialize` and then `Verify failed: Failed to parse <file>`, so the Verify
 * lines alone cover both.
 */
export function reloadResult(lines: string[]): ReloadResult | null {
  let rejected = 0
  for (const line of lines) {
    const loaded = /Loaded (\d+) Live Tuning settings/.exec(line)
    if (loaded) return { count: Number(loaded[1]), rejected }
    if (line.includes('Verify failed:')) rejected++
  }
  return null
}