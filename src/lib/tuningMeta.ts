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

// ── Known settings ─────────────────────────────────────────────────────────────
// Source: MHServerEmu LiveTuning documentation (game version 1.52.0.1700).
// requiresPrototype: false only for Global (eGTV_) — all other categories
// target a specific game prototype.

export interface KnownSetting {
  setting: string
  category: string
  description: string
  requiresPrototype: boolean
}

export const KNOWN_SETTINGS: KnownSetting[] = [
  // ── Global ──
  { setting: 'eGTV_VendorBuyPrice',                 category: 'Global',            description: 'Multiplier for prices when buying items from vendors.',                                                                                                                requiresPrototype: false },
  { setting: 'eGTV_VendorSellPrice',                category: 'Global',            description: 'Multiplier for prices when selling items to vendors.',                                                                                                               requiresPrototype: false },
  { setting: 'eGTV_VendorXPGain',                   category: 'Global',            description: 'Multiplier for vendor experience when donating items.',                                                                                                              requiresPrototype: false },
  { setting: 'eGTV_PVPEnabled',                     category: 'Global',            description: 'Disables PvP game modes when set to 0.',                                                                                                                            requiresPrototype: false },
  { setting: 'eGTV_XPGain',                         category: 'Global',            description: 'Multiplier for experience.',                                                                                                                                         requiresPrototype: false },
  { setting: 'eGTV_LootDropRate',                   category: 'Global',            description: 'Multiplier for the chance of loot rolling.',                                                                                                                         requiresPrototype: false },
  { setting: 'eGTV_LootSpecialDropRate',            category: 'Global',            description: 'Multiplier for special item find (SIF).',                                                                                                                            requiresPrototype: false },
  { setting: 'eGTV_LootRarity',                     category: 'Global',            description: 'Multiplier for rare item find (RIF).',                                                                                                                               requiresPrototype: false },
  { setting: 'eGTV_PartyXPBonusPct',                category: 'Global',            description: '',                                                                                                                                                                  requiresPrototype: false },
  { setting: 'eGTV_PlayerTradeEnabled',             category: 'Global',            description: 'Disables player trade window when set to 0.',                                                                                                                        requiresPrototype: false },
  { setting: 'eGTV_CosmicPrestigeXPPct',            category: 'Global',            description: 'Override for the cosmic prestige XP multiplier. Uses the game data multiplier when set to 1 (0.04 by default).',                                                   requiresPrototype: false },
  { setting: 'eGTV_LootVaporizationEnabled',        category: 'Global',            description: 'Disables loot vaporization when set to 0.',                                                                                                                         requiresPrototype: false },
  { setting: 'eGTV_XPBuffDisplay',                  category: 'Global',            description: '',                                                                                                                                                                  requiresPrototype: false },
  { setting: 'eGTV_SIFBuffDisplay',                 category: 'Global',            description: '',                                                                                                                                                                  requiresPrototype: false },
  { setting: 'eGTV_RIFBuffDisplay',                 category: 'Global',            description: '',                                                                                                                                                                  requiresPrototype: false },
  { setting: 'eGTV_OmegaXPPct',                     category: 'Global',            description: 'Multiplier for Omega experience.',                                                                                                                                   requiresPrototype: false },
  { setting: 'eGTV_RespectLevelForGlobalXP',        category: 'Global',            description: 'Disables minimum level requirement for global XP multipliers when set to 0.',                                                                                       requiresPrototype: false },
  { setting: 'eGTV_RespectLevelForGlobalRIF',       category: 'Global',            description: 'Disables minimum level requirement for global rare item find multipliers when set to 0.',                                                                            requiresPrototype: false },
  { setting: 'eGTV_RespectLevelForGlobalSIF',       category: 'Global',            description: 'Disables minimum level requirement for global special item find multipliers when set to 0.',                                                                         requiresPrototype: false },
  { setting: 'eGTV_RespectLevelForOmegaXP',         category: 'Global',            description: 'Disables minimum level requirement for Omega XP multipliers when set to 0.',                                                                                        requiresPrototype: false },
  { setting: 'eGTV_RespectLevelForAvatarXP',        category: 'Global',            description: 'Disables minimum level requirement for avatar XP multipliers when set to 0.',                                                                                       requiresPrototype: false },
  { setting: 'eGTV_RespectLevelForRegionXP',        category: 'Global',            description: 'Disables minimum level requirement for region XP multipliers when set to 0.',                                                                                       requiresPrototype: false },
  { setting: 'eGTV_ServerBonusUnlockLevelOverride', category: 'Global',            description: 'Override for the minimum level required for live tuning multipliers to apply. Uses game data value when set to 1 (60 by default).',                                 requiresPrototype: false },
  { setting: 'eGTV_BoostTimersRunning',             category: 'Global',            description: 'Pauses boost timers even outside of hubs when set to 0.',                                                                                                           requiresPrototype: false },
  { setting: 'eGTV_InfinityXPPct',                  category: 'Global',            description: 'Multiplier for Infinity experience.',                                                                                                                                requiresPrototype: false },
  { setting: 'eGTV_RespectLevelForInfinityXP',      category: 'Global',            description: 'Disables minimum level requirement for Infinity XP multipliers when set to 0.',                                                                                     requiresPrototype: false },
  { setting: 'eGTV_SuperVerboseMetricsEnabled',     category: 'Global',            description: '',                                                                                                                                                                  requiresPrototype: false },
  { setting: 'eGTV_HighVolumeMetricsEnabled',       category: 'Global',            description: '',                                                                                                                                                                  requiresPrototype: false },
  { setting: 'eGTV_MediumVolumeMetricsEnabled',     category: 'Global',            description: '',                                                                                                                                                                  requiresPrototype: false },
  { setting: 'eGTV_LowVolumeMetricsEnabled',        category: 'Global',            description: '',                                                                                                                                                                  requiresPrototype: false },

  // ── Area ──
  { setting: 'eATV_AreaMobSpawnHeat',               category: 'Area',              description: '',                                                                                                                                                                  requiresPrototype: true },
  { setting: 'eATV_AreaMobSpawnHeatReturn',         category: 'Area',              description: '',                                                                                                                                                                  requiresPrototype: true },

  // ── World Entity ──
  { setting: 'eWETV_MobPowerDamage',                category: 'World Entity',      description: 'Multiplier for mob damage.',                                                                                                                                         requiresPrototype: true },
  { setting: 'eWETV_MobHealth',                     category: 'World Entity',      description: 'Multiplier for mob health.',                                                                                                                                         requiresPrototype: true },
  { setting: 'eWETV_MobXP',                         category: 'World Entity',      description: 'Multiplier for mob kill experience reward.',                                                                                                                         requiresPrototype: true },
  { setting: 'eWETV_MobDropRate',                   category: 'World Entity',      description: 'Multiplier for the chance of loot rolling from a specific mob.',                                                                                                    requiresPrototype: true },
  { setting: 'eWETV_MobSpecialDropRate',            category: 'World Entity',      description: 'Multiplier for special item find (SIF) from a specific mob.',                                                                                                       requiresPrototype: true },
  { setting: 'eWETV_Enabled',                       category: 'World Entity',      description: 'Disables this world entity when set to 0.',                                                                                                                         requiresPrototype: true },
  { setting: 'eWETV_MobDropRarity',                 category: 'World Entity',      description: 'Multiplier for rare item find (RIF) from a specific mob.',                                                                                                          requiresPrototype: true },
  { setting: 'eWETV_VendorEnabled',                 category: 'World Entity',      description: 'Disables vendor functionality of a specific NPC.',                                                                                                                  requiresPrototype: true },
  { setting: 'eWETV_Unused1',                       category: 'World Entity',      description: '',                                                                                                                                                                  requiresPrototype: true },
  { setting: 'eWETV_EternitySplinterPrice',         category: 'World Entity',      description: 'Override for the Eternity Splinter price when buying this item from a vendor.',                                                                                     requiresPrototype: true },
  { setting: 'eWETV_LootGroupNum',                  category: 'World Entity',      description: 'Specifies loot group number for this item to add it to a loot table.',                                                                                              requiresPrototype: true },
  { setting: 'eWETV_LootNoDropPercent',             category: 'World Entity',      description: 'Specifies the no drop chance for an item. Use in combination with eWETV_LootGroupNum.',                                                                             requiresPrototype: true },
  { setting: 'eWETV_Visible',                       category: 'World Entity',      description: 'Makes this world entity invisible when set to 0.',                                                                                                                  requiresPrototype: true },

  // ── Avatar Entity ──
  { setting: 'eAETV_BonusXPPct',                    category: 'Avatar Entity',     description: 'Avatar-specific experience multiplier.',                                                                                                                             requiresPrototype: true },
  { setting: 'eAETV_XPBuffDisplay',                 category: 'Avatar Entity',     description: '',                                                                                                                                                                  requiresPrototype: true },
  { setting: 'eAETV_EternitySplinterPrice',         category: 'Avatar Entity',     description: 'Override for Eternity Splinter price when buying this avatar. NOTE: Broken client-side when buying from the roster panel.',                                         requiresPrototype: true },
  { setting: 'eAETV_Enabled',                       category: 'Avatar Entity',     description: 'Disables this avatar when set to 0.',                                                                                                                               requiresPrototype: true },

  // ── Population Object ──
  { setting: 'ePOTV_PopulationObjectWeight',        category: 'Population Object', description: '',                                                                                                                                                                  requiresPrototype: true },

  // ── Powers ──
  { setting: 'ePTV_PowerCost',                      category: 'Powers',            description: 'Multiplier for power cost.',                                                                                                                                         requiresPrototype: true },
  { setting: 'ePTV_PowerDamagePVE',                 category: 'Powers',            description: 'Multiplier for power PvE damage.',                                                                                                                                  requiresPrototype: true },
  { setting: 'ePTV_PowerDamagePVP',                 category: 'Powers',            description: 'Multiplier for power PvP damage.',                                                                                                                                  requiresPrototype: true },
  { setting: 'ePTV_PowerEnabled',                   category: 'Powers',            description: 'Disables this power when set to 0.',                                                                                                                                requiresPrototype: true },

  // ── Regions ──
  { setting: 'eRTV_PlayerLimit',                    category: 'Regions',           description: '',                                                                                                                                                                  requiresPrototype: true },
  { setting: 'eRTV_Enabled',                        category: 'Regions',           description: 'Disables this region when set to 0.',                                                                                                                               requiresPrototype: true },
  { setting: 'eRT_BonusXPPct',                      category: 'Regions',           description: 'Region-specific experience multiplier.',                                                                                                                             requiresPrototype: true },
  { setting: 'eRT_XPBuffDisplay',                   category: 'Regions',           description: '',                                                                                                                                                                  requiresPrototype: true },
  { setting: 'eRT_BonusItemFindMultiplier',         category: 'Regions',           description: 'Multiplier for bonus item find (BIF) in this region.',                                                                                                              requiresPrototype: true },

  // ── Loot ──
  { setting: 'eLTTV_Enabled',                       category: 'Loot',              description: 'Disables this loot table when set to 0.',                                                                                                                           requiresPrototype: true },
  { setting: 'eLTTV_Weight',                        category: 'Loot',              description: 'Multiplier for the weight value of this loot table.',                                                                                                               requiresPrototype: true },
  { setting: 'eLTTV_Rolls',                         category: 'Loot',              description: 'Override for the number of rolls for this loot table.',                                                                                                             requiresPrototype: true },
  { setting: 'eLTTV_NoDropPercent',                 category: 'Loot',              description: 'Override for the no drop chance for this loot table.',                                                                                                              requiresPrototype: true },
  { setting: 'eLTTV_GroupNum',                      category: 'Loot',              description: 'When this loot table is rolled, world entities with the same loot group number are also included.',                                                                  requiresPrototype: true },

  // ── Mission ──
  { setting: 'eMTV_Enabled',                        category: 'Mission',           description: 'Disables this mission when set to 0.',                                                                                                                               requiresPrototype: true },
  { setting: 'eMTV_EventInstance',                  category: 'Mission',           description: '',                                                                                                                                                                  requiresPrototype: true },

  // ── Condition ──
  { setting: 'eCTV_Enabled',                        category: 'Condition',         description: 'Disables this condition when set to 0.',                                                                                                                             requiresPrototype: true },

  // ── Public Events ──
  { setting: 'ePETV_Enabled',                       category: 'Public Events',     description: '',                                                                                                                                                                  requiresPrototype: true },
  { setting: 'ePETV_EventInstance',                 category: 'Public Events',     description: '',                                                                                                                                                                  requiresPrototype: true },

  // ── Metrics Frequency ──
  { setting: 'eMFTV_SampleRate',                    category: 'Metrics Frequency', description: '',                                                                                                                                                                  requiresPrototype: true },
]

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
  min: number
  max: number
  step: number
  note?: string
  /** A value that means "use the game data", shown as Game default. */
  gameDefault?: number
  /** Only one of Infinity XP and Omega XP is live, set by [GameOptions] InfinitySystemEnabled. */
  infinity?: boolean
}

