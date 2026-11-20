// ─── Types ───────────────────────────────────────────────────────────────────

export type FieldType = 'bool' | 'number' | 'string' | 'select'

export interface Field {
  key: string
  section: string
  type: FieldType
  label: string
  description: string
  min?: number
  max?: number
  /** The server reads it as an integer */
  int?: boolean
  /** The server parses it as an IP address */
  ip?: boolean
  options?: { value: string; label: string }[]
}

export interface SubSection {
  title: string
  fields: Field[]
}

export interface NavSection {
  id: string
  label: string
  fields?: Field[]
  subsections?: SubSection[]
}

const LOG_LEVELS = ['Trace', 'Debug', 'Info', 'Warn', 'Error', 'Fatal'].map((label, i) => ({ value: String(i), label }))

const NAME_COLOURS = ['White', 'Green', 'Blue', 'Purple', 'Orange', 'Red', 'Yellow (cosmic)'].map((label, i) => ({ value: String(i), label }))

// ─── Config schema ───────────────────────────────────────────────────────────
// Pages group keys by what they do, not by INI section. Keys left out here
// (rarely changed ones, OrbisTrophiesEnabled, keys from newer server builds)
// show under All settings.

export const CONFIG_SCHEMA: NavSection[] = [
  {
    id: 'network',
    label: 'Network',
    subsections: [
      {
        title: 'Game server',
        fields: [
          { key: 'BindIP',           section: 'Frontend', type: 'string', label: 'Bind IP',              description: 'IP address the game server listens on. 0.0.0.0 listens on all network interfaces.', ip: true },
          { key: 'Port',             section: 'Frontend', type: 'number', label: 'Port',                 description: 'Port the game client connects to.', min: 1, max: 65535, int: true },
          { key: 'PublicAddress',    section: 'Frontend', type: 'string', label: 'Public address',       description: 'Address clients use to reach this server. An IP like 192.168.1.2 or a host name.' },
        ],
      },
      {
        title: 'Web frontend',
        fields: [
          { key: 'Address',                section: 'WebFrontend', type: 'string', label: 'Address',            description: 'Address the web frontend listens on.' },
          { key: 'Port',                   section: 'WebFrontend', type: 'number', label: 'Port',               description: 'Port for logins, the web API and the dashboard. The bundled Apache forwards logins to 8080, so change it in Apache24\\conf\\extra\\httpd-ssl.conf to match.', min: 1, max: 65535, int: true },
          { key: 'EnableWebApi',           section: 'WebFrontend', type: 'bool',   label: 'Web API',            description: 'Turns on the server\'s custom web API endpoints.' },
          { key: 'EnableDashboard',        section: 'WebFrontend', type: 'bool',   label: 'Dashboard',          description: 'Serves a server dashboard in the browser. Needs the web API.' },
        ],
      },
      {
        title: 'Connection limits',
        fields: [
          { key: 'EnableLoginRateLimit', section: 'WebFrontend', type: 'bool',   label: 'Limit login attempts', description: 'Slows down repeated login attempts.' },
          { key: 'LoginRateLimitCostMS', section: 'WebFrontend', type: 'number', label: 'Cost per attempt (ms)', description: 'Time each login attempt costs.', min: 0, int: true },
          { key: 'LoginRateLimitBurst',  section: 'WebFrontend', type: 'number', label: 'Burst',                 description: 'Attempts allowed in a row before the limit applies. At least 2.', min: 2, int: true },
          { key: 'ReceiveTimeoutMS',     section: 'Frontend',    type: 'number', label: 'Receive timeout (ms)',  description: 'How long the server waits for data from a client before closing the connection. 0 waits forever. Doesn\'t affect the client\'s AFK kick.', min: 0, int: true },
        ],
      },
    ],
  },
  {
    id: 'server',
    label: 'Server',
    subsections: [
      {
        title: 'Identity',
        fields: [
          { key: 'ServerName',          section: 'GroupingManager', type: 'string',   label: 'Server name',        description: 'Name shown in chat for messages the server sends.' },
          { key: 'ServerPrestigeLevel', section: 'GroupingManager', type: 'select',   label: 'Server name colour', description: 'Colour of the server name in chat.', options: NAME_COLOURS },
          { key: 'MotdText',            section: 'GroupingManager', type: 'string',   label: 'Message of the day', description: 'Message shown to players when they log in.' },
        ],
      },
      {
        title: 'Players',
        fields: [
          { key: 'ServerCapacity',        section: 'PlayerManager', type: 'number', label: 'Capacity',          description: 'Most players online at once. Players over the cap wait in a queue. 0 means no limit.', min: 0, int: true },
          { key: 'UseWhitelist',          section: 'PlayerManager', type: 'bool',   label: 'Whitelist only',    description: 'Only accounts added with !account whitelist can log in.' },
          { key: 'MaxLoginQueueClients',  section: 'PlayerManager', type: 'number', label: 'Login queue size',  description: 'Most players that can wait in the login queue at once.', min: 0, int: true },
          { key: 'EnableTownPlayerLimit', section: 'PlayerManager', type: 'bool',   label: 'Town player limit', description: 'Towns respect the player limits set in their region data.' },
          { key: 'ShowNewsOnLogin',       section: 'PlayerManager', type: 'bool',   label: 'News on login',     description: 'Opens the news window after login.' },
          { key: 'NewsUrl',               section: 'PlayerManager', type: 'string', label: 'News URL',          description: 'Page shown in the news window.' },
        ],
      },
      {
        title: 'Chat',
        fields: [
          { key: 'EnableChatTips',         section: 'GroupingManager', type: 'bool',   label: 'Chat tips',             description: 'Broadcasts tips to all players in chat.' },
          { key: 'ChatTipFileName',        section: 'GroupingManager', type: 'string', label: 'Chat tips file',        description: 'File in the Data folder that holds the tips.' },
          { key: 'ChatTipIntervalMinutes', section: 'GroupingManager', type: 'number', label: 'Tip interval (min)',    description: 'Minutes between tips.', min: 0 },
          { key: 'ChatTipShuffle',         section: 'GroupingManager', type: 'bool',   label: 'Shuffle tips',          description: 'Shows the tips in random order.' },
          { key: 'LogPrivateChatRooms',    section: 'GroupingManager', type: 'bool',   label: 'Log private chat',      description: 'Writes tell, party and guild messages to the server log.' },
        ],
      },
      {
        title: 'Performance',
        fields: [
          { key: 'NumWorkerThreads', section: 'GameInstance', type: 'number', label: 'Worker threads', description: 'Threads that run game instances. Keep it at or below the number of logical CPUs.', min: 1, int: true },
        ],
      },
    ],
  },
  {
    id: 'persistence',
    label: 'Persistence',
    subsections: [
      {
        title: 'Player data',
        fields: [
          { key: 'EnablePersistence',          section: 'PlayerManager',     type: 'bool',   label: 'Save player data',        description: 'Saves player data between sessions.' },
          { key: 'AllowClientVersionMismatch', section: 'PlayerManager',     type: 'bool',   label: 'Allow version mismatch',  description: 'Lets clients with a different game version connect.' },
          { key: 'UseJsonDBManager',           section: 'PlayerManager',     type: 'bool',   label: 'Use JSON save file',      description: 'Saves to a JSON file instead of SQLite. Supports one account only.' },
          { key: 'AutosaveIntervalMinutes',    section: 'CustomGameOptions', type: 'number', label: 'Autosave interval (min)', description: 'How often player data is saved outside region changes. 0 or less turns autosave off.', min: -1, int: true },
        ],
      },
      {
        title: 'SQLite',
        fields: [
          { key: 'FileName',              section: 'SQLiteDBManager', type: 'string', label: 'Database file',         description: 'SQLite file name in the Data folder.' },
          { key: 'MaxBackupNumber',       section: 'SQLiteDBManager', type: 'number', label: 'Max backups',           description: 'Backups to keep. The oldest is overwritten. 0 turns backups off.', min: 0, int: true },
          { key: 'BackupIntervalMinutes', section: 'SQLiteDBManager', type: 'number', label: 'Backup interval (min)', description: 'Shortest time between backups.', min: 1, int: true },
        ],
      },
      {
        title: 'JSON',
        fields: [
          { key: 'FileName',              section: 'JsonDBManager', type: 'string', label: 'Save file',             description: 'JSON save file name in the Data folder.' },
          { key: 'MaxBackupNumber',       section: 'JsonDBManager', type: 'number', label: 'Max backups',           description: 'Backups to keep. The oldest is overwritten. 0 turns backups off.', min: 0, int: true },
          { key: 'BackupIntervalMinutes', section: 'JsonDBManager', type: 'number', label: 'Backup interval (min)', description: 'Shortest time between backups.', min: 1, int: true },
          { key: 'PlayerName',            section: 'JsonDBManager', type: 'string', label: 'Player name',           description: 'Name given to the account loaded from the JSON file.' },
        ],
      },
      {
        title: 'Leaderboards',
        fields: [
          { key: 'DatabaseFile',            section: 'Leaderboards', type: 'string', label: 'Database file',           description: 'SQLite file name in Data\\Leaderboards.' },
          { key: 'ScheduleFile',            section: 'Leaderboards', type: 'string', label: 'Schedule file',           description: 'JSON schedule file name in Data\\Leaderboards.' },
          { key: 'AutoSaveIntervalMinutes', section: 'Leaderboards', type: 'number', label: 'Autosave interval (min)', description: 'Shortest time between leaderboard saves.', min: 1, int: true },
        ],
      },
    ],
  },
  {
    id: 'gameplay',
    label: 'Gameplay',
    subsections: [
      {
        title: 'Heroes',
        fields: [
          { key: 'AutoUnlockAvatars',              section: 'CustomGameOptions', type: 'bool', label: 'Unlock all heroes',            description: 'Unlocks every hero for players who finish the tutorial.' },
          { key: 'AutoUnlockTeamUps',              section: 'CustomGameOptions', type: 'bool', label: 'Unlock all team-ups',          description: 'Unlocks every team-up for players who finish the tutorial.' },
          { key: 'AllowSameGroupTalents',          section: 'CustomGameOptions', type: 'bool', label: 'Allow same-group talents',     description: 'Lets talents from the same group be on together.' },
          { key: 'DisableMovementPowerChargeCost', section: 'CustomGameOptions', type: 'bool', label: 'Free movement power charges', description: 'Movement powers cost no charges, as before the Biggest Update Ever.' },
          { key: 'MultiSpecRewardsEnabled',        section: 'GameOptions',       type: 'bool', label: 'Multi-specs',                 description: 'Turns on multiple specs per hero.' },
        ],
      },
      {
        title: 'Progression',
        fields: [
          { key: 'NewPlayerExperienceEnabled', section: 'GameOptions',       type: 'bool', label: 'Tutorial',                description: 'New players start with the tutorial.' },
          { key: 'InfinitySystemEnabled',      section: 'GameOptions',       type: 'bool', label: 'Infinity',                description: 'Uses the Infinity system. Off falls back to Omega.' },
          { key: 'DisableMissionXPBonuses',    section: 'CustomGameOptions', type: 'bool', label: 'No XP bonus on missions', description: 'Experience bonuses no longer raise mission rewards.' },
          { key: 'UsePrestigeLootTable',       section: 'CustomGameOptions', type: 'bool', label: 'Prestige loot table',     description: 'Replaces the starting costume prestige rewards with the loot table in game data.' },
          { key: 'EnableUltimatePrestige',     section: 'CustomGameOptions', type: 'bool', label: 'Ultimate prestige',       description: 'Prestige can be reset after reaching the cap.' },
        ],
      },
      {
        title: 'Systems',
        fields: [
          { key: 'TeamUpSystemEnabled',       section: 'GameOptions', type: 'bool', label: 'Team-ups',          description: 'Turns on team-ups.' },
          { key: 'AchievementsEnabled',       section: 'GameOptions', type: 'bool', label: 'Achievements',      description: 'Turns on achievements.' },
          { key: 'OmegaMissionsEnabled',      section: 'GameOptions', type: 'bool', label: 'Shared quests',     description: 'Turns on shared quests.' },
          { key: 'VeteranRewardsEnabled',     section: 'GameOptions', type: 'bool', label: 'Login rewards',     description: 'Turns on login rewards.' },
          { key: 'LeaderboardsEnabled',       section: 'GameOptions', type: 'bool', label: 'Leaderboards',      description: 'Turns on leaderboards.' },
          { key: 'IsDifficultySliderEnabled', section: 'GameOptions', type: 'bool', label: 'Difficulty choice', description: 'Players can pick a region\'s difficulty.' },
        ],
      },
      {
        title: 'Loot and items',
        fields: [
          { key: 'DisableInstancedLoot',                section: 'CustomGameOptions', type: 'bool',   label: 'Shared loot',                description: 'Loot is free for all instead of per player.' },
          { key: 'LootSpawnGridCellRadius',             section: 'CustomGameOptions', type: 'number', label: 'Loot spacing',               description: 'Spacing of dropped loot, from 16 to 128.', min: 16, max: 128 },
          { key: 'TrashedItemExpirationTimeMultiplier', section: 'CustomGameOptions', type: 'number', label: 'Trashed item time',          description: 'Multiplies how long trashed items stay on the ground.', min: 0 },
          { key: 'DisableAccountBinding',               section: 'CustomGameOptions', type: 'bool',   label: 'No account binding',         description: 'Items no longer bind to the account on pickup.' },
          { key: 'DisableCharacterBinding',             section: 'CustomGameOptions', type: 'bool',   label: 'No character binding',       description: 'Items no longer bind to the hero on equip.' },
          { key: 'EnableCreditChestConversion',         section: 'CustomGameOptions', type: 'bool',   label: 'Credit chest conversion',    description: 'Players can turn credits into sellable chests with !item creditchest.' },
          { key: 'CreditChestConversionMultiplier',     section: 'CustomGameOptions', type: 'number', label: 'Chest conversion cost',      description: 'Multiplies the credit cost of a chest.', min: 0 },
        ],
      },
    ],
  },
  {
    id: 'gamedata',
    label: 'Game data',
    subsections: [
      {
        title: 'Loading',
        fields: [
          { key: 'LoadAllPrototypes',          section: 'GameData', type: 'bool', label: 'Load all game data',   description: 'Loads all game data at start. Slower start, no lag spikes the first time an area loads.' },
          { key: 'UseEquipmentSlotTableCache', section: 'GameData', type: 'bool', label: 'Equipment slot cache', description: 'Caches the equipment slot table. Slows the start unless all game data is loaded.' },
          { key: 'LoadLocaleFiles',            section: 'GameData', type: 'bool', label: 'Locale files',         description: 'Loads locale files from Data\\Game\\Loco.' },
        ],
      },
      {
        title: 'Patches and Live Tuning',
        fields: [
          { key: 'EnablePatchManager',     section: 'GameData', type: 'bool', label: 'Data patches',          description: 'Applies the files in Data\\Game\\Patches. Off also drops the stock fixes for mission bugs and prototype errors.' },
          { key: 'EnableLiveTuningEvents', section: 'GameData', type: 'bool', label: 'Live Tuning events',    description: 'Runs events by date when Live Tuning loads.' },
          { key: 'AutoRefreshLiveTuning',  section: 'GameData', type: 'bool', label: 'Daily Live Tuning reload', description: 'Reloads Live Tuning at the daily mission reset. Needs Live Tuning events.' },
        ],
      },
    ],
  },
  {
    id: 'store',
    label: 'Store',
    subsections: [
      {
        title: 'Currency',
        fields: [
          { key: 'GazillioniteBalanceForNewAccounts', section: 'MTXStore', type: 'number', label: 'Starting G',              description: 'G new accounts get on their first login.', min: 0, int: true },
          { key: 'ESToGazillioniteConversionRatio',   section: 'MTXStore', type: 'number', label: 'G per Eternity Splinter', description: 'G given for each Eternity Splinter converted.' },
          { key: 'ESToGazillioniteConversionStep',    section: 'MTXStore', type: 'number', label: 'Conversion step',         description: 'Splinters are converted in steps of this size to avoid rounding errors.', min: 1, int: true },
          { key: 'ESCooldownOverrideMinutes',         section: 'CustomGameOptions', type: 'number', label: 'Splinter drop cooldown (min)', description: 'Replaces the Eternity Splinter drop cooldown. A negative value keeps the stock cooldown.' },
          { key: 'CombineESStacks',                   section: 'CustomGameOptions', type: 'bool',   label: 'Combine splinter stacks',      description: 'Splinters that drop together become one stack.' },
        ],
      },
      {
        title: 'Gifting',
        fields: [
          { key: 'GiftingEnabled',                  section: 'GameOptions', type: 'bool',   label: 'Store gifting',      description: 'Players can buy gifts for others in the store.' },
          { key: 'GiftingAccountAgeInDaysRequired', section: 'GameOptions', type: 'number', label: 'Account age (days)', description: 'Days since the account was made before it can gift.', min: 0, int: true },
          { key: 'GiftingAvatarLevelRequired',      section: 'GameOptions', type: 'number', label: 'Hero level',         description: 'Hero level needed to gift.', min: 0, int: true },
          { key: 'GiftingLoginCountRequired',       section: 'GameOptions', type: 'number', label: 'Days logged in',     description: 'Days the account must have logged in before it can gift.', min: 0, int: true },
          { key: 'GiftingOmegaLevelRequired',       section: 'MTXStore',    type: 'number', label: 'Omega level',        description: 'Omega level needed to buy gifts, when Omega is in use.', min: 0, int: true },
          { key: 'GiftingInfinityLevelRequired',    section: 'MTXStore',    type: 'number', label: 'Infinity level',     description: 'Infinity level needed to buy gifts, when Infinity is in use.', min: 0, int: true },
        ],
      },
      {
        title: 'Store pages',
        fields: [
          { key: 'HomePageUrl',           section: 'MTXStore', type: 'string', label: 'Home page',       description: 'Page shown on the Home tab.' },
          { key: 'HomeBannerPageUrl',     section: 'MTXStore', type: 'string', label: 'Home banner',     description: 'Banner above the Home tab.' },
          { key: 'HeroesBannerPageUrl',   section: 'MTXStore', type: 'string', label: 'Heroes banner',   description: 'Banner above the Heroes tab.' },
          { key: 'CostumesBannerPageUrl', section: 'MTXStore', type: 'string', label: 'Costumes banner', description: 'Banner above the Costumes tab.' },
          { key: 'BoostsBannerPageUrl',   section: 'MTXStore', type: 'string', label: 'Items banner',    description: 'Banner above the Items tab.' },
          { key: 'ChestsBannerPageUrl',   section: 'MTXStore', type: 'string', label: 'Cards banner',    description: 'Banner above the Cards tab.' },
          { key: 'SpecialsBannerPageUrl', section: 'MTXStore', type: 'string', label: 'Specials banner', description: 'Banner above the Specials tab.' },
          { key: 'RealMoneyUrl',          section: 'MTXStore', type: 'string', label: 'Add G page',      description: 'Page shown in the Add G window.' },
        ],
      },
      {
        title: 'Bundles',
        fields: [
          { key: 'RewriteOriginalBundleUrls', section: 'MTXStore', type: 'bool',   label: 'Rewrite bundle URLs', description: 'Points bundle links that go to the old Gazillion CDN at the URLs below.' },
          { key: 'BundleInfoUrl',             section: 'MTXStore', type: 'string', label: 'Bundle info URL',     description: 'Replaces the start of bundle info links.' },
          { key: 'BundleImageUrl',            section: 'MTXStore', type: 'string', label: 'Bundle image URL',    description: 'Replaces the start of bundle image links.' },
        ],
      },
    ],
  },
  {
    id: 'logging',
    label: 'Logging',
    subsections: [
      {
        title: 'General',
        fields: [
          { key: 'EnableLogging',            section: 'Logging', type: 'bool', label: 'Logging',               description: 'Main switch for all server logging.' },
          { key: 'HideSensitiveInformation', section: 'Logging', type: 'bool', label: 'Hide sensitive info',   description: 'Hides email and IP addresses in logs.' },
        ],
      },
      {
        title: 'Console',
        fields: [
          { key: 'EnableConsole',            section: 'Logging', type: 'bool',   label: 'Console output', description: 'Sends log messages to the console shown in the Server tab.' },
          { key: 'ConsoleIncludeTimestamps', section: 'Logging', type: 'bool',   label: 'Timestamps',     description: 'Adds the time to each console message.' },
          { key: 'ConsoleMinLevel',          section: 'Logging', type: 'select', label: 'Lowest level',   description: 'Least severe level shown in the console.', options: LOG_LEVELS },
          { key: 'ConsoleMaxLevel',          section: 'Logging', type: 'select', label: 'Highest level',  description: 'Most severe level shown in the console.', options: LOG_LEVELS },
          { key: 'ConsoleChannels',          section: 'Logging', type: 'string', label: 'Channels',       description: 'Message sources the console accepts.' },
        ],
      },
      {
        title: 'File',
        fields: [
          { key: 'EnableFile',            section: 'Logging', type: 'bool',   label: 'File output',  description: 'Writes log messages to files in the server folder.' },
          { key: 'FileIncludeTimestamps', section: 'Logging', type: 'bool',   label: 'Timestamps',   description: 'Adds the time to each file message.' },
          { key: 'FileMinLevel',          section: 'Logging', type: 'select', label: 'Lowest level', description: 'Least severe level written to file.', options: LOG_LEVELS },
          { key: 'FileMaxLevel',          section: 'Logging', type: 'select', label: 'Highest level', description: 'Most severe level written to file.', options: LOG_LEVELS },
          { key: 'FileChannels',          section: 'Logging', type: 'string', label: 'Channels',     description: 'Message sources the file accepts.' },
          { key: 'FileSplitOutput',       section: 'Logging', type: 'bool',   label: 'Split by category', description: 'Writes a separate file for each message category.' },
        ],
      },
    ],
  },
  {
    // Filled in by ConfigPanel from whatever Config.ini and ConfigOverride.ini hold beyond the pages above
    id: 'all',
    label: 'All settings',
  },
  {
    id: 'multiverse',
    label: 'Multiverse',
  },
]

