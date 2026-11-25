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
| App self-update | `reqwest`, `sha2`, `minisign-verify` (release signatures), `semver` |
| Timestamps | `chrono` |
| SQLite | `rusqlite` (`Account.db` - read queries player-login lookups; read/write for account import and restore) |
| Log parsing | `regex` (player login/logout extraction) |
| Overlay integration | `sha2` (trust hashing), `windows` (`ToolHelp` process and module lookup, `Threading` input-idle wait), std named-pipe client |

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
│   │   ├── releaseNotes.ts       Minimal Markdown → blocks parser for release notes (no {@html})
│   │   ├── tooltip.ts            `tip` Svelte action: the app's only tooltip (400 ms on hover,
│   │   │                         instant on keyboard focus); native `title=` is not used
│   │   ├── serverEvents.ts       Tauri event listeners (log, start, stop, player-event)
│   │   ├── serverCommands.ts     Fallback command list for autocomplete; maskSecrets for
│   │   │                         passwords in account commands
│   │   ├── accountCommands.ts    syncAccount: create or check a local Account.db account through
│   │   │                         console commands, waiting on the server's reply lines
│   │   ├── browseExe.ts          Shared exe file picker (Settings, Get Started)
│   │   ├── playerMeta.ts         PlayerSession type, user level labels, ban/whitelist flag helpers
│   │   ├── tuningMeta.ts         Tuning enum prefixes, category maps, known file sets,
│   │   │                         Tuning/Events payload type interfaces
│   │   ├── catalogMeta.ts        Catalog type interfaces, type/modifier metadata, categories
│   │   ├── configMeta.ts         ConfigPanel schema (Field/SubSection/NavSection types + data)
│   │   ├── patchesMeta.ts        Patches payload type interfaces (PatchFileInfo, PatchEntry),
│   │   │                         stock pack labels
│   │   ├── prototypes.ts         PrototypeMatch type and the debounced search behind PrototypePicker
│   │   ├── serverReload.ts       reloadCatalog / reloadLiveTuning: send the reload command, report failures
│   │   ├── eventSchedule.ts      Which events run on a given day (mirrors LiveTuningEventRule.cs),
│   │   │                         custom event ids and rules for the Simple Events view
│   │   ├── storeSimple.ts        Store Simple view logic: sections, hide, price, sale, what to write
│   │   ├── gameTweaks.ts         Game Tweaks recipes written into PatchDataZMultiverseTweaks.json
│   │   ├── patchEntryEdit.ts     Advanced patch entry editor helpers: Start from list, JSON view, checks
│   │   ├── linkedVersions.ts     Version follow-up when a linked entry's exes change
│   │   └── versionCaps.ts        What the app offers per linked version (overlay, Steam mode,
│   │                             Infinity XP, stock patch labels, nightly updates)
│   └── components/
│       ├── TitleBar.svelte        Custom window chrome (drag region, active version select, min/max/close)
│       ├── Rail.svelte            Left nav rail (MHO / Local / App groups)
│       ├── PanelSidebar.svelte    Reusable sidebar layout wrapper
│       ├── ConfirmDialog.svelte   Themed confirm dialog driven by askConfirm() in store.ts
│       ├── Dropdown.svelte        Dropdown whose option list follows the theme (a native select's list is drawn by Windows). Every dropdown in the app uses it
│       ├── PrototypePicker.svelte Prototype path input with search by name, class and blueprint filters
│       ├── ConfigPanel.svelte     INI editor (Config.ini / ConfigOverride.ini): pages by purpose, All settings, per-row reset
│       ├── OpsPanel.svelte        Server update, backup create/restore/delete
│       ├── launch/
│       │   ├── LaunchPanel.svelte Server profiles, credentials, game launch (shows an "Overlay" indicator when inject-on-launch is on; clicking it opens the Overlay card in Settings)
│       │   ├── GetStarted.svelte  Launch-page checklist shown with no servers (game/server exe, game data with Copy from client, client and server version match, Server + Apache start, local account, launch options); reopened by the Servers header's ? button
│       │   └── ServerModal.svelte Add/edit server dialog; tabs Remote / Local / From Backup. Local
│       │                          profiles on an Account.db server sync the account on Save
│       │                          (accountCommands.ts); From Backup imports or restores by storage mode
│       ├── server/
│       │   ├── ServerPanel.svelte Server start/stop, log viewer, command input, online players
│       │   ├── PlayersBlade.svelte  Searchable list of players connected to the server
│       │   └── PlayerCard.svelte  Per-player moderation/admin card (kick, ban, user level, info)
│       ├── data/
│       │   ├── DataPanel.svelte   Sub-tabs Events / Tuning / Store / Patches and the Simple / Advanced switch
│       │   ├── NewFileModal.svelte  New catalog or patch file name dialog (Store and Patches)
│       │   ├── events/
│       │   │   ├── EventsSimple.svelte   Simple: stock events with on/off and schedule, Make your own
│       │   │   ├── CustomEventModal.svelte  Make your own event: settings, schedule, saved with save_event_bundle
│       │   │   ├── EventsPanel.svelte    Advanced: schedule dashboard, rule list, definition list
│       │   │   ├── EventRuleEditorModal.svelte   Schedule rule editor (type, days, events list)
│       │   │   └── EventDefinitionEditorModal.svelte  Event definition editor (id, display name, file path, etc.)
│       │   ├── tuning/
│       │   │   ├── ServerRates.svelte    Simple: global rate sliders written to LiveTuningDataMultiverse.json
│       │   │   ├── TuningPanel.svelte    Advanced: LiveTuning files grouped Your files / Core / Events / Other, on/off switches
│       │   │   └── TuningFileEditor.svelte  Inline per-file tuning entry editor (Revert / Save)
│       │   ├── store/
│       │   │   ├── StoreSimple.svelte    Simple: prices, hide, Start/End sale, Add item
│       │   │   ├── AddItemModal.svelte   Add a single item to CatalogMultiverse.json
│       │   │   ├── SaleModal.svelte      Sale settings and how many items they reprice
│       │   │   ├── StorePanel.svelte     Advanced: catalog entry list, file switches, New catalog file
│       │   │   └── StoreEditorModal.svelte  Per-entry catalog editor, item picker, HTML gen
│       │   └── patches/
│       │       ├── PatchesSimple.svelte  Simple: patch packs and Game Tweaks
│       │       ├── AddTweakModal.svelte  Per-power cooldown and summon cap tweaks
│       │       ├── PatchesPanel.svelte   Advanced: PatchData file list, enable/disable
│       │       └── PatchEditorModal.svelte  Per-file patch entry editor
│       └── settings/
│           ├── AppPanel.svelte    Settings: linked versions, launch options, display, integrations
│           │                      (overlay link/trust/flags/loot log), about
│           ├── LinkedVersions.svelte  Settings list of linked versions: an IntegrationCard each, opened to edit Name, Client, Server, Version, Custom
│           └── integrations/      Integrations page building blocks
│               ├── IntegrationCard.svelte     Collapsible card shell (title, subtitle, optional head actions); one per integration and per linked version; openCard(id) pre-opens one for deep links
│               ├── OverlayIntegration.svelte  MarvelHeroesOmega Overlay card: folder/trust, inject toggle and Inject now, flags, loot log
│               └── ProfileImport.svelte       Import server profiles card (Bifrost, overlay launcher): detect/browse, preview, import
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
│   │   ├── game_data.rs          Copies Calligraphy.sip and mu_cdata.sip from the client to the server when missing
│   │   ├── versions.rs           Game version (1.48 / 1.52 / 1.53) of a client exe, server dll or save
│   │   ├── launcher.rs           launch_game (spawn with args), LaunchedGame (last launched game), game_is_running (sysinfo poll)
│   │   ├── ini.rs                Config.ini / ConfigOverride.ini read/write with diff-only saving
│   │   ├── tuning.rs             LiveTuningData*.json scan/read/write/create/toggle
│   │   ├── patches.rs            PatchData*.json scan/load/save/create/toggle
│   │   ├── store.rs              Catalog*.json load/save/delete, display name resolution,
│   │   │                         u64 precision handling, bundle HTML generation
│   │   ├── calligraphy.rs        Calligraphy.sip pak reader, blueprint/prototype directory
│   │   │                         parsing, prototype search, ID/GUID/path resolution
│   │   ├── updater.rs            Nightly build update (download/extract/install),
│   │   │                         backup create/list/restore/delete with manifests
│   │   ├── app_update.rs         MH Multiverse self-update: GitHub Releases check, verified
│   │   │                         download, portable exe swap or MSI install
│   │   ├── accounts.rs           Account export (!account download JSON) import and restore;
│   │   │                         Download/ folder scanning, ID remapping, Add/Replace modes;
│   │   │                         JSON-storage restore; read-only account/password checks
│   │   ├── test_helpers.rs       Test-only fixtures (temp server dir, seeded Account.db); the
│   │   │                         tests live in tests/rust/test_*.rs, one #[path] mod per module
│   │   ├── overlay/              Third-party overlay integration (see Rust Modules)
│   │   │   ├── mod.rs            OverlayConfig/TrustRecord, hashing, trust + flag commands,
│   │   │   │                     post-launch injection and Inject now orchestration
│   │   │   ├── inject.rs         Runs the overlay's MhInject.exe for one PID
│   │   │   ├── pipe.rs           Status pipe reader, line splitting/sanitising, event formatting, enumIdx name table
│   │   │   └── tests.rs          Unit tests (compiled only under `cargo test`)
│   │   ├── profile_import/       Server profile import from Bifrost and the overlay launcher
│   │   │   ├── mod.rs            Source detection, preview and import commands
│   │   │   └── tests.rs          Unit tests
│   │   └── paths.rs             Shared server_dir() helper (derives the server's containing
│   │                             directory from server_exe), used across most other modules
│   ├── assets/
│   │   └── display_names.json    Embedded prototype path → display name map (~260KB)
│   ├── capabilities/
│   │   └── default.json          Tauri permission grants
│   ├── Cargo.toml
│   └── tauri.conf.json           Window config (1100×700, min 900×580, decorations: false)
│
├── scripts/
│   └── bump-version.js           Version bump script (package.json, Cargo.toml, tauri.conf.json)
│
├── package.json
├── vite.config.ts                
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

`App.svelte` renders the active panel inside a fixed layout of `TitleBar` + `Rail` + content area. `DataPanel.svelte` is a sub-router that renders a secondary tab bar and a Simple / Advanced switch (`advanced_data` in `multiverse.json`, one setting for all four tabs). Simple mounts `EventsSimple`, `ServerRates`, `StoreSimple` or `PatchesSimple`. Advanced mounts `EventsPanel`, `TuningPanel`, `StorePanel` or `PatchesPanel`.

There is no URL-based routing. Tab state is in-memory only and resets to `launch` on app restart.

### Window Chrome

`decorations: false` in `tauri.conf.json` disables native window decorations. `TitleBar.svelte` provides a custom title bar with `data-tauri-drag-region` for window dragging and manual minimize/maximize/close buttons that call Tauri's window API. With any linked versions it also shows a `Dropdown` of them that calls `activate_linked`, and shows the refusal next to it for a few seconds when the server or the game is running.

Window size and position are persisted via `tauri-plugin-window-state`, filtered to `SIZE | POSITION` flags only to avoid conflicts with the custom titlebar.

### Store Layer (`src/lib/store.ts`)

All shared frontend state lives in Svelte writable stores. The store module also exports async wrapper functions that optimistically update local state, then invoke the corresponding Tauri command:

```
setLaunchOptions(options)  →  appConfig.update(...)  →  invoke('set_launch_options', { options })
```

This pattern (`updateConfigOptimistic`) is used by `setLaunchOptions`, `setShutdownConfig`, `setBackupTargets`, `setConsolePresets`, `setTextSize` and `setAdvancedData`. Each returns an error string, `''` when saved. On a failure it re-reads `get_config`, so a revert can't undo a later change that did save. `setGameExe` and `setServerExe` are not optimistic: a `server_exe` change remounts the tab, which would drop the caller's error.

`loadConfig()` is called once on mount. It invokes `get_config`, populates `appConfig`, applies the saved theme to the DOM, and sets `activeServerId`.

`setSchedulerNow(dt)` accepts a UTC `Date` parsed from server log output, stores it in `schedulerNow`, and computes `eventTimezoneOffset` (integer hours) as the difference between the parsed server time and the local `Date.now()`. This offset is consumed by `EventsPanel` to evaluate schedule rules against server time rather than client local time.

### Event Bridge (`src/lib/serverEvents.ts`)

Server process events are received via Tauri's event system, not polling. The bridge is initialised once on mount and guards against duplicate initialisation using a `window.__mhmServerBridge` state object.

Six listeners are registered:

| Event | Source | Effect |
|---|---|---|
| `server-log` | Batched stdout/stderr from `server.rs` | Normalised and appended to `serverLog` store; each line is also checked for the scheduler-now pattern |
| `server-started` | Emitted after successful spawn | Sets `serverRunning`, starts uptime timer |
| `server-stopped` | Emitted on process exit (normal or crash) | Clears `serverRunning`/`apacheRunning`, stops uptime, logs exit code |
| `player-event` | Emitted by `server.rs` on player login/logout/clear | Consumed by `ServerPanel` to update the online player list |
| `overlay-inject` | Emitted by `overlay/mod.rs` when an injection (post-launch or Inject now) finishes | Sets the `overlayInject` store (shown in Settings → Integrations). A failed Inject on launch (`on_launch`) also turns the Launch page's Overlay chip amber, with the reason in its tooltip, until the game closes |
| `overlay-pipe` | Emitted by `overlay/pipe.rs` per read of the overlay's status pipe | Appended to the `overlayLog` store (capped at 500 rows) |

`server-started` and `server-stopped` use a logTimestamp() function to mirror the formatting used for MHServerEmu entries

On bridge init, `syncInitialState()` polls `server_is_running` and `apache_is_running` to recover state if the frontend reloads while the server is running.

**Scheduler-now extraction**: each incoming log line is matched against the regex `Checking Live Tuning events (now=[...])`. When matched, the timestamp string (format `MM/DD/YYYY HH:MM:SS`, server-local time) is parsed as UTC and passed to `setSchedulerNow()`. This keeps `schedulerNow` and `eventTimezoneOffset` continuously up to date while the server is running.

### Metadata Modules

**`tuningMeta.ts`** - maps tuning enum prefixes (e.g. `eGTV_`, `eWETV_`) to display categories and blueprint hints for prototype search scoping. Contains known file sets (`KNOWN_CORE`, `KNOWN_EVENTS`) for the Live Tuning sidebar groups, and the full tuning setting reference with descriptions and default values.

**`catalogMeta.ts`** - defines TypeScript interfaces mirroring the Rust catalog types (PascalCase field names matching `serde(rename_all = "PascalCase")`). Contains the catalog type/modifier taxonomy, item category definitions with prototype path prefixes for the item picker, and helper functions for type inference and modifier construction.

**`playerMeta.ts`** - defines the `PlayerSession` type (mirroring the Rust struct serialised from `server.rs`), user level label maps and option arrays, and helper functions: `userLevelLabel(level)`, `isBanned(flags)` (flag value 2), `isWhitelisted(flags)` (flag value 16), `formatLastSeen(ts)` (handles .NET ticks, Unix ms, and Unix seconds).

**`serverCommands.ts`** - hardcoded fallback command list used for autocomplete in the command input. The `ServerPanel` has commented-out code for fetching commands from the server's `/Commands` endpoint at runtime, which is not yet implemented on the MHServerEmu side.

### Component Patterns

Panels follow a consistent layout: `PanelSidebar` on the left (file/item list with search and filters), detail/editor area on the right. Modal editors (`StoreEditorModal`, `PatchEditorModal`, `EventRuleEditorModal`, `EventDefinitionEditorModal`) are mounted conditionally when editing state is set, and communicate back to their parent via `onClose`/`onSaved`/`onDeleted` callback props. Live Tuning edits inline instead: `TuningPanel` shows the selected file in `TuningFileEditor`, which asks before switching files with unsaved edits.

Every prototype field uses `PrototypePicker` (search by name through `search_prototypes`, optionally limited by class or blueprint). Unsaved edits register through `setUnsaved`, and `confirmLeave` asks before a tab switch drops them.

**`EventsPanel.svelte`** deviates from the standard sidebar/editor split. The sidebar lists schedule rules grouped by type (`AlwaysOn`, `DayOfWeek`, `WeeklyRotation`, `SpecialDate`), and the main area is a dashboard showing currently active events and a collapsible full event definition list. Selecting a rule or definition opens an inline editor area (not a separate modal) in place of the dashboard. Override status controls (Reset / Merge) for both Events and Schedule are rendered at the bottom of the sidebar.

**`PlayerCard.svelte`** is a self-contained card rendered per online player in `PlayersBlade.svelte` (accessed via a button in `ServerPanel`). It issues moderation commands (`!client kick`, `!account ban/unban`, `!account whitelist/unwhitelist`, `!client info`) and admin commands (`!account userlevel`) directly via `invoke('send_command', { cmd })`. Actions requiring an email address check `player.email` before proceeding and display inline feedback. Destructive actions (kick, ban) require a two-step confirmation UI within the card.

**`ServerModal.svelte`** is tabbed rather than a single form: Remote, Local and From Backup. Add mode's From Backup creates a new local account from a `!account download` export, then its server profile in the same step. Edit mode shows From Backup only for profiles saved as local, and there it restores into the profile's own account (no identity fields, resolved from the profile's `email`). A backup for another email or player name shows a warning and the button reads Restore Anyway. With JSON persistence From Backup writes the server's single save instead (`restore_json_account`), and Add hides From Backup once a local profile exists for the active version, since restoring is then done from that profile's Edit. Picking a file already refuses an export from another game version, and a missing Account.db shows before any fields are filled in. While the server runs, both tabs offer Stop Server.

---

## Backend Architecture

### Managed State

Seven state objects are registered via `.manage()` in `lib.rs`:

| State | Type | Purpose |
|---|---|---|
| `ServerState` | `Arc<Mutex<ServerProcess>>` | Owns child process handles for MHServerEmu and Apache, plus the Job Object handle |
| `PlayerState` | `Arc<Mutex<HashMap<String, PlayerSession>>>` | In-memory map of active player sessions, keyed by hex session ID |
| `DbPath` | `Arc<Mutex<Option<PathBuf>>>` | Path to `Account.db`, derived from `server_exe` and updated whenever `set_server_exe` is called |
| `CatalogueState` | `Mutex<Option<(String, PrototypeCatalogue)>>` | Cached Calligraphy.sip parse result, keyed by sip file path |
| `DisplayNameState` | Embedded + lazy-loaded maps | Prototype path → display name resolution (embedded JSON + optional per-server override file) |
| `GameProcessState` | `Mutex<sysinfo::System>` | Reused across `game_is_running` polls so each poll refreshes processes instead of building a new `System` |
| `LaunchedGame` | `Mutex<Option<Game>>` | The game this app last launched (PID plus its `Child`, so the PID cannot be reused). Set on every launch, whether or not injection is on; used by overlay injection |

### Rust Modules

**`config.rs`** - `AppConfig` is the root persisted configuration. Stored as `multiverse.json` in the OS app data directory (`%APPDATA%\com.mhmultiverse.app\`). Passwords are encrypted with AES-256-GCM; the 256-bit key is stored in and retrieved from the OS keychain via `keyring`. Each config-mutating command loads the full config from disk, modifies the relevant field, and writes back - there is no in-process config cache on the Rust side. `AppConfig` includes an `overlay` field (`OverlayConfig`, defined in `overlay/mod.rs`, `#[serde(default)]` so older files load unchanged). It also includes a `console_presets` field (`Vec<String>`) for persisted command shortcut strings shown in the server panel. Server struct includes is_local (bool) and use_https (bool) fields; is_local suppresses host storage and triggers Config.ini-derived URL building at launch time. `linked` (`Vec<LinkedInstall>`) holds client and server pairs per game version, and `active_linked_id` names the one in use. The active entry is copied into `game_exe`, `server_exe` and `launch_options.no_steam`, so nothing else reads `linked`. The Settings setters write back into the active entry, and new exes there detect its version again (a name that was only the old version follows it). A config from before linked installs becomes one entry named after its detected version the first time it loads. `app_version` records the app version that last loaded the file. When it differs from the running one, or the file can't be parsed, `load_config` first copies the file to `multiverse.json.bak` (one copy, replaced at each upgrade), so a migration can be undone by hand. An unparseable file is left on disk as it is.

**`server.rs`** - Server lifecycle management and player session tracking. `start_server` spawns MHServerEmu with piped stdin/stdout/stderr and assigns it to a Windows Job Object (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`). Four named background threads are spawned per server start (`mhserver-stdout-reader`, `mhserver-stderr-reader`, `mhserver-log-batcher`, `mhserver-watcher`): stdout/stderr readers, a batcher that collects log lines and emits them to the frontend in batches (up to 50 lines per 50ms flush interval), and a watcher thread that polls `try_wait()` every 150ms to detect process exit, clearing `proc.child` and emitting `server-stopped` when it does. The watcher is the sole source of truth for exit state - `server_is_running` just reads `proc.child.is_some()` rather than also calling `try_wait()` itself. `start_server` performs a pre-flight TcpStream::connect check against the configured WebFrontend Port before spawning, returning a clear error if the port is already in use.

The stdout reader additionally calls `parse_player_log_event` on every line (regexes compiled once via `LazyLock`, not per line); matches trigger `handle_player_log_event` which updates `PlayerState` and emits `player-event` to the frontend. On login, `lookup_account` opens a read-only SQLite connection to `Account.db` and queries the `Account`, `Player`, `Guild`, `GuildMember`, and `Avatar` tables to populate the full `PlayerSession`. On server stop, `clear_player_state` empties `PlayerState` and emits a `player-event` with `kind: "clear"`. `stop_server` writes `!server shutdown\n` to stdin once (`ServerProcess.stopping` makes a second Stop a no-op) and spawns a fifth thread (`mhserver-stop-timeout`) that falls back to a hard kill after 60 seconds if the watcher hasn't already detected exit. When the server logs "All services shut down" or a Fatal line, `release_console_wait` writes an empty line so a server waiting in `Console.ReadLine` exits. The server runs without a console, so its stdin and stdout use the OEM code page (`oem_encode` / `oem_decode`), not UTF-8. Apache is managed independently via `start_apache`/`stop_apache`. A small thread polls Apache every 500 ms and, when it exits on its own, logs it and emits `apache-stopped`. `apache_is_running` still calls `try_wait()` directly. `DbPath.set_from_server_exe` derives `<server_exe_dir>/Data/Account.db` and is called both when `set_server_exe` is invoked from config and when `start_server` is called.

**`events.rs`** - File I/O for `Events.json` / `EventsOverride.json` and `EventSchedule.json` / `EventScheduleOverride.json` in `Data/Game/LiveTuning/`. Both file pairs follow the same override pattern: the override file takes precedence if it exists, otherwise the default file is read. `Events.json` is a JSON object keyed by event ID; `EventSchedule.json` is a JSON array. Read functions deserialise into `EventDefinition` and `ScheduleRule` structs respectively. Write functions always target the override file. `reset_*_override` copies the default file over the override; `merge_*_override` adds any entries present in the default but missing from the override (by ID for events, by name for rules), without overwriting existing override entries. For the Simple view, `load_stock_events` reads the stock files whether or not overrides exist, and `save_event_bundle` saves custom events in one go: their tuning files in `LiveTuning/Events/Multiverse/LiveTuningData<slug>.json`, then both override files together (a schedule naming an id missing from the events list loses that rule), then deleted events' files moved aside to `<name>.<timestamp>.bak`. Custom event ids start with `zzMultiverse_`.

**`launcher.rs`** - Spawns the game client as a detached process with command-line arguments derived from the active server profile and launch options. Credentials are decrypted from the config at launch time. For local servers, the siteconfigurl host is derived at launch time from WebFrontend.Port in Config.ini via ini::read_merged_value. For remote servers, normalize_host strips any scheme and path from the stored host before building the URL; use_https determines the scheme. game_is_running uses sysinfo to check for a process running the linked game exe or its `Binaries\Win32`/`Win64` sibling. It compares full paths because 1.52 and 1.53 both ship MarvelHeroesOmega.exe (1.48 has MarvelHeroes2016.exe), and falls back to the file name when a process's path can't be read. After a successful spawn, `launch_game` stores the game as a `Game` (PID plus the `Child`) in `LaunchedGame`, then hands it and a copy of the overlay config to `overlay::on_game_spawned`, which returns immediately unless overlay injection is enabled and trusted. `game_is_running` finds the linked install's game and so also sees it when started elsewhere; `LaunchedGame` knows only our own launch.

**`overlay/`** - Integrates the third-party MarvelHeroesOmega Overlay (closed source, not part of this project; none of its files are bundled or redistributed). Multiverse never runs any overlay executable: the overlay is a native DLL (`MhClientHook.dll`) that runs inside the game process, so it lives and dies with the game.
- *Trust (TOFU)*: the user links the overlay folder (`overlay_set_dir`, canonicalised, must contain `MhClientHook.dll`) and presses Trust. `scan_set` hashes (SHA-256) a fixed allowlist: the DLL, `terminals.bin`, `enum-names.bin`, the loot-filter default JSON, the injector (`MhInject.exe`, `MhInject.dll`, its `deps.json` and `runtimeconfig.json`, `MarvelHeroes.Injection.Core.dll`), and the portraits/icons/fonts/textures folders. Flags, panel layout JSON, the overlay's launcher, legacy exes and its updater are never hashed. The per-file hashes and a combined set hash are stored in `multiverse.json`. A symlink or subfolder inside an asset folder cannot be hashed: Trust refuses it, and one appearing later shows as `added`. Status is derived live: `not_linked`, `missing`, `untrusted`, `trusted`, or `changed` (with `changed:`/`missing:`/`added:` lines). There is no known-hash list in the repo. Changing folder or untrusting also switches injection off.
- *Injection*: opt-in (`auto_inject`, default off, only settable while trusted). `on_game_spawned` starts a background thread (it does no hashing on the caller's thread) that holds the shared `Game` (so the PID cannot be reused), refuses immediately if the files no longer match (reported via `overlay-inject`), writes `skip-boot-loading-gate.flag` as `0` on a fresh launch (a stale `1` would skip the loading gate), waits up to 15 s for the game to go input-idle, then re-verifies the hashes immediately before each attempt and refuses if anything changed. Multiverse never writes into the game's memory itself: Defender quarantined 1.7.1 (`Behavior:Win32/DefenseEvasion.A!ml`) for doing so. `inject.rs` instead runs the overlay's own `MhInject.exe <dll> <exe stem>` (no window, output captured), which does the classic `LoadLibraryW` remote-thread injection and waits 5 s. MhInject takes the first process with that name, so `inject.rs` first looks up our PID's exe name and refuses if another process shares it, and `mod.rs` confirms afterwards that the DLL is in our PID's module list. Exit 0 is success. A `TIMEOUT` line on stderr is retried (up to 5 times); between attempts it waits up to 2 s (10 s after the last) for the module to show up, since a slow hook can finish loading late. A .NET app host error (`0x800080xx`, or its "install or update .NET" text) is reported as a missing .NET 8 Runtime. Any other failure shows the exit code and the first stderr line, capped at 300 characters. MhInject also rewrites `MhClientHook.hidden_enum.dat` from a built-in rule. The hook only logs that file, and it never touches `loot-filter-config.json`. Failures never block the game launch; the outcome is reported via `overlay-inject`.
- *Inject now*: `overlay_inject_now` runs the same injection on the game in `LaunchedGame` when injection on launch is off. It refuses if no game was launched by this app or it has exited, and reports "already loaded" instead of injecting twice. Because the game is already past boot it writes `skip-boot-loading-gate.flag` as `1` (inferred from the flag's name; unverified against the hook).
- *Flags*: `overlay_get_flags`/`overlay_set_flag` read and atomically write the one-byte `'1'`/`'0'` files the hook reads next to the DLL (terminals, DPS, debug logging; keys are allowlisted; refused unless the overlay is trusted). A missing file reads as the hook's default (terminals/DPS on, logging off). The hook reads them at load, so changes apply on the next game launch.
- *Status pipe*: after a successful injection `pipe.rs` connects (retrying for 30 s) to the hook's `\\.\pipe\MhTerminalsHelper` and reads JSON lines until the game exits. `pipe::spawn` holds a liveness check backed by the `Game`: if the pipe drops while the game is still running it waits 2 s and reconnects (events in between are lost). The pipe's access rules are unverified, so every line is untrusted: lines over 8 KB are dropped, control characters are replaced, text is capped at 512 characters, and only allowlisted `event` names pass: `loot_hidden`, `region_entered` (only with a terminal), `tier_resolved` (only with a tier), `mission_state` (only when completed) and `player_death`. A `loot_hidden` event is rendered as `time | category | item name | item code | rarity` plus the filter reason. Death events carry a recent-damage table (`hits`). The hook's `enumIdx` values (items, killers, attackers) are the game's EntityPrototype enum index, rebuilt on the pipe thread from the client's `Data/Game/Calligraphy.sip` (the server's copy as fallback; `calligraphy::entity_paths`: prototypes whose blueprint type derives from EntityPrototype, sorted by PrototypeId, 1-based). Names are the prototype file name and the category comes from the path folder. If no sip is readable, names fall back to `enum-names.bin` in the linked folder (items only, category = name minus trailing digits). Rarity codes map through an embedded table (unknown codes are shown as numbers).
- *Overlay update check*: `app_update::check_overlay_update` compares the linked folder's `MhClientHook.dll` version resource (read, never loaded; not the folder name) with the latest release of the overlay's public GitHub repo. It only reports; the card links to the release page and to the linked folder, and installs nothing. It runs only when the user presses the button on the Overlay card. After the user updates, the normal trust check reports the changed files.
- *Omega gear*: one toggle on the Overlay card (`overlay_set_omega_gear`, stored as `overlay.omega_gear`) controls `showOmega` in `%LOCALAPPDATA%\MarvelHeroesOmega.LootFilter\loot-filter-config.json`, the hook's loot panel layout (the Omega column and Omega Slot 1-5; hiding and pipe events don't depend on it). Toggling writes the value immediately, either way; either direction needs a trusted overlay. While it is on and the overlay is trusted, each injected launch re-applies it once before injecting and once after, since the hook reloads the file when its write time changes. With it off, or with the overlay changed or untrusted, launches never touch the file. The file is never created (the hook seeds it only when absent), an unchanged value is not rewritten, and an unparsable or non-object file is left alone and reported. It is user data outside the overlay folder, so it is not part of the trust hashes.
- *Why no copy*: an earlier design copied the trusted files into an app-owned folder and loaded from there. It was dropped: a same-user process can write to both locations, so the copy only narrowed a timing race, and it broke the overlay's own settings handling. The check now happens right before injection instead.

**`ini.rs`** - Reads `Config.ini` (defaults) and `ConfigOverride.ini` (overrides), merges them, and returns the merged values plus a set of which keys are overridden and the Config.ini defaults. Writes use diff-only logic: values matching the default are removed from the override file; only differing values are written. Also exposes `read_merged_value` (non-command pub fn) for reading a single merged key by section, used by `server.rs` for the port pre-check.

**`tuning.rs`** - File I/O for `LiveTuningData*.json` files in `Data/Game/LiveTuning`. The enable/disable toggle works by renaming files with an `OFF_` prefix. JSON is read/written using PascalCase field names (`Prototype`, `Setting`, `Value`) to match MHServerEmu's expected format. Handles UTF-8 BOM stripping. `read_global_tuning` returns the global settings the server would load from the base files, in its load order, for the Server Rates view, which writes its own file `LiveTuningDataMultiverse.json`.

**`patches.rs`** - File I/O for `PatchData*.json` files in `Data/Game/Patches`. Enable/disable works by moving files between `Patches/` and `Patches/Off/`. Patch entries use `serde_json::Value` for the `Value` field to support arbitrary JSON value types.

**`store.rs`** - Catalog entry management for `Catalog*.json` in `Data/Game/MTXStore`. Uses a base/MODIFIED file separation: base files are read-only from MH Multiverse's perspective, and all edits go to `*MODIFIED.json` sidecar files. MODIFIED entries override base entries with the same `SkuId`. The module handles u64 precision by using a dual type system: `CatalogEntryDisk` (raw u64 for on-disk JSON) and `CatalogEntry` (String for the JS boundary). Display name resolution chains through: custom override file → embedded `display_names.json` → prototype path → raw ID. Also generates HTML/CSS bundle pages for in-game store display. A catalog file is switched off by moving it and its MODIFIED file into `MTXStore/Off/`, since the server only reads the MTXStore folder itself. `apply_store_changes` writes many changes at once (each touched MODIFIED file is parsed first and nothing is written unless all parse), and the Simple view's sale record goes last to `MultiverseStore.json`.

**`calligraphy.rs`** - Reads MHServerEmu's `Calligraphy.sip` pak file (LZ4-compressed entries with a `KAPG` signature). Parses the `Blueprint.directory` and `Prototype.directory` files within the pak to build a `PrototypeCatalogue` with indices by runtime ID, path, and GUID. The catalogue is cached per sip file path and rebuilt automatically if the server executable changes. Prototype search supports multi-prefix category filtering and case-insensitive matching against both path and display name.

**`updater.rs`** - Downloads nightly builds from `nightly.link/Crypto137/MHServerEmu/...` (the `windows-v48`, `-v52` or `-v53` workflow, matching the server's own game version), extracts them, and overlays onto the server directory. The update flow: check availability (HTTP range probe) → download with progress events → extract to staging dir → pre-update backup of the selected targets, every file the install will replace and the user's copies of stock files switched the other way → copy to server dir (a failure rolls back from that backup) → keep the user's on/off choice for switched stock files. The targets are not copied back, so upstream fixes always land. Files the app had edited (a `.json.bak` beside them) that the install replaced are reported, and `put_back_edits` copies them back from the pre-update backup after backing up the update's copies as a pre-restore backup. `cancel_update` stops it at the next check before the install, which can't be cancelled. Backups are stored in `{server_dir}/Backups/{timestamp}/` with a `manifest.json` (an id taken in the same second gets a `-1`, `-2` suffix). The manifest's `replaced` lists the files an update overwrote. A plain Restore leaves them out, so it can't put the old server back. Roll Back Update (`restore_backup` with `roll_back`) copies them back too, which puts the old build back. Files new in the newer build stay. A Restore first makes a "pre-restore" backup of the files it will replace. A roll back's pre-restore backup also holds the newer server files and the versions, so its Undo Roll Back is the same roll back in reverse. After each update, restore or Restore My Edits, the oldest automatic (pre-update and pre-restore) backups beyond `keep_auto_backups` (default 10, 0 keeps all) are deleted, never the one just restored from. Manual backups are never deleted. `Calligraphy.sip` and `mu_cdata.sip` are blacklisted from backups.

**`app_update.rs`** - Self-update for MH Multiverse itself. Checks `api.github.com/repos/Cackl/MH-Multiverse/releases`, picks the highest published `vX.Y.Z` (skipping drafts and prereleases), and returns notes for every newer release, flagging any whose body contains a `## Breaking Changes` heading. Installs re-fetch the release by tag and refuse to touch the running exe unless the download carries a valid minisign signature from the embedded `PUBKEY`, names the expected file in its trusted comment, and (exe) reports the expected version in its version resource. See [Releasing](#releasing-signed-updates).

**`accounts.rs`** - Parses `!account download` JSON exports and writes them into `Account.db` in one of two modes. Both modes, and the JSON-save restore, first compare the game build in the export's Player save header (u32 size, varint archive version, varint build) with the server's detected version, and refuse a save from another game version. Add creates a new account: uniqueness-checked against existing Email/PlayerName, with optional overrides for both plus the password (falling back to the export's own PasswordHash/Salt if no new password is given). Replace overwrites only the game-data of an existing account identified by `target_id` — Email, PlayerName, Password, UserLevel, and Flags on the Account row are never touched, so the caller (`ServerModal`'s Restore tab) is responsible for verifying the file actually belongs to the target before invoking it. Both modes route through `insert_player_data`, which assigns every entity (Player, Avatar, TeamUp, Item, ControlledEntity) a fresh DbGuid and rewrites ContainerDbGuid references through an in-memory remap table — this remapping is entirely self-contained per file, so it has no dependency on the file's original account ID matching anything. `scan_download_backups` lists candidate files from `<game_exe_dir>/Download/` by filename pattern (`0x{IdHex}_{PlayerName}_{yyyy-MM-dd_HH.mm.ss}.json`) without parsing them, so the frontend can offer a quick-select list before committing to a full parse. All writes run inside a transaction and are guarded by `server_process_is_running` to avoid DB lock contention with a live MHServerEmu process.

### Window Close Hook

`lib.rs` intercepts `CloseRequested`, prevents the default close, and emits `close-requested` with a request id. The window answers with `close_ack`, then `handleCloseRequest` (store.ts) waits for a running update, asks about unsaved edits and (with `shutdown.ask_before_close`) a running server, and calls `exit_app`. `exit_app` refuses while an update runs, otherwise `server::exit_gracefully` sends `!server shutdown`, waits for the server to exit (the stop timer force-closes it after 60 s) and calls `shutdown()`. If the window doesn't acknowledge within 3 s it is taken as hung, and `exit_gracefully` runs from the backend.

`shutdown()` kills child processes via `kill_child()`, saves window state, then calls `app.exit(0)`. `app_update.rs` uses the same `shutdown()` after installing an update.

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
| `set_game_exe` | `path: String` | `AppConfig` | Set game executable path, detects the active entry's version again |
| `set_server_exe` | `path: String` | `AppConfig` | Set server executable path, updates DbPath, detects the active entry's version again |
| `set_theme` | `theme: String` | `()` | Set UI theme |
| `set_launch_options` | `options: LaunchOptions` | `()` | Set game launch flags |
| `activate_linked` | `id: String` | `AppConfig` | Switch linked version: copies its exes and Steam setting to the active ones, updates DbPath. Refused while the server or the game runs |
| `upsert_linked` | `install: LinkedInstall` | `AppConfig` | Add/update a linked version. New exes on the active one count as a switch. With none active, the entry becomes the active one. The frontend sets `version` from detection (`linkedVersions.ts`), or from the user's pick when detection fails |
| `delete_linked` | `id: String` | `AppConfig` | Remove a linked version other than the active one |
| `detect_client_version` | `game_exe: String` | `Option<String>` | `1.48` / `1.52` / `1.53` from the exe's file version (`versions.rs`) |
| `detect_server_version` | `server_exe: String` | `Option<String>` | Same, from the version string in `MHServerEmu.dll` |
| `missing_game_data` | `server_exe: String` | `Vec<String>` | Which of `Calligraphy.sip` and `mu_cdata.sip` the server's `Data/Game` lacks (`game_data.rs`) |
| `copy_game_data` | `game_exe, server_exe` | `Vec<String>` | Copies the missing ones from the client's `Data/Game` (searched up from the game exe) through a `.part` file, never replacing one that is there. Returns the files copied |
| `set_shutdown_config` | `shutdown: ShutdownConfig` | `()` | Set shutdown delay and broadcast message |
| `set_keep_auto_backups` | `keep: u32` | `()` | How many automatic backups to keep, 0 keeps all |
| `set_backup_targets` | `targets: Vec<String>` | `()` | Set backup target paths. `Players` stands for the account database and JSON save, resolved when a backup is made |
| `set_console_presets` | `presets: Vec<String>` | `()` | Set command shortcut preset list |
| `set_text_size` | `size: f64` | `()` | Set the Display text size (0.5 to 2.0, applied as webview zoom) |
| `set_advanced_data` | `advanced: bool` | `()` | Set the Data tab's Simple / Advanced mode |
| `take_config_notice` | - | `Option<String>` | The one-time notice when `multiverse.json` couldn't be read and the app started with defaults (names where the old file was kept) |

### App (`lib.rs`)

| Command | Parameters | Returns | Description |
|---|---|---|---|
| `close_ack` | `id: u64` | `()` | The window received close request `id`, so the 3 s hung-window fallback stands down |
| `exit_app` | - | `()` | Stop the server gracefully, then exit. Refused while an update runs |

### Launcher (`launcher.rs`)

| Command | Parameters | Returns | Description |
|---|---|---|---|
| `launch_game` | `server_id: String` | `()` | Spawn game client with server args and launch options. siteconfigurl uses /Dashboard/SiteConfig.xml for patched local clients, /SiteConfig.xml for unpatched, normalize_host + use_https for remote |
| `game_is_running` | - | `bool` | Poll a reused `sysinfo::System` (managed state) for a process running `game_exe` or its Win32/Win64 sibling (by name when the path can't be read) |
| `normalize_host` | `raw: String` | `String` | Strip scheme/path from a host string; also used internally by `launch_game`. Exposed so the frontend derives dashboard/home URLs from the same logic instead of a separate TS copy |

### Server (`server.rs`)

| Command | Parameters | Returns | Description |
|---|---|---|---|
| `start_server` | `server_exe: String` | `()` | Spawn MHServerEmu with log streaming and player tracking |
| `stop_server` | - | `()` | Graceful shutdown via stdin (sent once), 60s hard-kill fallback |
| `start_apache` | `server_exe: String` | `()` | Spawn Apache (derived path from server_exe) |
| `stop_apache` | - | `()` | Kill Apache process |
| `send_command` | `cmd: String` | `()` | Write to MHServerEmu stdin |
| `server_is_running` | - | `bool` | Read watcher-owned state (`proc.child.is_some()`) - the exit watcher thread is the sole source of truth |
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
| `load_stock_events` | `server_exe: String` | `StockEvents` | The stock Events.json and EventSchedule.json, whether or not overrides exist. Empty when missing |
| `save_event_bundle` | `server_exe, definitions, rules, owned_files: Vec<OwnedEventFile>, removed_slugs: Vec<String>` | `()` | Save custom events: their tuning files, both override files together, then move deleted events' files aside |

### INI (`ini.rs`)

| Command | Parameters | Returns | Description |
|---|---|---|---|
| `read_config` | `server_exe: String` | `ConfigState` | Merge Config.ini + ConfigOverride.ini |
| `write_config` | `server_exe: String, updates: IniData` | `()` | Diff-write overrides only |
| `get_config_dir` | `server_exe: String` | `String` | Return server directory path |
| `uses_json_persistence` | `server_exe: String` | `bool` | Whether `[PlayerManager] UseJsonDBManager` is true (merged), i.e. the server stores its one account in a JSON file |

### Tuning (`tuning.rs`)

| Command | Parameters | Returns | Description |
|---|---|---|---|
| `scan_tuning_files` | `server_exe: String, apply_auto_enable: bool` | `Vec<TuningFileInfo>` | List LiveTuningData*.json files with enabled state; auto-enables OFF_ event-owned files on disk when `apply_auto_enable` |
| `read_tuning_file` | `server_exe: String, canonical_name: String` | `Vec<TuningEntry>` | Parse tuning entries from file |
| `write_tuning_file` | `server_exe: String, canonical_name: String, entries: Vec<TuningEntry>` | `()` | Write entries back to file |
| `create_tuning_file` | `server_exe: String, canonical_name: String, entries: Vec<TuningEntry>` | `()` | Create new tuning file |
| `toggle_tuning_file` | `server_exe: String, canonical_name: String, enabled: bool` | `()` | Rename with/without OFF_ prefix |
| `get_live_tuning_dir` | `server_exe: String` | `String` | Return LiveTuning directory path |
| `read_global_tuning` | `server_exe: String` | `GlobalTuning` | Global settings per base file in the server's load order, plus `[GameOptions] InfinitySystemEnabled` and whether the Server Rates file is switched off |

### Patches (`patches.rs`)

| Command | Parameters | Returns | Description |
|---|---|---|---|
| `scan_patch_files` | `server_exe: String` | `Vec<PatchFileInfo>` | List PatchData*.json in Patches/ and Patches/Off/ |
| `load_patch_file` | `server_exe: String, file_name: String, enabled: bool` | `Vec<PatchEntry>` | Parse patch entries from file |
| `save_patch_file` | `server_exe: String, file_name: String, enabled: bool, entries: Vec<PatchEntry>` | `()` | Write entries to file |
| `create_patch_file` | `server_exe: String, file_name: String` | `()` | Create empty patch file in Patches/ |
| `set_patch_file_enabled` | `server_exe: String, file_name: String, currently_enabled: bool` | `bool` | Move between Patches/ and Patches/Off/ |
| `get_patches_dir` | `server_exe: String` | `String` | Return Patches directory path |
| `patch_manager_enabled` | `server_exe: String` | `bool` | `[GameData] EnablePatchManager` (merged). Off, the server loads no patch file |

### Store (`store.rs`)

| Command | Parameters | Returns | Description |
|---|---|---|---|
| `get_mtxstore_dir` | `server_exe: String` | `String` | Return MTXStore directory path |
| `list_catalog_files` | `server_exe: String` | `Vec<String>` | List base Catalog*.json filenames |
| `load_catalog_entries` | `server_exe: String` | `Vec<CatalogEntryWithMeta>` | Load all entries with base/MODIFIED merge |
| `save_catalog_entry` | `server_exe: String, entry: CatalogEntry, target_file: String` | `()` | Upsert entry into *MODIFIED.json |
| `delete_catalog_entry` | `server_exe: String, sku_id: String, source_file: String, from_modified: bool` | `()` | Delete entry by SKU from target file |
| `get_next_sku_id` | `server_exe: String` | `String` | Return max SKU + 1 (floor 1001), as a decimal string |
| `resolve_display_name` | `server_exe: String, prototype_runtime_id: String` | `String` | Resolve prototype ID to display name |
| `generate_bundle_html` | `server_exe: String, entry: CatalogEntry, slug: String, save_to_apache: bool` | `String` | Generate HTML bundle page (always backed up to `MH-Multiverse-Bundles`; also written to Apache's `bundles/` dir when `save_to_apache`), return backup file path |
| `save_thumbnail` | `server_exe: String, slug: String, sku_id: String, png_base64: String, save_to_apache: bool` | `String` | Decode and save a bundle thumbnail PNG (same backup/Apache dual-write pattern), return backup file path |
| `apache_bundles_found` | `server_exe: String` | `bool` | Whether the bundled Apache's `htdocs/bundles` folder exists, so bundle pages can be served |
| `list_off_catalog_files` | `server_exe: String` | `Vec<String>` | Base catalog files switched off in `MTXStore/Off/` |
| `set_catalog_file_enabled` | `server_exe: String, file: String, enabled: bool` | `()` | Move a catalog file and its MODIFIED file into `MTXStore/Off/` or back. Nothing moves if the destination has either file |
| `create_catalog_file` | `server_exe: String, suffix: String` | `String` | Create an empty `Catalog<suffix>.json`. The suffix is letters, digits, `_` or `-`, without MODIFIED |
| `apply_store_changes` | `server_exe, upserts: Vec<StoreUpsert>, removes: Vec<StoreRemoval>, state: Option<Value>` | `()` | Write many Store changes at once: every touched MODIFIED file is parsed first and nothing is written unless all parse. `state` goes to `MultiverseStore.json` last |
| `load_store_state` | `server_exe: String` | `Value` | Contents of `MultiverseStore.json` (the running sale), or `null` |

### Calligraphy (`calligraphy.rs`)

| Command | Parameters | Returns | Description |
|---|---|---|---|
| `search_prototypes` | `server_exe, query, blueprint_hint?, category_path?, is_inventory_type?` | `Vec<PrototypeMatch>` | Search prototype paths in Calligraphy.sip |
| `name_patch_targets` | `server_exe, paths: Vec<String>, ids: Vec<String>` | `PatchNames` | Names for a patch file's Prototype column and id values in one call. Paths the catalogue lacks are named from their file name, unknown ids are left out |

### Updater (`updater.rs`)

| Command | Parameters | Returns | Description |
|---|---|---|---|
| `check_update_available` | `server_exe: Option<String>` | `UpdateInfo` | Probe nightly build URL availability and compare it with the installed build |
| `run_update` | `server_exe: String, backup_targets: Vec<String>` | `UpdateResult` | Full update flow: download → extract → backup → install → keep switched files. Returns the switched files kept on or off, the edited files the update replaced and the pre-update backup's id |
| `cancel_update` | - | `()` | Stop the running update at its next check. The install itself can't be cancelled |
| `create_backup` | `server_exe: String, targets: Vec<String>, label: String` | `BackupManifest` | Create a named backup (off the main thread, refused while the server runs) |
| `list_backups` | `server_exe: String` | `Vec<BackupManifest>` | List backups (newest first) |
| `restore_backup` | `server_exe: String, backup_id: String, roll_back: Option<bool>, only: Option<Vec<String>>` | `Vec<String>` | Back up what the restore replaces, then copy the backup's targets back (just `only` of them when given), and with `roll_back` the server files its update replaced. Returns where copies switched the other way were moved aside |
| `put_back_edits` | `server_exe: String, backup_id: String` | `()` | Back up the update's copies of the edited files a pre-update backup's update replaced, then copy the user's back |
| `delete_backup` | `server_exe: String, backup_id: String` | `()` | Delete backup directory |
| `get_backups_dir` | `server_exe: String` | `String` | Return Backups directory path |

### App Update (`app_update.rs`)

| Command | Parameters | Returns | Description |
|---|---|---|---|
| `check_app_update` | - | `AppUpdateInfo` | Latest release vs running version, with notes for all newer releases |
| `check_overlay_update` | - | `OverlayUpdateInfo` | Installed `MhClientHook.dll` version vs the latest overlay release; reports only, installs nothing |
| `install_app_update` | `version: String` | `()` | Download, verify and install that release, then exit/relaunch |

### Overlay (`overlay/mod.rs`)

| Command | Parameters | Returns | Description |
|---|---|---|---|
| `overlay_status` | - | `OverlayStatus` | `{ state, dir, auto_inject, omega_gear, changes[] }`; state is computed by re-hashing the linked folder |
| `overlay_set_dir` | `dir: String` | `OverlayStatus` | Link a folder (must contain `MhClientHook.dll`); clears trust and injection |
| `overlay_trust` | - | `OverlayStatus` | Hash the code set and store the record in `multiverse.json` |
| `overlay_untrust` | - | `OverlayStatus` | Drop the trust record and turn injection off |
| `overlay_set_auto_inject` | `enabled: bool` | `OverlayStatus` | Toggle injection after launch; refused unless state is `trusted` |
| `overlay_set_omega_gear` | `enabled: bool` | `OverlayStatus` | Save the setting and write `showOmega` now; refused unless state is `trusted` |
| `overlay_inject_now` | - | - | Inject into the game in `LaunchedGame`; refused if none or it has exited; the result arrives as `overlay-inject` |
| `overlay_get_flags` | - | `{ terminals, dps, detector_log }` | Read the overlay's control flags from the linked folder |
| `overlay_set_flag` | `key: String, on: bool` | flags map | Write one allowlisted flag file; refused unless state is `trusted`; applies on the next game launch |

### Profile import (`profile_import/mod.rs`)

Imports server profiles from uploaded JSON files, detected by shape: Bifrost `Bifrost.ServerList.json` (array) plus optional `Bifrost.LaunchConfig.json` (global login, attached to its selected server only if auto-login is on), or the MHO Overlay launcher's `launcher-settings.json` (`servers[]`, per-server logins). Imports always create remote (`is_local: false`) profiles; profiles already present are skipped, matched case-insensitively under a user-chosen strategy (`host + email` default, `host`, `host + name`, or none; under host + email an entry with no email matches any profile on that host). Passwords stay in the backend: the preview only reports whether a login exists, and `import_profiles` re-reads the files. Surfaced as the "Import server profiles" card in Settings → Integrations.

| Command | Parameters | Returns | Description |
|---|---|---|---|
| `preview_profile_import` | `paths: Vec<String>, include_credentials: bool, match_by: MatchBy` | `Preview` | Parse and sanitise the files; lists candidates (flagged `duplicate`) and rejected entries. Writes nothing |
| `detect_profile_sources` | - | `Vec<String>` | Paths of known source files that exist: Bifrost JSON found by walking up from the game exe and checking `Bifrost*` subfolders (newest version wins), the overlay launcher settings in LocalAppData |
| `import_profiles` | `paths: Vec<String>, include_credentials: bool, include: Vec<usize>` | `usize` | Add the profiles at the given positions of the parsed list (the preview rows the user left selected); stores logins only when `include_credentials` |

### Accounts (`accounts.rs`)

| Command | Parameters | Returns | Description |
|---|---|---|---|
| `parse_import_json` | `json_path: String, server_exe: String` | `ImportSummary` | Parse an export file and summarise it, refusing one from another game version than the server's; no database access |
| `scan_download_backups` | `game_exe: String` | `Vec<BackupFileEntry>` | List candidate files in `<game_exe_dir>/Download/` by filename pattern only |
| `list_accounts_for_import` | `server_exe: String` | `Vec<AccountEntry>` | All accounts (id, player name, email) — used for Add conflict pre-checks and Restore target resolution |
| `import_account` | `server_exe: String, json_path: String, mode: String, target_id: Option<String>, overrides: Option<ImportOverrides>` | `Option<String>` | `mode: "add"` creates a new account (`overrides` applies); `mode: "replace"` first copies Account.db to `Account.db.multiverse-<timestamp>.bak` (returned, pruned like the JSON save copies by `[SQLiteDBManager] MaxBackupNumber`), then overwrites `target_id`'s game data only (`overrides` ignored). `target_id` is a decimal string (see u64 Precision) |
| `restore_json_account` | `server_exe: String, json_path: String` | `Option<String>` | JSON storage only: write the export as the server's save (see Account Import and Restore below); returns the file name of the copy made of the previous save |
| `account_exists` | `server_exe: String, email: String` | `bool` | Read-only check that an account with this email is in Account.db; errors while the file is missing or locked |
| `account_password_matches` | `server_exe: String, email: String, password: String` | `bool` | Read-only: recompute the PBKDF2 hash with the stored salt to confirm a console `!account create` / `password` landed |
| `blocked_player_names` | `server_exe: String` | `Vec<String>` | Lowercased names from `Data/PlayerNameBlacklist.txt`, which the server refuses for new accounts |

---

## Svelte Store Reference (`src/lib/store.ts`)

### State Stores

| Export | Type | Description |
|---|---|---|
| `activeTab` | `writable<Tab>` | Current top-level tab |
| `settingsSection` | `writable<SettingsSection>` | Settings section to open next (`client` / `display` / `integrations` / `about`); read once by `AppPanel` on mount, then reset to `client`. Set by the Launch page overlay indicator |
| `activeDataTab` | `writable<DataTab>` | Current data sub-tab (`events` / `tuning` / `store` / `patches`) |
| `serverRunning` | `writable<boolean>` | MHServerEmu process state |
| `serverReady` | `writable<boolean>` | True once the server logs its ready line and reads console commands; reset when it stops |
| `startingServer` / `startingApache` | `writable<boolean>` | A start is in progress |
| `serverStartError` / `apacheStartError` | `writable<string>` | Last start failure, shown on the Rail dots; '' when none |
| `jsonPersistence` | `writable<boolean>` | The linked server stores its one account in a JSON file; set by `refreshPersistence` |
| `unsavedSources` | `writable<Record<string, string>>` | Editors holding unsaved edits, keyed by source; checked by `confirmLeave` |
| `confirmRequest` | `writable<ConfirmRequest \| null>` | The open `askConfirm` question, rendered by `ConfirmDialog` |
| `gameRunning` | `writable<boolean>` | Game process state (matched by `game_exe`'s file name) |
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
| `overlayStatus` | `writable<OverlayStatus \| null>` | Latest overlay status seen by the UI; set by the Integrations card and refreshed by the Launch page (only while inject-on-launch is on) |
| `overlayInject` | `writable<{ ok, message, on_launch } \| null>` | Outcome of the latest overlay injection attempt |
| `overlayLog` | `writable<OverlayLine[]>` | Overlay status-pipe lines (capped at 500); `appendOverlayLog` adds to it |
| `LOG_LEVEL_SEVERITY` | `Record<LogLevel, number>` | Severity order map used by threshold filtering; trace(0) through fatal(6), all(-1) |
| `serverLogFilter` | `writable<LogLevel>` | Server console level filter, persisted to local storage |
| `serverStopping` | `writable<boolean>` | True from a Stop until the server exits, so a remount doesn't re-enable Stop |
| `shutdownCountdownSec` / `shutdownCountdownActive` | `writable` | Timed shutdown countdown, kept across tab switches |
| `eventsRestartPending` | `writable<boolean>` | Events were saved since the running server started. Both Events views show a restart notice (the server reads Events only at startup) |
| `activeLinked` | `derived` | The active linked version, from `appConfig` |
| `activeCaps` | `derived<VersionCaps>` | What the active linked version supports (`versionCaps.ts`). With none linked, 1.52 behaviour |
| `switchBlocked` | `derived<string \| null>` | Why the active version can't change right now (server, game or update running) |
| `guideRequested` | `writable<boolean>` | Get Started opened by hand, kept here so a remount on a new server exe keeps it open |
| `gameDataCopying` | `writable<boolean>` | Get Started's game data copy is running, so reopening the guide can't start a second one |
| `updateRunning` / `updateProgress` / `updateOutcome` | `writable` | Nightly update state, kept across tab switches. The outcome is keyed by server exe |
| `appUpdate` / `appUpdateError` / `appUpdateCheckedAt` | `writable` | Latest app self-update check |
| `updateRemindLater` | `writable<boolean>` | Session only: hides the update badge until next launch |
| `dismissedUpdate` | `writable<string>` | Version the user chose to ignore, persisted to local storage |
| `showUpdateBadge` | `derived<boolean>` | Drives the update dot on the Rail and the About nav item |

### Functions

| Export | Signature | Description |
|---|---|---|
| `askConfirm` | `(opts) => Promise<boolean>` | Ask through the themed `ConfirmDialog`; a newer question cancels an unanswered one |
| `setUnsaved` | `(key, label \| null) => void` | Register or clear an editor's unsaved edits |
| `confirmLeave` | `() => Promise<boolean>` | True when nothing is unsaved or the user agrees to discard it |
| `goTab` / `goDataTab` | `(tab) => Promise<void>` | Navigate after `confirmLeave` |
| `startServer` / `startApache` | `() => Promise<void>` | Start the process, tracking `starting*` and `*StartError` |
| `refreshPersistence` | `(serverExe) => Promise<void>` | Set `jsonPersistence` from `uses_json_persistence` |
| `startUptime` | `() => void` | Start uptime timer (no-op if running) |
| `stopUptime` | `() => void` | Stop and reset uptime timer |
| `appendLog` | `(line: Omit<LogLine, 'id'>) => void` | Append single log line with auto-ID |
| `appendLogBatch` | `(lines: Omit<LogLine, 'id'>[]) => void` | Append batch, cap at 2000 |
| `clearLog` | `() => void` | Clear log |
| `setServerError` | `(message: string) => void` | Set error message |
| `clearServerError` | `() => void` | Clear error message |
| `loadConfig` | `() => Promise<void>` | Load config from backend, apply theme, set active server |
| `upsertServer` | `(server, password) => Promise<void>` | Add/edit server |
| `deleteServer` | `(serverId) => Promise<string>` | Delete server, update active if needed |
| `selectServer` | `(serverId) => Promise<string>` | Set active server and persist |
| `setGameExe` | `(path) => Promise<string>` | Update game exe path |
| `setServerExe` | `(path) => Promise<string>` | Update server exe path |
| `setTheme` | `(theme) => Promise<string>` | Apply theme to DOM and persist |
| `setLaunchOptions` | `(options) => Promise<string>` | Update launch flags |
| `setShutdownConfig` | `(shutdown) => Promise<string>` | Update shutdown delay/message |
| `setBackupTargets` | `(targets) => Promise<string>` | Update backup target list |
| `setConsolePresets` | `(presets) => Promise<string>` | Update command shortcut preset list |
| `setTextSize` | `(size) => Promise<string>` | Persist the text size and apply it as webview zoom (undone if the save fails) |
| `setAdvancedData` | `(advanced) => Promise<string>` | Switch the Data tab between Simple and Advanced |
| `activateLinked` / `upsertLinked` / `deleteLinked` | `(...) => Promise<void>` | Linked version commands. `activateLinked` confirms unsaved edits first and throws the refusal reason |
| `setSchedulerNow` | `(dt: Date) => void` | Store server-parsed UTC time and recompute `eventTimezoneOffset` |
| `markEventsSaved` | `() => void` | Set `eventsRestartPending`, cleared by the next server start |
| `startGameRunningPoll` | `() => void` | Poll `game_is_running` into `gameRunning` |
| `startShutdownCountdown` / `cancelShutdownCountdown` / `clearShutdownCountdown` | | Timed shutdown with broadcast messages |
| `stopServerNow` | `() => Promise<void>` | Ask the server to shut down now, once |
| `runServerUpdate` | `(serverExe, backupTargets) => Promise<void>` | Run a nightly update, tracking `updateRunning` / `updateProgress` / `updateOutcome`. Resolves whatever the outcome |
| `handleCloseRequest` | `(id) => Promise<void>` | Answer the window's close request (see Window Close Hook) |
| `checkAppUpdate` | `() => Promise<void>` | Check for an app update into `appUpdate` |

---

## Tauri Event Reference

### Backend → Frontend

| Event | Payload | Source | Description |
|---|---|---|---|
| `server-log` | `LogLinePayload[]` | `server.rs` batcher thread | Batched log lines (up to 50 per emission, 50ms flush) |
| `server-started` | `ServerStatusPayload` | `server.rs` after spawn | Server process started |
| `server-stopped` | `ServerStatusPayload` | `server.rs` watcher thread | Server process exited (includes exit code) |
| `player-event` | `PlayerEventPayload` | `server.rs` stdout reader | Player login, logout, or full session clear on server stop |
| `apache-stopped` | `()` | `server.rs` Apache poll thread | Apache exited on its own (also logged to the console) |
| `close-requested` | `u64` | `lib.rs` | The window's close button was pressed, answered with `close_ack` and `exit_app` |
| `update-progress` | `UpdateProgressPayload` | `updater.rs` | Update stage + percentage (downloading, extracting, backing_up, installing) |
| `app-update-progress` | `{ stage, pct }` | `app_update.rs` | App self-update stage (downloading, verifying, installing) |
| `overlay-inject` | `{ ok: bool, message: string }` | `overlay/mod.rs` injection thread | Result of an overlay injection (post-launch or Inject now) |
| `overlay-pipe` | `{ event: string, text: string, fields: string[], hits: OverlayHit[] }[]` | `overlay/pipe.rs` reader thread | Display rows from the overlay status pipe; `event` is `loot_hidden` (`fields` = time, category, item, code, rarity, reason), `region_entered` (time, terminal, tier), `tier_resolved`/`mission_state` (text only), `player_death` (`fields` = time, heading, detail; `hits` = damage rows) or `_status` (connection notes, no fields) |

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

Passwords follow a separate path: the frontend sends plaintext to `upsert_server`, which encrypts via AES-256-GCM before storing. The `Server` type sent to the frontend includes `password_enc` and `password_nonce` (encrypted, `#[serde(default)]`), but the frontend never decrypts or sends them back: `upsert_server` keeps the stored values when no new password is given. Passwords are only decrypted at game launch time in `launcher.rs`. The is_local and use_https fields are included in the serialised Server record. For local servers, host is stored as an empty string; the actual siteconfigurl and dashboard URL are computed at use time from Config.ini.

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
  → already stopping? → return (one shutdown per server)
  → write "!server shutdown\n" to stdin, set stopping
  → return immediately
  → (watcher thread detects exit, clears PlayerState, emits "server-stopped")
  → safety net: background thread hard-kills after 60s if still running,
                also clears PlayerState and emits "server-stopped" as fallback

Window close
  → prevent default close, emit "close-requested" { id }
  → window: close_ack(id), handleCloseRequest
      update running? → tell the user, stay open
      confirmLeave (unsaved edits), ask about a running server
      exit_app → exit_gracefully: stop_server, wait for exit (60s timer + 5s)
  → no close_ack within 3s → exit_gracefully from the backend
  → shutdown(): kill_child() (MHServerEmu and Apache), save window state, exit(0)
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
  → return merged values + set of overridden keys + the Config.ini defaults
    + key descriptions read from Config.ini's `; Key: text` comments

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

On load, entries from both files are merged by `SkuId` - MODIFIED entries override base entries. On save, the entry is upserted into the MODIFIED file. On delete, the entry's row is removed from the MODIFIED file, which reverts a changed stock entry or deletes one only MODIFIED has. Stock files are never written, so deleting a stock entry is refused. A `.bak` snapshot of the target file is written before every mutation.

### Calligraphy.sip Reading

```
ensure_catalogue_loaded
  → derive sip path from server_exe
  → if cached path matches → return (cache hit)
  → load_pak: read file, verify KAPG signature, parse entry table
  → extract Blueprint.directory (LZ4 decompress)
  → extract Prototype.directory (LZ4 decompress)
  → parse directories into PrototypeCatalogue
  → build indices: by_id, by_path
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
check_update_available(server_exe)
  → nightly flavour (v48 / v52 / v53) from the game version in the server's MHServerEmu.dll;
    none for a Custom server or an undetected version: UpdateInfo.unavailable_reason says why
    (run_update refuses with the same reason)
  → candidate build dates: today and yesterday (UTC)
  → HTTP range probe (bytes=0-0) of each nightly.link URL, newest first
  → read the installed build date from MHServerEmu.exe's ProductVersion
    ("1.0.3+build20261027131155"), if it has one
  → return UpdateInfo { build_date, download_url, available, installed_build_date, up_to_date }

run_update
  → guard: server must not be running
  → refuse when the installed build is the same or newer
  → download zip with streaming progress events
  → extract to {server_dir}/_update_staging/
  → detect wrapper directory (if zip contains single subdirectory)
  → pre-update backup to {server_dir}/Backups/{timestamp}/: the selected targets (Players
    resolved to the account database and JSON save the config names) plus every file the
    install will replace and the user's switched copies (manifest `replaced`, and `update`
    with the linked version name and the server version before and after)
  → copy staged files over server directory, on a failure roll back from the backup
  → keep_switched_files: a stock file the user had switched off stays off
  → return the files kept on or off, the edited files the update replaced and the backup id
  → clean up temp files (_update.zip, _update_staging/)
```

`cancel_update` is checked during the download, after the extraction and after the backup. A run cancelled after the backup deletes it. Once the install starts it can't be cancelled.

### App Self-Update Flow

```
check_app_update (on launch, and from Settings → About)
  → GET /repos/Cackl/MH-Multiverse/releases?per_page=30
  → drop drafts, prereleases, non-vX.Y.Z tags; keep versions > running
  → AppUpdateInfo { latest, up_to_date, notes[] (newest first, breaking flag each) }

install_app_update(version)
  → guards: release build, PUBKEY set, server not running, version > running
  → re-fetch release by tag (never trusts a URL from the frontend)
  → pick asset: mh-multiverse.exe (portable) or MH.Multiverse_{v}_x64_en-US.msi (exe under Program Files)
  → download .sig + asset (HTTPS, redirects limited to GitHub hosts, size must match API)
  → verify minisign signature → signed file name → exe version resource
  → portable: exe → exe.old, exe.new → exe, spawn exe, shutdown()   (exe.old deleted on next launch)
  → msi: msiexec /i <msi> /passive, shutdown()                       (MajorUpgrade replaces old install)
```

Frontend: `showUpdateBadge` (store) drives the dot on the Rail's Settings tab and the About nav item. "Remind me later" is in-memory (badge returns next launch); "Dismiss this version" stores the version in localStorage (`app-update-dismissed`), so a newer release shows the badge again.

### Overlay Injection

```
Settings → Integrations
  overlay_set_dir(folder) → overlay_trust → hashes stored in multiverse.json
  overlay_set_auto_inject(true)            (only while status == trusted)

launch_game
  → spawn game, store Game (PID + Child) in LaunchedGame
  → overlay::on_game_spawned(config.overlay, game)
      no-op unless auto_inject
      thread "overlay-inject":
        status != trusted?      → fail ("changed since trusted")
        write skip-boot-loading-gate.flag = '0'   ('1' for Inject now)
        WaitForInputIdle(game, 15 s)
        repeat (max 5):
          game exited?            → fail
          status != trusted?      → fail ("changed since trusted")
          inject (LoadLibraryW remote thread, 5 s)
            ok      → spawn pipe reader, emit overlay-inject { ok: true }
            timeout → sleep 2 s, retry
            error   → emit overlay-inject { ok: false, message }
      thread "overlay-pipe":
        connect \\.\pipe\MhTerminalsHelper (retry 30 s)
        read → split lines (8 KB cap) → JSON → allowlisted event → format → emit overlay-pipe
        pipe closes when the game exits → "closed" status line
```

If the game is restarted outside Multiverse, the overlay is not re-injected, and Inject now cannot find it; relaunch from Multiverse.

### Account Import and Restore

```
scan_download_backups(game_exe)
  → derive Download/ as a sibling of game_exe
  → list files matching 0x{IdHex}_{PlayerName}_{yyyy-MM-dd_HH.mm.ss}.json
  → non-matching files skipped, not errored
  → return [{ path, player_name, modified }, ...] sorted newest first

parse_import_json(json_path, server_exe)
  → deserialise the export, refuse one from another game version than the server's
  → return a summary only (no DB access)

import_account(mode: "add", overrides)
  → resolve final email/player_name/password (overrides win, else export values)
  → EMAIL_CONFLICT / NAME_CONFLICT if already registered
  → INSERT Account row
  → insert_player_data: remap every entity's DbGuid, rewrite ContainerDbGuid
    references through that map, insert Player/Avatar/TeamUp/Item/ControlledEntity

import_account(mode: "replace", target_id)
  → VACUUM INTO a copy of Account.db next to it (takes in the WAL), prune old copies
  → verify target_id exists — no other checks against the target
  → DELETE Item, ControlledEntity, Avatar, TeamUp, then Player rows in that
    order (foreign keys are off, so ON DELETE CASCADE never fires)
  → insert_player_data against target_id, same remap logic as Add
  → Account row's Email/PlayerName/Password/UserLevel/Flags untouched

restore_json_account(server_exe, json_path)   [JSON storage only]
  → refuse while the server runs; validate with read_and_parse
  → [JsonDBManager] FileName must be a plain name in Data/
  → copy the current save to <FileName>.multiverse-<timestamp>.bak, keeping
    the newest max(MaxBackupNumber, 1) of those copies
  → write the file as is with UserLevel = 2 (no remap: the loader doesn't check the Id)
  → write its PlayerName to [JsonDBManager] PlayerName in ConfigOverride.ini
```

The remap step means an imported file's original account identity is irrelevant to whether it can be written — Replace will happily attach any file's data to any target. `ServerModal`'s Restore tab is the only safeguard: it resolves the target account from the profile's own `email` and hard-blocks the Restore button if the selected file's email/player name don't match that account, since nothing on the backend would otherwise catch a mismatched file.

---

## Key Design Decisions

### u64 Precision Across the JS Boundary

Prototype runtime IDs, GUIDs, catalog SKU IDs, and account IDs are all integer values that can exceed JavaScript's `Number.MAX_SAFE_INTEGER` (2^53 - 1). The catalog system handles this with dual types: `CatalogEntryDisk` uses raw `u64` for on-disk serialisation, while the frontend-facing `CatalogEntry` represents these as `String`. GuidItem conversion happens in `guid_disk_to_view` and `guid_view_to_disk`; `SkuId` is converted inline in `disk_to_view` and `view_to_disk` since it's a scalar field rather than a nested collection. Calligraphy prototype IDs and GUIDs are similarly transported as decimal strings. `AccountEntry.id` (from `list_accounts_for_import`) and `import_account`'s `target_id` follow the same pattern - account IDs are far past `Number.MAX_SAFE_INTEGER` in practice, so passing them as raw JSON numbers silently corrupted the value and broke Restore's account lookup entirely until fixed.

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

### Replace Never Touches Identity (Accounts)

`do_replace` only ever deletes and reinserts an account's Player/Avatar/TeamUp/Item/ControlledEntity rows — Email, PlayerName, Password, UserLevel, and Flags on the Account row itself are out of scope, even though an earlier version of this function did write them. The entity ID remap in `insert_player_data` is what makes this safe: every DbGuid in the imported file is rewritten relative to the target account, not the file's original account, so Replace has no functional dependency on the file "belonging" to the target in any sense the database would enforce. That's a deliberate tradeoff - it makes Replace simple and keeps credentials/permissions stable across a restore, but it also means the backend alone can't tell a legitimate restore from a mismatched file. The mismatch check lives entirely in `ServerModal`'s Restore tab (comparing the parsed file's email/player name against the resolved target account) rather than in `accounts.rs`.

---

## Releasing (Signed Updates)

The in-app updater only installs files signed with the release key. The signature (minisign, Ed25519) carries no identity: just a random key ID, the signature, and a trusted comment with a timestamp and the file name.

**One-time setup**
1. In PowerShell, run `New-Item -ItemType Directory -Force "$HOME\.mhm"` then `npx tauri signer generate -w "$HOME\.mhm\update.key"`, and set a password. (Don't use `%USERPROFILE%`: only cmd expands it, so PowerShell and Git Bash create a folder with that literal name.) Back the key up: if it is lost, installed copies can't verify new releases and users must download one release manually.
2. Paste the printed public key into `PUBKEY` in `src-tauri/src/app_update.rs`. Builds with an empty `PUBKEY` can check for updates but refuse to install.

**Each release**
1. `npm run bump-version -- <patch|minor|major>` then `npm run tauri build`.
2. `npm run sign-release`. This writes `.sig` files next to `target/release/mh-multiverse.exe` and the msi.
3. Upload the exe, the msi and both `.sig` files to the GitHub release (GitHub turns spaces in file names into dots; the updater accounts for this). The tag must be `vX.Y.Z` and the release must not be marked prerelease.
4. If the release breaks compatibility, add a `## Breaking Changes` heading to its notes. Users see a warning before updating, even when skipping past that version.

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
│   │   │   │   ├── LiveTuningDataMultiverse.json (Server Rates, created by MH Multiverse)
│   │   │   │   └── Events/
│   │   │   │       ├── Weekly/
│   │   │   │       └── Multiverse/   ← custom events' tuning files (created by MH Multiverse)
│   │   │   ├── Patches/          ← PatchData*.json files (enabled)
│   │   │   │   ├── PatchDataZMultiverseTweaks.json (Game Tweaks, created by MH Multiverse)
│   │   │   │   └── Off/          ← PatchData*.json files (disabled)
│   │   │   └── MTXStore/         ← Catalog*.json + *MODIFIED.json files
│   │   │       ├── CatalogMultiverse.json  (Simple view's added items)
│   │   │       ├── MultiverseStore.json    (Simple view's sale record)
│   │   │       └── Off/          ← switched-off catalog files
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