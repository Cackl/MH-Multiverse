# MH Multiverse — Architecture

## Tech Stack

| Layer | Technology |
|---|---|
| App framework | Tauri 2 |
| Frontend | Svelte 5, Vite 8 |
| Backend | Rust (stable, 1.77.2+) |
| Credential encryption | `aes-gcm` 0.10 via OS keychain (`keyring` 2) |
| Process management | `sysinfo`, Windows Job Objects (`windows` crate) |
| Calligraphy parsing | `lz4_flex` (LZ4 block decompression) |
| Server updates | `reqwest` (HTTP streaming), `zip` (extraction) |
| Timestamps | `chrono` |
| SQLite | `rusqlite` (Account.db read-only access) |
| Log parsing | `regex` (player login/logout extraction) |

---

## Project Structure

```
mh-multiverse/
├── src/
│   ├── App.svelte                Root - tab router, mounts config + event bridge
│   ├── app.css                   Design tokens, CSS variables, data-theme variants
│   ├── main.ts                   Svelte 5 entry (mount())
│   ├── vite-env.d.ts             TypeScript ambient declarations
│   ├── lib/
│   │   ├── store.ts              Global stores, types, Tauri invoke wrappers
│   │   ├── serverEvents.ts       Tauri event listeners (log, start, stop, player-event)
│   │   ├── serverCommands.ts     Fallback command list for autocomplete
│   │   ├── playerMeta.ts         PlayerSession type, user level labels, ban/whitelist flag helpers
│   │   ├── tuningMeta.ts         Tuning enum prefixes, category maps, known file sets
│   │   └── catalogMeta.ts        Catalog type interfaces, type/modifier metadata, categories
│   └── components/
│       ├── TitleBar.svelte        Custom window chrome (drag region, min/max/close)
│       ├── Rail.svelte            Left nav rail (MHO / Local / App groups)
│       ├── PanelSidebar.svelte    Reusable sidebar layout wrapper
│       ├── LaunchPanel.svelte     Server profiles, credentials, game launch
│       ├── ServerPanel.svelte     Server start/stop, log viewer, command input, online players
│       ├── PlayersBlade.svelte    Searchable list of players connected to the server
│       ├── PlayerCard.svelte      Per-player moderation/admin card (kick, ban, user level, info)
│       ├── ConfigPanel.svelte     INI editor (Config.ini / ConfigOverride.ini)
│       ├── DataPanel.svelte       Sub-tab container for Events / Tuning / Store / Patches
│       ├── EventsPanel.svelte     Event schedule dashboard, rule list, definition list
│       ├── EventRuleEditorModal.svelte   Schedule rule editor (type, days, events list)
│       ├── EventDefinitionEditorModal.svelte  Event definition editor (id, display name, file path, etc.)
│       ├── TuningPanel.svelte     LiveTuning file list, tags, favourites
│       ├── TuningEditorModal.svelte  Per-file tuning entry editor
│       ├── StorePanel.svelte      Catalog entry list, filters, bulk operations
│       ├── StoreEditorModal.svelte   Per-entry catalog editor, item picker, HTML gen
│       ├── PatchesPanel.svelte    PatchData file list, enable/disable
│       ├── PatchEditorModal.svelte   Per-file patch entry editor
│       ├── OpsPanel.svelte        Server update, backup create/restore/delete
│       ├── ServerModal.svelte     Add/edit server dialog, Local/HTTPS toggles
│       └── AppPanel.svelte        Settings: exe paths, launch options, themes, about
│
├── src-tauri/
│   ├── src/
│   │   ├── main.rs               Entry point - calls app_lib::run()
│   │   ├── lib.rs                Tauri builder: plugins, managed state, command registration,
│   │   │                         window close hook, window state persistence
│   │   ├── config.rs             AppConfig, Server, LaunchOptions, ShutdownConfig structs;
│   │   │                         AES-256-GCM encryption; keychain key management;
│   │   │                         multiverse.json persistence; all config Tauri commands
│   │   ├── server.rs             ServerProcess/ServerState, PlayerState, DbPath managed state;
│   │   │                         start/stop MHServerEmu + Apache; stdout/stderr log streaming
│   │   │                         with batched emission; player login/logout parsing from stdout;
│   │   │                         Account.db SQLite lookup at login time; Job Object lifecycle;
│   │   │                         process exit watcher
│   │   ├── events.rs             Events.json / EventSchedule.json read/write; override file
│   │   │                         management (load, save, reset, merge) for both definitions
│   │   │                         and schedule rules
│   │   ├── launcher.rs           launch_game (spawn with args), game_is_running (sysinfo poll)
│   │   ├── ini.rs                Config.ini / ConfigOverride.ini read/write with diff-only saving
│   │   ├── tuning.rs             LiveTuningData*.json scan/read/write/create/toggle
│   │   ├── patches.rs            PatchData*.json scan/load/save/create/toggle
│   │   ├── store.rs              Catalog*.json load/save/delete, display name resolution,
│   │   │                         u64 precision handling, bundle HTML generation
│   │   ├── calligraphy.rs        Calligraphy.sip pak reader, blueprint/prototype directory
│   │   │                         parsing, prototype search, ID/GUID/path resolution
│   │   └── updater.rs            Nightly build update (download/extract/install),
│   │                             backup create/list/restore/delete with manifests
│   ├── assets/
│   │   └── display_names.json    Embedded prototype path → display name map (~260KB)
│   ├── capabilities/
│   │   └── default.json          Tauri permission grants
│   ├── Cargo.toml
│   └── tauri.conf.json           Window config (1100×700, min 900×580, decorations: false)
│
├── package.json
├── vite.config.ts                Injects __APP_VERSION__ from tauri.conf.json
├── svelte.config.js
├── tsconfig.app.json
├── tsconfig.node.json
└── tsconfig.json
```