/** "Section.Key" for every key a named page above already shows. */
const SCHEMA_KEYS = new Set(
  CONFIG_SCHEMA.flatMap(p => [...(p.fields ?? []), ...(p.subsections ?? []).flatMap(s => s.fields)])
    .map(f => `${f.section}.${f.key}`)
)

type IniData = Record<string, Record<string, string>>

/** Keys a version without the Infinity system (1.48) doesn't read. */
const NEEDS_INFINITY = new Set(['GameOptions.InfinitySystemEnabled', 'GameOptions.IsDifficultySliderEnabled'])

/** The fields to show for the active version. */
export function fieldsFor(fields: Field[], hasInfinity: boolean): Field[] {
  return hasInfinity ? fields : fields.filter(f => !NEEDS_INFINITY.has(`${f.section}.${f.key}`))
}

/** Why the server can't use this value, or null if it can. The server crashes on a bad port or bind IP and skips other bad numbers. */
export function fieldError(field: Field, value: string): string | null {
  const v = value.trim()
  if (field.ip) {
    const parts = v.split('.')
    const v4 = parts.length === 4 && parts.every(p => /^\d{1,3}$/.test(p) && Number(p) <= 255)
    const v6 = v.includes(':') && /^[0-9a-f:.]+$/i.test(v)
    return v4 || v6 ? null : 'Enter an IP address like 127.0.0.1'
  }
  if (field.type !== 'number') return null
  if (field.int ? !/^-?\d+$/.test(v) : v === '' || !Number.isFinite(Number(v))) {
    return field.int ? 'Enter a whole number' : 'Enter a number'
  }
  const n = Number(v)
  if ((field.min !== undefined && n < field.min) || (field.max !== undefined && n > field.max)) {
    return field.min !== undefined && field.max !== undefined ? `Enter a number from ${field.min} to ${field.max}`
      : field.min !== undefined ? `Enter a number of at least ${field.min}`
      : `Enter a number of at most ${field.max}`
  }
  return null
}

