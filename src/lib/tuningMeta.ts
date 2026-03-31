// ── Category prefix map ────────────────────────────────────────────────────────
// Order matters: eRT_ must come after eRTV_ to avoid false prefix matches.

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

export function categoryForSetting(setting: string): string {
  for (const [prefix, label] of CATEGORY_PREFIXES) {
    if (setting.startsWith(prefix)) return label
  }
  return 'Other'
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