---

## Frontend Architecture

### Routing

The app uses a flat conditional routing model driven by two stores:

- `activeTab` (`Tab`): selects the top-level panel - `launch`, `server`, `config`, `data`, `ops`, `settings`
- `activeDataTab` (`DataTab`): selects within the Data panel - `events`, `tuning`, `store`, `patches`

`App.svelte` renders the active panel inside a fixed layout of `TitleBar` + `Rail` + content area. `DataPanel.svelte` is a sub-router that renders a secondary tab bar and conditionally mounts `EventsPanel`, `TuningPanel`, `StorePanel`, or `PatchesPanel`.

There is no URL-based routing. Tab state is in-memory only and resets to `launch` on app restart.

### Window Chrome

`decorations: false` in `tauri.conf.json` disables native window decorations. `TitleBar.svelte` provides a custom title bar with `data-tauri-drag-region` for window dragging and manual minimize/maximize/close buttons that call Tauri's window API.

Window size and position are persisted via `tauri-plugin-window-state`, filtered to `SIZE | POSITION` flags only to avoid conflicts with the custom titlebar.

### Store Layer (`src/lib/store.ts`)

All shared frontend state lives in Svelte writable stores. The store module also exports async wrapper functions that optimistically update local state, then invoke the corresponding Tauri command:

```
setTheme(theme)  →  activeTheme.set(theme)  →  invoke('set_theme', { theme })
```

This pattern is consistent across `setGameExe`, `setServerExe`, `setLaunchOptions`, `setShutdownConfig`, `setTuningTags`, `setTuningFavourites`, `setBackupTargets`, `setStoreHtmlOutputDir`, and `setConsolePresets`.

`loadConfig()` is called once on mount. It invokes `get_config`, populates `appConfig`, applies the saved theme to the DOM, and sets `activeServerId`.

`setSchedulerNow(dt)` accepts a UTC `Date` parsed from server log output, stores it in `schedulerNow`, and computes `eventTimezoneOffset` (integer hours) as the difference between the parsed server time and the local `Date.now()`. This offset is consumed by `EventsPanel` to evaluate schedule rules against server time rather than client local time.

### Event Bridge (`src/lib/serverEvents.ts`)

Server process events are received via Tauri's event system, not polling. The bridge is initialised once on mount and guards against duplicate initialisation using a `window.__mhmServerBridge` state object.

Four listeners are registered:

| Event | Source | Effect |
|---|---|---|
| `server-log` | Batched stdout/stderr from `server.rs` | Normalised and appended to `serverLog` store; each line is also checked for the scheduler-now pattern |
| `server-started` | Emitted after successful spawn | Sets `serverRunning`, starts uptime timer |
| `server-stopped` | Emitted on process exit (normal or crash) | Clears `serverRunning`/`apacheRunning`, stops uptime, logs exit code |
| `player-event` | Emitted by `server.rs` on player login/logout/clear | Consumed by `ServerPanel` to update the online player list |

`server-started` and `server-stopped` use a logTimestamp() function to mirror the formatting used for MHServerEmu entries

On bridge init, `syncInitialState()` polls `server_is_running` and `apache_is_running` to recover state if the frontend reloads while the server is running.

**Scheduler-now extraction**: each incoming log line is matched against the regex `Checking Live Tuning events (now=[...])`. When matched, the timestamp string (format `MM/DD/YYYY HH:MM:SS`, server-local time) is parsed as UTC and passed to `setSchedulerNow()`. This keeps `schedulerNow` and `eventTimezoneOffset` continuously up to date while the server is running.

### Metadata Modules

**`tuningMeta.ts`** - maps tuning enum prefixes (e.g. `eGTV_`, `eWETV_`) to display categories and blueprint hints for prototype search scoping. Contains known file sets (`KNOWN_CORE`, `KNOWN_EVENTS`) for auto-tagging, and the full tuning setting reference with descriptions and default values.

**`catalogMeta.ts`** - defines TypeScript interfaces mirroring the Rust catalog types (PascalCase field names matching `serde(rename_all = "PascalCase")`). Contains the catalog type/modifier taxonomy, item category definitions with prototype path prefixes for the item picker, and helper functions for type inference and modifier construction.

**`playerMeta.ts`** - defines the `PlayerSession` type (mirroring the Rust struct serialised from `server.rs`), user level label maps and option arrays, and helper functions: `userLevelLabel(level)`, `isBanned(flags)` (flag value 2), `isWhitelisted(flags)` (flag value 16), `formatLastSeen(ts)` (handles .NET ticks, Unix ms, and Unix seconds).

**`serverCommands.ts`** - hardcoded fallback command list used for autocomplete in the command input. The `ServerPanel` has commented-out code for fetching commands from the server's `/Commands` endpoint at runtime, which is not yet implemented on the MHServerEmu side.

### Component Patterns

Panels follow a consistent layout: `PanelSidebar` on the left (file/item list with search and filters), detail/editor area on the right. Modal editors (`TuningEditorModal`, `StoreEditorModal`, `PatchEditorModal`, `EventRuleEditorModal`, `EventDefinitionEditorModal`) are mounted conditionally when editing state is set, and communicate back to their parent via `onClose`/`onSaved`/`onDeleted` callback props.

**`EventsPanel.svelte`** deviates from the standard sidebar/editor split. The sidebar lists schedule rules grouped by type (`AlwaysOn`, `DayOfWeek`, `WeeklyRotation`, `SpecialDate`), and the main area is a dashboard showing currently active events and a collapsible full event definition list. Selecting a rule or definition opens an inline editor area (not a separate modal) in place of the dashboard. Override status controls (Reset / Merge) for both Events and Schedule are rendered at the bottom of the sidebar.