// Ranges from SimpleView-LiveTuning.md §5.3. Stock Global sets all of these keys.
export const RATE_ROWS: RateRow[] = [
  { setting: 'eGTV_XPGain',                  label: 'XP from kills and orbs', group: 'XP',      format: 'times',  min: 0.25, max: 10,   step: 0.25, note: 'Mission reward XP is not affected.' },
  { setting: 'eGTV_InfinityXPPct',           label: 'Infinity XP',            group: 'XP',      format: 'times',  min: 0,    max: 10,   step: 0.5, infinity: true },
  { setting: 'eGTV_OmegaXPPct',              label: 'Omega XP',               group: 'XP',      format: 'times',  min: 0,    max: 10,   step: 0.5, infinity: false },
  { setting: 'eGTV_PartyXPBonusPct',         label: 'Party XP bonus',         group: 'XP',      format: 'bonus',  min: 1,    max: 5,    step: 0.5 },
  { setting: 'eGTV_CosmicPrestigeXPPct',     label: 'XP at max prestige',     group: 'XP',      format: 'pct',    min: 0.04, max: 0.99, step: 0.01, gameDefault: 1 },
  { setting: 'eGTV_LootDropRate',            label: 'Drop rate',              group: 'Loot',    format: 'times',  min: 0.25, max: 10,   step: 0.25 },
  { setting: 'eGTV_LootRarity',              label: 'Rare item find',         group: 'Loot',    format: 'times',  min: 0.5,  max: 10,   step: 0.5 },
  { setting: 'eGTV_LootSpecialDropRate',     label: 'Special item find',      group: 'Loot',    format: 'times',  min: 0.5,  max: 10,   step: 0.5 },
  { setting: 'eGTV_LootVaporizationEnabled', label: 'Auto-convert drops',     group: 'Loot',    format: 'toggle', min: 0,    max: 1,    step: 1 },
  { setting: 'eGTV_VendorBuyPrice',          label: 'Buy prices',             group: 'Vendors', format: 'times',  min: 0,    max: 3,    step: 0.25 },
  { setting: 'eGTV_VendorSellPrice',         label: 'Sell prices',            group: 'Vendors', format: 'times',  min: 0,    max: 3,    step: 0.25 },
  { setting: 'eGTV_VendorXPGain',            label: 'Vendor XP',              group: 'Vendors', format: 'times',  min: 0,    max: 10,   step: 0.5 },
  { setting: 'eGTV_PVPEnabled',              label: 'PvP',                    group: 'Rules',   format: 'toggle', min: 0,    max: 1,    step: 1 },
  { setting: 'eGTV_PlayerTradeEnabled',      label: 'Player trading',         group: 'Rules',   format: 'toggle', min: 0,    max: 1,    step: 1 },
  { setting: 'eGTV_BoostTimersRunning',      label: 'Boost timers run',       group: 'Rules',   format: 'toggle', min: 0,    max: 1,    step: 1 },
]