/** Every key whose value on screen differs from the saved one, across all pages. Missing counts as ''. */
export function changedValues(values: IniData, saved: IniData): IniData {
  const out: IniData = {}
  for (const [section, keys] of Object.entries(values)) {
    for (const [key, val] of Object.entries(keys)) {
      if (val !== (saved[section]?.[key] ?? '')) (out[section] ??= {})[key] = val
    }
  }
  return out
}

/** The All settings page: every key the named pages don't show, grouped by INI section. */
export function otherSettings(values: IniData, defaults: IniData, descriptions: IniData): SubSection[] {
  const sections = [...new Set([...Object.keys(defaults), ...Object.keys(values)])].sort()
  return sections.map(section => {
    const keys = [...new Set([...Object.keys(defaults[section] ?? {}), ...Object.keys(values[section] ?? {})])]
      .filter(key => !SCHEMA_KEYS.has(`${section}.${key}`))
      .sort()
    const fields: Field[] = keys.map(key => {
      const sample = (defaults[section]?.[key] ?? values[section]?.[key] ?? '').toLowerCase()
      return {
        key, section, label: key,
        type: sample === 'true' || sample === 'false' ? 'bool' : 'string',
        description: descriptions[section]?.[key] ?? '',
      }
    })
    return { title: section, fields }
  }).filter(s => s.fields.length)
}