**`PlayerCard.svelte`** is a self-contained card rendered per online player in `PlayersPanel.svelte` (accessed via a button in `ServerPanel`). It issues moderation commands (`!client kick`, `!account ban/unban`, `!account whitelist/unwhitelist`, `!client info`) and admin commands (`!account userlevel`) directly via `invoke('send_command', { cmd })`. Actions requiring an email address check `player.email` before proceeding and display inline feedback. Destructive actions (kick, ban) require a two-step confirmation UI within the card.

---

## Backend Architecture

### Managed State

Five state objects are registered via `.manage()` in `lib.rs`:

| State | Type | Purpose |
|---|---|---|
| `ServerState` | `Arc<Mutex<ServerProcess>>` | Owns child process handles for MHServerEmu and Apache, plus the Job Object handle |
| `PlayerState` | `Arc<Mutex<HashMap<String, PlayerSession>>>` | In-memory map of active player sessions, keyed by hex session ID |
| `DbPath` | `Arc<Mutex<Option<PathBuf>>>` | Path to `Account.db`, derived from `server_exe` and updated whenever `set_server_exe` is called |
| `CatalogueState` | `Mutex<Option<(String, PrototypeCatalogue)>>` | Cached Calligraphy.sip parse result, keyed by sip file path |
| `DisplayNameState` | Embedded + lazy-loaded maps | Prototype path → display name resolution (embedded JSON + optional per-server override file) |

### Rust Modules

**`config.rs`** - `AppConfig` is the root persisted configuration. Stored as `multiverse.json` in the OS app data directory (`%APPDATA%\com.mhmultiverse.app\`). Passwords are encrypted with AES-256-GCM; the 256-bit key is stored in and retrieved from the OS keychain via `keyring`. Each config-mutating command loads the full config from disk, modifies the relevant field, and writes back - there is no in-process config cache on the Rust side. `AppConfig` includes a `console_presets` field (`Vec<String>`) for persisted command shortcut strings shown in the server panel. Server struct includes is_local (bool) and use_https (bool) fields; is_local suppresses host storage and triggers Config.ini-derived URL building at launch time.

**`server.rs`** - Server lifecycle management and player session tracking. `start_server` spawns MHServerEmu with piped stdin/stdout/stderr and assigns it to a Windows Job Object (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`). Five background threads are spawned per server start: stdout reader, stderr reader, a batcher that collects log lines and emits them to the frontend in batches (up to 50 lines per 50ms flush interval), and a watcher thread that polls `try_wait()` every 150ms to detect process exit. `start_server` performs a pre-flight TcpStream::connect check against the configured WebFrontend Port before spawning, returning a clear error if the port is already in use.

The stdout reader additionally calls `parse_player_log_event` on every line; matches trigger `handle_player_log_event` which updates `PlayerState` and emits `player-event` to the frontend. On login, `lookup_account` opens a read-only SQLite connection to `Account.db` and queries the `Account`, `Player`, `Guild`, `GuildMember`, and `Avatar` tables to populate the full `PlayerSession`. On server stop, `clear_player_state` empties `PlayerState` and emits a `player-event` with `kind: "clear"`. `stop_server` writes `!server shutdown\n` to stdin and falls back to a hard kill after 10 seconds. Apache is managed independently via `start_apache`/`stop_apache`. `DbPath.set_from_server_exe` derives `<server_exe_dir>/Data/Account.db` and is called both when `set_server_exe` is invoked from config and when `start_server` is called.

**`events.rs`** - File I/O for `Events.json` / `EventsOverride.json` and `EventSchedule.json` / `EventScheduleOverride.json` in `Data/Game/LiveTuning/`. Both file pairs follow the same override pattern: the override file takes precedence if it exists, otherwise the default file is read. `Events.json` is a JSON object keyed by event ID; `EventSchedule.json` is a JSON array. Read functions deserialise into `EventDefinition` and `ScheduleRule` structs respectively. Write functions always target the override file. `reset_*_override` copies the default file over the override; `merge_*_override` adds any entries present in the default but missing from the override (by ID for events, by name for rules), without overwriting existing override entries.

**`launcher.rs`** - Spawns the game client as a detached process with command-line arguments derived from the active server profile and launch options. Credentials are decrypted from the config at launch time. For local servers, the siteconfigurl host is derived at launch time from WebFrontend.Port in Config.ini via ini::read_merged_value. For remote servers, normalize_host strips any scheme and path from the stored host before building the URL; use_https determines the scheme. game_is_running uses sysinfo to check for a MarvelHeroesOmega.exe process.

**`ini.rs`** - Reads `Config.ini` (defaults) and `ConfigOverride.ini` (overrides), merges them, and returns the merged values plus a set of which keys are overridden. Writes use diff-only logic: values matching the default are removed from the override file; only differing values are written. Also exposes `read_merged_value` (non-command pub fn) for reading a single merged key by section, used by `server.rs` for the port pre-check.

**`tuning.rs`** - File I/O for `LiveTuningData*.json` files in `Data/Game/LiveTuning`. The enable/disable toggle works by renaming files with an `OFF_` prefix. JSON is read/written using PascalCase field names (`Prototype`, `Setting`, `Value`) to match MHServerEmu's expected format. Handles UTF-8 BOM stripping.

**`patches.rs`** - File I/O for `PatchData*.json` files in `Data/Game/Patches`. Enable/disable works by moving files between `Patches/` and `Patches/Off/`. Patch entries use `serde_json::Value` for the `Value` field to support arbitrary JSON value types.

