// ─── Types ───────────────────────────────────────────────────────────────────

export type FieldType = 'bool' | 'number' | 'string' | 'textarea'

export interface Field {
  key: string
  section: string
  type: FieldType
  label: string
  description: string
  min?: number
  max?: number
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

// ─── Config schema ───────────────────────────────────────────────────────────

export const CONFIG_SCHEMA: NavSection[] = [
  {
    id: 'server',
    label: 'Server',
    subsections: [
      {
        title: 'Frontend',
        fields: [
          { key: 'BindIP',        section: 'Frontend', type: 'string', label: 'Bind IP',        description: 'IP address the frontend server binds to. Set to 0.0.0.0 to listen on all interfaces.' },
          { key: 'Port',          section: 'Frontend', type: 'number', label: 'Port',            description: 'Port for the game client to connect to.', min: 1, max: 65535 },
          { key: 'PublicAddress', section: 'Frontend', type: 'string', label: 'Public Address',  description: 'Address clients use to reach this server. Can be an IP (e.g. 192.168.1.2) or hostname.' },
        ],
      },
      {
        title: 'Web Frontend',
        fields: [
          { key: 'Address',         section: 'WebFrontend', type: 'string', label: 'Address',          description: 'Address the web frontend listens on.' },
          { key: 'Port',            section: 'WebFrontend', type: 'number', label: 'Port',              description: 'Port for the web dashboard and API.', min: 1, max: 65535 },
          { key: 'EnableDashboard', section: 'WebFrontend', type: 'bool',   label: 'Enable Dashboard',  description: 'Enables the web dashboard accessible via browser. Requires EnableWebApi to be true.' },
        ],
      },
      {
        title: 'Identity & Access',
        fields: [
          { key: 'ServerName',               section: 'GroupingManager', type: 'string',   label: 'Server Name',           description: 'Name shown in chat for system messages sent by the server.' },
          { key: 'MotdText',                 section: 'GroupingManager', type: 'textarea', label: 'Message of the Day',    description: 'Message broadcast to players on login.' },
          { key: 'ServerPrestigeLevel',      section: 'GroupingManager', type: 'number',   label: 'Server Name Colour',    description: 'Colour of the server name in chat. 0=white 1=green 2=blue 3=purple 4=orange 5=red 6=yellow (cosmic).', min: 0, max: 6 },
          { key: 'UseWhitelist',             section: 'PlayerManager',   type: 'bool',     label: 'Use Whitelist',         description: 'When enabled, only accounts added via !account whitelist can log in.' },
          { key: 'ServerCapacity',           section: 'PlayerManager',   type: 'number',   label: 'Server Capacity',       description: 'Maximum concurrent players. 0 = unlimited. Players over capacity are queued.', min: 0 },
          { key: 'LoadAllPrototypes',        section: 'GameData',        type: 'bool',     label: 'Load All Prototypes',   description: 'Preloads all game data on startup. Makes the server start slower but eliminates in-game lag spikes when new areas are loaded for the first time.' },
          { key: 'UseEquipmentSlotTableCache', section: 'GameData',      type: 'bool',     label: 'Equipment Slot Cache',  description: 'Caches the equipment slot table. Slower startup unless used alongside Load All Prototypes.' },
        ],
      },
    ],
  },
  {
    id: 'persistence',
    label: 'Persistence',
    subsections: [
      {
        title: 'Player Manager',
        fields: [
          { key: 'EnablePersistence',          section: 'PlayerManager', type: 'bool', label: 'Enable Persistence',            description: 'Saves player data between sessions. Disable for a fresh-start-every-time experience.' },
          { key: 'AllowClientVersionMismatch', section: 'PlayerManager', type: 'bool', label: 'Allow Version Mismatch',         description: 'Allows clients whose game version does not match the server to connect.' },
          { key: 'UseJsonDBManager',           section: 'PlayerManager', type: 'bool', label: 'Use JSON Backend',              description: 'Use JSON file instead of SQLite for player data. Supports only a single account.' },
          { key: 'AutosaveIntervalMinutes',    section: 'CustomGameOptions', type: 'number', label: 'Autosave Interval (min)', description: 'How often player data is saved outside of region transfers. Set to 0 or less to disable autosaving.', min: -1 },
        ],
      },
      {
        title: 'SQLite DB',
        fields: [
          { key: 'FileName',              section: 'SQLiteDBManager', type: 'string', label: 'Database File',         description: 'SQLite database filename, relative to the server\'s Data directory.' },
          { key: 'MaxBackupNumber',       section: 'SQLiteDBManager', type: 'number', label: 'Max Backups',           description: 'Maximum number of backup files to keep. 0 disables backups.', min: 0 },
          { key: 'BackupIntervalMinutes', section: 'SQLiteDBManager', type: 'number', label: 'Backup Interval (min)', description: 'Minimum time in minutes between automatic backups.', min: 1 },
        ],
      },
      {
        title: 'JSON DB',
        fields: [
          { key: 'FileName',              section: 'JsonDBManager', type: 'string', label: 'Save File',             description: 'JSON save filename, relative to the server\'s Data directory.' },
          { key: 'MaxBackupNumber',       section: 'JsonDBManager', type: 'number', label: 'Max Backups',           description: 'Maximum number of backup files to keep. 0 disables backups.', min: 0 },
          { key: 'BackupIntervalMinutes', section: 'JsonDBManager', type: 'number', label: 'Backup Interval (min)', description: 'Minimum time in minutes between automatic backups.', min: 1 },
          { key: 'PlayerName',            section: 'JsonDBManager', type: 'string', label: 'Player Name',           description: 'Player name assigned to the single account when using the JSON backend.' },
        ],
      },
      {
        title: 'Leaderboards',
        fields: [
          { key: 'DatabaseFile',           section: 'Leaderboards', type: 'string', label: 'Database File',          description: 'Leaderboard SQLite filename relative to Data/Leaderboards.' },
          { key: 'ScheduleFile',           section: 'Leaderboards', type: 'string', label: 'Schedule File',          description: 'Leaderboard schedule JSON filename relative to Data/Leaderboards.' },
          { key: 'AutoSaveIntervalMinutes',section: 'Leaderboards', type: 'number', label: 'Autosave Interval (min)',description: 'Minimum time in minutes between leaderboard autosaves.', min: 1 },
        ],
      },
    ],
  },
  {
    id: 'gameplay',
    label: 'Gameplay',
    fields: [
      { key: 'AutoUnlockAvatars',                section: 'CustomGameOptions', type: 'bool',   label: 'Auto Unlock Heroes',             description: 'Automatically unlocks all heroes for players who complete the tutorial.' },
      { key: 'AutoUnlockTeamUps',                section: 'CustomGameOptions', type: 'bool',   label: 'Auto Unlock Team-Ups',           description: 'Automatically unlocks all team-ups for players who complete the tutorial.' },
      { key: 'ESCooldownOverrideMinutes',        section: 'CustomGameOptions', type: 'number', label: 'ES Drop Cooldown (min)',          description: 'Overrides the Eternity Splinter drop cooldown duration. Set to a negative value to use the default.' },
      { key: 'CombineESStacks',                  section: 'CustomGameOptions', type: 'bool',   label: 'Combine ES Stacks',              description: 'Merges multiple Eternity Splinter stacks into a single item when they drop at the same time.' },
      { key: 'DisableMovementPowerChargeCost',   section: 'CustomGameOptions', type: 'bool',   label: 'No Movement Power Charge Cost',  description: 'Removes charge costs for movement powers. Imitates pre-Biggest Update Ever behaviour.' },
      { key: 'AllowSameGroupTalents',            section: 'CustomGameOptions', type: 'bool',   label: 'Allow Same-Group Talents',       description: 'Allows mutually exclusive talents to be enabled at the same time.' },
      { key: 'EnableCreditChestConversion',      section: 'CustomGameOptions', type: 'bool',   label: 'Enable Credit Chest Conversion', description: 'Allows players to convert credits to sellable chest items via the !item creditchest command.' },
      { key: 'CreditChestConversionMultiplier',  section: 'CustomGameOptions', type: 'number', label: 'Chest Conversion Multiplier',    description: 'Credit cost multiplier when converting credits to chest items.', min: 0 },
      { key: 'DisableAccountBinding',            section: 'CustomGameOptions', type: 'bool',   label: 'Disable Account Binding',        description: 'Disables account-bound-on-pickup for items.' },
      { key: 'DisableCharacterBinding',          section: 'CustomGameOptions', type: 'bool',   label: 'Disable Character Binding',      description: 'Disables character-bound-on-equip for items.' },
      { key: 'UsePrestigeLootTable',             section: 'CustomGameOptions', type: 'bool',   label: 'Prestige Loot Table',            description: 'Replaces the starting costume prestige reward with items from the loot table.' },
      { key: 'EnableUltimatePrestige',           section: 'CustomGameOptions', type: 'bool',   label: 'Enable Ultimate Prestige',       description: 'Allows prestige level to be reset after reaching the prestige level cap.' },
    ],
  },
  {
    id: 'store',
    label: 'Store',
    fields: [
      { key: 'GazillioniteBalanceForNewAccounts', section: 'MTXStore', type: 'number', label: 'Starting G Balance',            description: 'Amount of Gs (Gazillionite) new accounts receive on first login.', min: 0 },
      { key: 'ESToGazillioniteConversionRatio',   section: 'MTXStore', type: 'number', label: 'ES to G Conversion Ratio',      description: 'Amount of Gs awarded per Eternity Splinter when converting.' },
      { key: 'ESToGazillioniteConversionStep',    section: 'MTXStore', type: 'number', label: 'ES to G Conversion Step',       description: 'Eternity Splinter step size for conversion, used to avoid rounding errors.', min: 1 },
      { key: 'GiftingOmegaLevelRequired',         section: 'MTXStore', type: 'number', label: 'Omega Level for Gifting',       description: 'Minimum Omega level required to purchase gifts for other players. 0 = no requirement.', min: 0 },
      { key: 'GiftingInfinityLevelRequired',      section: 'MTXStore', type: 'number', label: 'Infinity Level for Gifting',    description: 'Minimum Infinity level required to purchase gifts for other players. 0 = no requirement.', min: 0 },
    ],
  },
  {
    id: 'logging',
    label: 'Logging',
    fields: [
      { key: 'EnableLogging',           section: 'Logging', type: 'bool',   label: 'Enable Logging',              description: 'Master switch for the logging system.' },
      { key: 'HideSensitiveInformation',section: 'Logging', type: 'bool',   label: 'Hide Sensitive Info',         description: 'Masks email addresses and IP addresses in log output.' },
      { key: 'EnableConsole',           section: 'Logging', type: 'bool',   label: 'Console Output',              description: 'Outputs log messages to the console (captured here in MH Multiverse).' },
      { key: 'ConsoleIncludeTimestamps',section: 'Logging', type: 'bool',   label: 'Console Timestamps',          description: 'Includes message timestamps in console output.' },
      { key: 'ConsoleMinLevel',         section: 'Logging', type: 'number', label: 'Console Min Level',           description: 'Minimum log level for console output. 0=trace 1=debug 2=info 3=warn 4=error 5=fatal.', min: 0, max: 5 },
      { key: 'ConsoleMaxLevel',         section: 'Logging', type: 'number', label: 'Console Max Level',           description: 'Maximum log level for console output.', min: 0, max: 5 },
      { key: 'EnableFile',              section: 'Logging', type: 'bool',   label: 'File Output',                 description: 'Outputs log messages to a file in the server directory.' },
      { key: 'FileIncludeTimestamps',   section: 'Logging', type: 'bool',   label: 'File Timestamps',             description: 'Includes message timestamps in file output.' },
      { key: 'FileMinLevel',            section: 'Logging', type: 'number', label: 'File Min Level',              description: 'Minimum log level for file output.', min: 0, max: 5 },
      { key: 'FileMaxLevel',            section: 'Logging', type: 'number', label: 'File Max Level',              description: 'Maximum log level for file output.', min: 0, max: 5 },
      { key: 'FileSplitOutput',         section: 'Logging', type: 'bool',   label: 'Split File Output',           description: 'Splits log file output into separate files based on message category.' },
    ],
  },
  {
    id: 'multiverse',
    label: 'MH Multiverse',
  }
]