/** The rows to show: all but the dead one of Infinity XP and Omega XP. */
export function rateRows(infinitySystem: boolean): RateRow[] {
  return RATE_ROWS.filter(r => r.infinity === undefined || r.infinity === infinitySystem)
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

/** True when the value can't be shown on the row's slider, e.g. 15× from a hand-written file. */
export function isCustomRate(row: RateRow, value: number): boolean {
  if (row.format === 'toggle' || value === row.gameDefault) return false
  return value < row.min || value > row.max
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
      noEffect.push(`${f.file} sets ${formatRate(row, v)}: no effect, because ${GLOBAL_FILE} loads after it.`)
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
  /** What the server rejected while loading, from its `Verify failed:` lines. */
  rejected: string[]
}

/**
 * The outcome of `!server reloadlivetuning` from the log lines printed after it was sent, or
 * null while the load hasn't finished. The command prints nothing itself; the load ends with
 * `Loaded N Live Tuning settings`, even when a file in it was skipped. A file that won't parse
 * logs `Failed to deserialize` and then `Verify failed: Failed to parse <file>`, so the Verify
 * lines alone cover both.
 */
export function reloadResult(lines: string[]): ReloadResult | null {
  const rejected: string[] = []
  for (const line of lines) {
    const loaded = /Loaded (\d+) Live Tuning settings/.exec(line)
    if (loaded) return { count: Number(loaded[1]), rejected }
    const failed = /Verify failed: (.*)/.exec(line)
    if (failed) rejected.push(failed[1].trim())
  }
  return null
}