**`store.rs`** - Catalog entry management for `Catalog*.json` in `Data/Game/MTXStore`. Uses a base/MODIFIED file separation: base files are read-only from MH Multiverse's perspective, and all edits go to `*MODIFIED.json` sidecar files. MODIFIED entries override base entries with the same `SkuId`. The module handles u64 precision by using a dual type system: `CatalogEntryDisk` (raw u64 for on-disk JSON) and `CatalogEntry` (String for the JS boundary). Display name resolution chains through: custom override file → embedded `display_names.json` → prototype path → raw ID. Also generates HTML/CSS bundle pages for in-game store display.

**`calligraphy.rs`** - Reads MHServerEmu's `Calligraphy.sip` pak file (LZ4-compressed entries with a `KAPG` signature). Parses the `Blueprint.directory` and `Prototype.directory` files within the pak to build a `PrototypeCatalogue` with indices by runtime ID, path, and GUID. The catalogue is cached per sip file path and rebuilt automatically if the server executable changes. Prototype search supports multi-prefix category filtering and case-insensitive matching against both path and display name.

**`updater.rs`** - Downloads nightly builds from `nightly.link/Crypto137/MHServerEmu/...`, extracts them, and overlays onto the server directory. The update flow: check availability (HTTP range probe) → backup selected targets → download with progress events → extract to staging dir → detect wrapper directory → copy to server dir → restore backed-up user files. Backups are stored in `{server_dir}/Backups/{timestamp}/` with a `manifest.json`. `Calligraphy.sip` and `mu_cdata.sip` are blacklisted from backups.

### Window Close Hook

`lib.rs` intercepts `CloseRequested`, prevents the default close, kills child processes via `kill_child()`, saves window state, then calls `app.exit(0)`. This runs in an async Tauri runtime task.

---

## Tauri Command Reference

### Config (`config.rs`)

| Command | Parameters | Returns | Description |
|---|---|---|---|
| `get_config` | - | `AppConfig` | Load config from `multiverse.json` |
| `cmd_save_config` | `config: AppConfig` | `()` | Write full config to disk |
| `upsert_server` | `server: Server, password: String` | `AppConfig` | Add/update server, encrypts password |
| `delete_server` | `server_id: String` | `AppConfig` | Remove server by ID |
| `set_active_server` | `server_id: String` | `()` | Update active server selection |
| `set_game_exe` | `path: String` | `()` | Set game executable path |
| `set_server_exe` | `path: String` | `()` | Set server executable path, updates DbPath |
| `set_theme` | `theme: String` | `()` | Set UI theme |
| `set_launch_options` | `options: LaunchOptions` | `()` | Set game launch flags |
| `set_shutdown_config` | `shutdown: ShutdownConfig` | `()` | Set shutdown delay and broadcast message |
| `set_tuning_tags` | `tags: HashMap<String, String>` | `()` | Set tuning file tag assignments |
| `set_tuning_favourites` | `favourites: Vec<String>` | `()` | Set pinned tuning filenames |
| `set_backup_targets` | `targets: Vec<String>` | `()` | Set backup target paths |
| `set_store_html_output_dir` | `dir: String` | `()` | Set bundle HTML output directory |
| `set_console_presets` | `presets: Vec<String>` | `()` | Set command shortcut preset list |

### Launcher (`launcher.rs`)

| Command | Parameters | Returns | Description |
|---|---|---|---|
| `launch_game` | `server_id: String` | `()` | Spawn game client with server args and launch options. siteconfigurl uses /Dashboard/SiteConfig.xml for patched local clients, /SiteConfig.xml for unpatched, normalize_host + use_https for remote |
| `game_is_running` | - | `bool` | Poll sysinfo for `MarvelHeroesOmega.exe` |

### Server (`server.rs`)

| Command | Parameters | Returns | Description |
|---|---|---|---|
| `start_server` | `server_exe: String` | `()` | Spawn MHServerEmu with log streaming and player tracking |
| `stop_server` | - | `()` | Graceful shutdown via stdin, 10s hard-kill fallback |
| `start_apache` | `server_exe: String` | `()` | Spawn Apache (derived path from server_exe) |
| `stop_apache` | - | `()` | Kill Apache process |
| `send_command` | `cmd: String` | `()` | Write to MHServerEmu stdin |
| `server_is_running` | - | `bool` | Check MHServerEmu child process via try_wait |
| `apache_is_running` | - | `bool` | Check Apache child process via try_wait |
| `get_players` | - | `Vec<PlayerSession>` | Return current online players sorted alphabetically |

### Events (`events.rs`)

| Command | Parameters | Returns | Description |
|---|---|---|---|
| `load_events` | `server_exe: String` | `EventsData` | Load event definitions (override preferred over default) |
| `load_event_schedule` | `server_exe: String` | `ScheduleData` | Load schedule rules (override preferred over default) |
| `save_events_override` | `server_exe: String, definitions: Vec<EventDefinition>` | `()` | Write full definitions list to EventsOverride.json |
| `save_schedule_override` | `server_exe: String, rules: Vec<ScheduleRule>` | `()` | Write full rules list to EventScheduleOverride.json |
| `reset_events_override` | `server_exe: String` | `EventsData` | Copy Events.json → EventsOverride.json, return result |
| `reset_schedule_override` | `server_exe: String` | `ScheduleData` | Copy EventSchedule.json → EventScheduleOverride.json, return result |
| `merge_events_override` | `server_exe: String` | `EventsData` | Add missing default events to override (by ID), return result |
| `merge_schedule_override` | `server_exe: String` | `ScheduleData` | Add missing default rules to override (by name), return result |

### INI (`ini.rs`)

| Command | Parameters | Returns | Description |
|---|---|---|---|
| `read_config` | `server_exe: String` | `ConfigState` | Merge Config.ini + ConfigOverride.ini |
| `write_config` | `server_exe: String, updates: IniData` | `()` | Diff-write overrides only |
| `reset_config_section` | `server_exe: String, section: String` | `()` | Remove section from ConfigOverride.ini |
| `get_config_dir` | `server_exe: String` | `String` | Return server directory path |

### Tuning (`tuning.rs`)

| Command | Parameters | Returns | Description |
|---|---|---|---|
| `scan_tuning_files` | `server_exe: String` | `Vec<TuningFileInfo>` | List LiveTuningData*.json files with enabled state |
| `read_tuning_file` | `server_exe: String, canonical_name: String` | `Vec<TuningEntry>` | Parse tuning entries from file |
| `write_tuning_file` | `server_exe: String, canonical_name: String, entries: Vec<TuningEntry>` | `()` | Write entries back to file |
| `create_tuning_file` | `server_exe: String, canonical_name: String, entries: Vec<TuningEntry>` | `()` | Create new tuning file |
| `toggle_tuning_file` | `server_exe: String, canonical_name: String, enabled: bool` | `()` | Rename with/without OFF_ prefix |
| `get_live_tuning_dir` | `server_exe: String` | `String` | Return LiveTuning directory path |

### Patches (`patches.rs`)

| Command | Parameters | Returns | Description |
|---|---|---|---|
| `scan_patch_files` | `server_exe: String` | `Vec<PatchFileInfo>` | List PatchData*.json in Patches/ and Patches/Off/ |
| `load_patch_file` | `server_exe: String, file_name: String, enabled: bool` | `Vec<PatchEntry>` | Parse patch entries from file |
| `save_patch_file` | `server_exe: String, file_name: String, enabled: bool, entries: Vec<PatchEntry>` | `()` | Write entries to file |
| `create_patch_file` | `server_exe: String, file_name: String` | `()` | Create empty patch file in Patches/ |
| `set_patch_file_enabled` | `server_exe: String, file_name: String, currently_enabled: bool` | `bool` | Move between Patches/ and Patches/Off/ |
| `get_patches_dir` | `server_exe: String` | `String` | Return Patches directory path |

### Store (`store.rs`)

| Command | Parameters | Returns | Description |
|---|---|---|---|
| `get_mtxstore_dir` | `server_exe: String` | `String` | Return MTXStore directory path |
| `list_catalog_files` | `server_exe: String` | `Vec<String>` | List base Catalog*.json filenames |
| `load_catalog_entries` | `server_exe: String` | `Vec<CatalogEntryWithMeta>` | Load all entries with base/MODIFIED merge |
| `save_catalog_entry` | `server_exe: String, entry: CatalogEntry, target_file: String` | `()` | Upsert entry into *MODIFIED.json |
| `delete_catalog_entry` | `server_exe: String, sku_id: u64, source_file: String, from_modified: bool` | `()` | Delete entry by SKU from target file |
| `get_next_sku_id` | `server_exe: String` | `u64` | Return max SKU + 1 (floor 1001) |
| `resolve_display_name` | `server_exe: String, prototype_runtime_id: String` | `String` | Resolve prototype ID to display name |
| `generate_bundle_html` | `server_exe: String, entry: CatalogEntry, output_dir: String` | `String` | Generate HTML bundle page, return file path |

### Calligraphy (`calligraphy.rs`)

| Command | Parameters | Returns | Description |
|---|---|---|---|
| `search_prototypes` | `server_exe, query, blueprint_hint?, category_path?, is_inventory_type?` | `Vec<PrototypeMatch>` | Search prototype paths in Calligraphy.sip |
| `lookup_prototype_id` | `server_exe: String, prototype_path: String` | `String` | Resolve path to runtime ID (decimal string) |

### Updater (`updater.rs`)

| Command | Parameters | Returns | Description |
|---|---|---|---|
| `check_update_available` | - | `UpdateInfo` | Probe nightly build URL availability |
| `run_update` | `server_exe: String, backup_targets: Vec<String>` | `()` | Full update flow: backup → download → extract → install → restore |
| `create_backup` | `server_exe: String, targets: Vec<String>, label: String` | `BackupManifest` | Create a named backup |
| `list_backups` | `server_exe: String` | `Vec<BackupManifest>` | List backups (newest first) |
| `restore_backup` | `server_exe: String, backup_id: String` | `()` | Restore backup by ID |
| `delete_backup` | `server_exe: String, backup_id: String` | `()` | Delete backup directory |
| `get_backups_dir` | `server_exe: String` | `String` | Return Backups directory path |

---

## Svelte Store Reference (`src/lib/store.ts`)

### State Stores

| Export | Type | Description |
|---|---|---|
| `activeTab` | `writable<Tab>` | Current top-level tab |
| `activeDataTab` | `writable<DataTab>` | Current data sub-tab (`events` / `tuning` / `store` / `patches`) |
| `serverRunning` | `writable<boolean>` | MHServerEmu process state |
| `gameRunning` | `writable<boolean>` | MarvelHeroesOmega.exe process state |
| `apacheRunning` | `writable<boolean>` | Apache process state |
| `uptimeSec` | `writable<number>` | Server uptime counter (seconds) |
| `serverError` | `writable<string>` | Current error message (empty = no error) |
| `serverLog` | `writable<LogLine[]>` | Log lines (capped at 2000) |
| `appConfig` | `writable<AppConfig>` | Full config including server list, launch options, etc. |
| `activeTheme` | `writable<string>` | Current theme ID |
| `activeServerId` | `writable<string>` | Currently selected server UUID |
| `tuningFocusFile` | `writable<string \| null>` | Canonical tuning filename to auto-select when switching to the Tuning tab (set by EventsPanel when navigating to a linked tuning file) |
| `schedulerNow` | `writable<Date \| null>` | Most recently parsed server-side "now" timestamp from log output (UTC) |
| `eventTimezoneOffset` | `writable<number>` | Offset in whole hours between server time and client UTC, derived from `schedulerNow`; used by EventsPanel to evaluate schedule rule active states |
| `logFilterThreshold` | `writeable<boolean>` | When true, log filter shows selected level and all more severe levels. When false, exact level match only. Persisted to local storage. |
| `LOG_LEVEL_SEVERITY` | `Record<LogLevel, number>` | Severity order map used by threshold filtering; trace(0) through fatal(6), all(-1) |

### Functions

| Export | Signature | Description |
|---|---|---|
| `startUptime` | `() => void` | Start uptime timer (no-op if running) |
| `stopUptime` | `() => void` | Stop and reset uptime timer |
| `appendLog` | `(line: Omit<LogLine, 'id'>) => void` | Append single log line with auto-ID |
| `appendLogBatch` | `(lines: Omit<LogLine, 'id'>[]) => void` | Append batch, cap at 2000 |
| `clearLog` | `() => void` | Clear log |
| `setServerError` | `(message: string) => void` | Set error message |
| `clearServerError` | `() => void` | Clear error message |
| `loadConfig` | `() => Promise<void>` | Load config from backend, apply theme, set active server |
| `upsertServer` | `(server, password) => Promise<void>` | Add/edit server |
| `deleteServer` | `(serverId) => Promise<void>` | Delete server, update active if needed |
| `selectServer` | `(serverId) => Promise<void>` | Set active server and persist |
| `setGameExe` | `(path) => Promise<void>` | Update game exe path |
| `setServerExe` | `(path) => Promise<void>` | Update server exe path |
| `setTheme` | `(theme) => Promise<void>` | Apply theme to DOM and persist |
| `setLaunchOptions` | `(options) => Promise<void>` | Update launch flags |
| `setShutdownConfig` | `(shutdown) => Promise<void>` | Update shutdown delay/message |
| `setTuningTags` | `(tags) => Promise<void>` | Update tuning file tag map |
| `setTuningFavourites` | `(favourites) => Promise<void>` | Update pinned tuning files |
| `setBackupTargets` | `(targets) => Promise<void>` | Update backup target list |
| `setStoreHtmlOutputDir` | `(dir) => Promise<void>` | Update HTML output directory |
| `setConsolePresets` | `(presets) => Promise<void>` | Update command shortcut preset list |
| `setSchedulerNow` | `(dt: Date) => void` | Store server-parsed UTC time and recompute `eventTimezoneOffset` |

---

## Tauri Event Reference

### Backend → Frontend

| Event | Payload | Source | Description |
|---|---|---|---|
| `server-log` | `LogLinePayload[]` | `server.rs` batcher thread | Batched log lines (up to 50 per emission, 50ms flush) |
| `server-started` | `ServerStatusPayload` | `server.rs` after spawn | Server process started |
| `server-stopped` | `ServerStatusPayload` | `server.rs` watcher thread | Server process exited (includes exit code) |
| `player-event` | `PlayerEventPayload` | `server.rs` stdout reader | Player login, logout, or full session clear on server stop |
| `update-progress` | `UpdateProgressPayload` | `updater.rs` | Update stage + percentage (downloading, extracting, installing, restoring, done) |

`PlayerEventPayload` shape: `{ kind: "login" | "logout" | "clear", session_id: string | null, username: string | null, count: number }`.

---

## Data Flow

### Config Persistence

```
Frontend store ──invoke──→ Rust command ──→ load multiverse.json from disk
                                          ──→ modify field
                                          ──→ write multiverse.json to disk
```

Every config mutation loads from disk, modifies, and writes back. There is no in-memory cache on the Rust side. This is simple and correct but means rapid successive mutations each do a full read-write cycle. The frontend's optimistic store update keeps the UI responsive regardless.

Passwords follow a separate path: the frontend sends plaintext to `upsert_server`, which encrypts via AES-256-GCM before storing. The `Server` type sent to the frontend omits `password_enc` and `password_nonce` (they are `#[serde(default)]` fields not populated in the response). Passwords are only decrypted at game launch time in `launcher.rs`. The is_local and use_https fields are included in the serialised Server record. For local servers, host is stored as an empty string; the actual siteconfigurl and dashboard URL are computed at use time from Config.ini.

### Server Lifecycle

```
start_server
  → validate exe path
  → update DbPath to <server_exe_dir>/Data/Account.db
  → spawn MHServerEmu (piped stdin/stdout/stderr, cwd = server dir)
  → assign to Job Object (Windows)
  → spawn stdout reader thread → log_tx + player event parsing
  → spawn stderr reader thread → log_tx
  → spawn batcher thread: collects from log_rx, emits "server-log" events
  → spawn watcher thread: polls try_wait(150ms), emits "server-stopped" on exit,
                          calls clear_player_state before emitting
  → emit "server-started"

stop_server
  → write "!server shutdown\n" to stdin
  → return immediately
  → (watcher thread detects exit, clears PlayerState, emits "server-stopped")
  → safety net: background thread hard-kills after 10s if still running,
                also clears PlayerState and emits "server-stopped" as fallback

Window close
  → prevent default close
  → kill_child() - kills both MHServerEmu and Apache
  → save window state
  → exit(0)
```

Apache start/stop is independent: `start_apache` derives the Apache path from `server_exe` (`../../Apache24/bin/httpd.exe`), spawns with `APACHE_SERVER_ROOT` env var, and stores the handle in `ServerProcess.apache_child`. Apache stdout/stderr is piped to null.

### Player Session Tracking

```
stdout line arrives on reader thread
  → parse_log_line → send to log_tx (always)
  → parse_player_log_event → match on "Accepted and registered client" / "Removed client"
      Login:
        → extract username + session_id via regex
        → lookup_account(db_path, username)
            → open Account.db read-only (rusqlite)
            → JOIN Account + Player + GuildMember + Guild
            → separate COUNT query for Avatar
            → return AccountInfo (best-effort, None on any error)
        → insert PlayerSession into PlayerState map (keyed by session_id)
        → emit "player-event" { kind: "login", session_id, username, count }
      Logout:
        → extract session_id (and username if present) via regex
        → remove session from PlayerState map
        → emit "player-event" { kind: "logout", session_id, username, count }

server stopped / hard-killed:
  → clear_player_state: PlayerState.clear()
  → emit "player-event" { kind: "clear", count: 0 }
```

The Account.db connection is opened fresh on every login event. This is safe because MHServerEmu holds its own separate connection and SQLite supports concurrent readers.

### INI Editing

```
read_config
  → parse Config.ini (defaults)
  → parse ConfigOverride.ini (overrides, may not exist)
  → merge: defaults + overrides
  → return merged values + set of overridden keys

write_config
  → parse Config.ini (defaults)
  → parse existing ConfigOverride.ini
  → for each updated key:
      if value == default → remove from overrides
      if value != default → set in overrides
  → write ConfigOverride.ini (only non-default values)
```

### Events File I/O

```
load_events / load_event_schedule
  → check for EventsOverride.json / EventScheduleOverride.json
  → if exists → read and return (using_override: true)
  → else check Events.json / EventSchedule.json
  → if exists → read and return (using_override: false)
  → else return empty list (using_override: false)

save_events_override / save_schedule_override
  → always write to EventsOverride.json / EventScheduleOverride.json
  → Events.json format: JSON object keyed by event ID
  → EventSchedule.json format: JSON array

reset_events_override / reset_schedule_override
  → copy default file → override file (requires default to exist)
  → return loaded override content

merge_events_override / merge_schedule_override
  → read default file (required)
  → read override file if it exists, else start from empty
  → append any default entries whose ID/name is not already in override
  → write merged result to override file
  → return merged content
```

### Live Tuning File I/O

Tuning files are discovered by scanning `{server_dir}/Data/Game/LiveTuning/` for filenames containing `LiveTuningData` and ending with `.json`. Files prefixed with `OFF_` are treated as disabled. The canonical name is always without the `OFF_` prefix.

Toggling renames the file on disk: `LiveTuningData_X.json` ↔ `OFF_LiveTuningData_X.json`.

JSON uses PascalCase keys (`Prototype`, `Setting`, `Value`) matching MHServerEmu's expected format. The `Prototype` field may be null in the source JSON; it is normalised to an empty string on read.

### Patch File I/O

Patch files are discovered by scanning `{server_dir}/Data/Game/Patches/` and `Patches/Off/` for filenames starting with `PatchData` and ending with `.json`. Toggling moves the file between the two directories.

Patch entries use `serde_json::Value` for the `Value` field, allowing arbitrary JSON types (strings, numbers, booleans, arrays, objects) as required by MHServerEmu's patching system.

### Catalog File I/O

The catalog editor uses a base/MODIFIED file separation pattern:

```
CatalogBundle.json           ← base file (read-only from MH Multiverse)
CatalogBundleMODIFIED.json   ← sidecar with user edits
```

On load, entries from both files are merged by `SkuId` - MODIFIED entries override base entries. On save, the entry is upserted into the MODIFIED file. On delete, the entry is removed from whichever file contains it (base or MODIFIED). A `.bak` snapshot of the target file is written before every mutation.

### Calligraphy.sip Reading

```
ensure_catalogue_loaded
  → derive sip path from server_exe
  → if cached path matches → return (cache hit)
  → load_pak: read file, verify KAPG signature, parse entry table
  → extract Blueprint.directory (LZ4 decompress)
  → extract Prototype.directory (LZ4 decompress)
  → parse directories into PrototypeCatalogue
  → build indices: by_id, by_path, by_guid
  → cache result keyed by sip path
```

Directory record format: CalligraphyHeader (3 bytes magic + 1 byte version) → record count (u16 or i32 depending on version) → records. Blueprint records contain `id + guid + flags + path`. Prototype records contain `prototype_id + prototype_guid + blueprint_id + flags + path`. Abstract prototypes (flag bit 0) are excluded from search results.

### Display Name Resolution

Resolution cascades through three sources:

1. `display_names_custom.json` in the server directory (user overrides, loaded lazily per server dir)
2. Embedded `display_names.json` compiled into the binary (~260KB, covers stock prototypes)
3. Prototype path string (verbatim fallback)

Values of `"N/A"` or empty string are treated as absent at each level.

### Update Flow

```
check_update_available
  → compute target build date (today, or yesterday if before 07:15 UTC)
  → construct nightly.link URL
  → HTTP range probe (bytes=0-0) to check existence
  → return UpdateInfo { build_date, download_url, available }

run_update
  → guard: server must not be running
  → backup selected targets to {server_dir}/Backups/{timestamp}/
  → download zip with streaming progress events
  → extract to {server_dir}/_update_staging/
  → detect wrapper directory (if zip contains single subdirectory)
  → copy staged files over server directory
  → restore backed-up files
  → clean up temp files (_update.zip, _update_staging/)
```

---

## Key Design Decisions

### u64 Precision Across the JS Boundary

Prototype runtime IDs and GUIDs are u64 values that can exceed JavaScript's `Number.MAX_SAFE_INTEGER` (2^53 - 1). The catalog system handles this with dual types: `CatalogEntryDisk` uses raw `u64` for on-disk serialisation, while the frontend-facing `CatalogEntry` represents these as `String`. Conversion happens in `guid_disk_to_view` and `guid_view_to_disk`. Calligraphy prototype IDs and GUIDs are similarly transported as decimal strings.

### MODIFIED File Pattern (Store)

Catalog edits are never written to the base `Catalog*.json` files. Instead, a `*MODIFIED.json` sidecar file holds user-created or user-modified entries. This means a server update can safely overwrite base catalog files without losing user edits. The merge-by-SkuId logic in `load_catalog_entries` makes the MODIFIED version authoritative when both exist.

### EventsOverride / Default File Pattern (Events)

Event definitions and schedule rules follow a similar override pattern to catalogs, but without sidecar files. Instead, MHServerEmu natively recognises `EventsOverride.json` and `EventScheduleOverride.json` as alternatives to `Events.json` and `EventSchedule.json`. MH Multiverse always reads the override if it exists and always writes to the override. The default files are never modified. This means a server update can overwrite the default files without affecting the operator's custom event configuration. The Reset operation recreates the override as a copy of the current default; the Merge operation forward-fills any new entries from the default into the existing override.

### OFF_ Prefix Convention (Tuning)

Tuning files are toggled by renaming with an `OFF_` prefix rather than modifying file contents or moving to a subdirectory. This matches how MHServerEmu discovers tuning files: it loads files matching `LiveTuningData*.json`, so `OFF_LiveTuningData_X.json` does not match and is effectively disabled.

### Patches/Off/ Subdirectory (Patches)

Patch files use a different toggle convention from tuning: disabled files are moved to a `Patches/Off/` subdirectory rather than being renamed. This is because MHServerEmu loads all `PatchData*.json` files from the `Patches/` directory, and a prefix-based rename would still match the `PatchData` prefix.

### Player Session State (In-Memory, Derived from Logs)

`PlayerState` is a runtime-only in-memory map. It is populated by parsing MHServerEmu's stdout log lines for login and logout events. It is not persisted and is always cleared on server stop. This means it reflects only the current server run. Account data (email, flags, user level, balance, etc.) is fetched from Account.db once at login time and stored in the session; it is not refreshed while the player remains online. The DB connection is opened read-only and closed immediately after each lookup, which is safe given SQLite's concurrent-reader model.

### Job Object Lifecycle (Windows)

The Windows Job Object is created with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. Since the Job Object handle is stored in `ServerProcess` (which is behind `Arc<Mutex<>>`), it is dropped when the server process is cleaned up. If MH Multiverse crashes, the handle is closed by the OS, which kills the child processes. This provides defense-in-depth beyond the explicit cleanup in the window close hook.

### Config Read-Write-on-Every-Mutation

Each Rust config command loads `multiverse.json` from disk, mutates, and writes back. There is no in-process config singleton. This avoids stale-state bugs at the cost of extra I/O - acceptable given the low mutation frequency and small file size.

### Server Host Normalisation

The Server record stores a raw host string (hostname, IP, or hostname:port) for remote servers and an empty string for local ones. The siteconfigurl passed to the game client is computed at launch time in launcher.rs rather than stored:

- Local server, patched client (`is_local` && `patched_client`): `http://localhost/Dashboard/SiteConfig.xml`
- Local server, unpatched client (`is_local` && `!patched_client`): `http://localhost/SiteConfig.xml`
- Remote server: `normalize_host` strips any scheme and path suffix from the stored host string; `use_https` determines the scheme. Result: `{scheme}://{host}/SiteConfig.xml`

`patched_client` is part of `LaunchOptions`, which is app-global rather than per-server. For remote servers the flag has no effect - the `patched_client` branch is only evaluated inside the `is_local` path.
Computing the URL at use time rather than storing it prevents the class of bugs where user-entered schemes or path suffixes corrupt the siteconfigurl, and ensures the local paths always reflect the actual routing through Apache on port 80.

---

## MHServerEmu Directory Structure (Expected)

```
<root>/
├── Apache24/
│   ├── bin/
│   │   └── httpd.exe
│   ├── conf/
│   │   └── httpd.conf            (uses ${APACHE_SERVER_ROOT})
│   └── logs/
│       └── error.log
├── MHServerEmu/
│   ├── MHServerEmu.exe           ← server_exe points here
│   ├── Config.ini
│   ├── ConfigOverride.ini        (created/managed by MH Multiverse)
│   ├── Data/
│   │   ├── Account.db            ← SQLite database read at player login
│   │   ├── Game/
│   │   │   ├── Calligraphy.sip
│   │   │   ├── mu_cdata.sip
│   │   │   ├── LiveTuning/       ← LiveTuningData*.json files
│   │   │   │   ├── Events.json                (default, not modified)
│   │   │   │   ├── EventsOverride.json        (created/managed by MH Multiverse)
│   │   │   │   ├── EventSchedule.json         (default, not modified)
│   │   │   │   ├── EventScheduleOverride.json (created/managed by MH Multiverse)
│   │   │   │   ├── LiveTuningData*.json       
│   │   │   │   └── Events/
│   │   │   │       └── Weekly/
│   │   │   ├── Patches/          ← PatchData*.json files (enabled)
│   │   │   │   └── Off/          ← PatchData*.json files (disabled)
│   │   │   └── MTXStore/         ← Catalog*.json + *MODIFIED.json files
│   │   └── Web/
│   │       └── MH-Multiverse-Bundles/          ← Generated HTML/CSS (default output)
│   ├── Backups/                  ← Created by MH Multiverse's backup system
│   │   └── {timestamp}/
│   │       ├── manifest.json
│   │       └── {backed up files}
│   └── display_names_custom.json (optional user override)
└── StartServer.bat
```

`server_exe` in the config points to `MHServerEmu/MHServerEmu.exe`. All data paths are derived relative to this executable's parent directory. The Apache path is derived as `../../Apache24/bin/httpd.exe` relative to the server exe.