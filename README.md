# MH Manifold — Developer Handover

Custom launcher and local server controller for Marvel Heroes (MHServerEmu).
Built with Tauri 2, Svelte 5, and Rust.

---

## Stack

| Layer | Choice |
|---|---|
| App framework | Tauri 2 |
| Frontend | Svelte 5 + Vite 8 |
| Backend | Rust (stable, 1.93+) |
| Encryption | `aes-gcm` 0.10 via OS keychain (`keyring` 2) |
| Process detection | `sysinfo` |

---

## Project Structure

```
mh-manifold/
├── src/
│   ├── App.svelte              Root component — tab router, loads config on mount
│   ├── app.css                 All design tokens and shared styles (CSS variables), data-themes.
│   ├── main.ts                 Svelte 5 entry point (uses mount(), not new App())
│   ├── lib/
│   │   └── store.ts            Global Svelte stores — config, server/game/apache
│   │                           state, log, uptime timer
│   └── components/
│       ├── Titlebar.svelte     App header with hex logo and server status pill
│       ├── TabBar.svelte       Tab navigation (Launch / Local Server / Config) [not in use, see Rail]
│       ├── LaunchPanel.svelte  Server list, credentials display, game exe, launch button
│       ├── Rail.svelte         Side panel for navigation (Launch / Local Server / App)
│       ├── ServerModal.svelte  Add/edit server dialog
│       ├── ServerPanel.svelte  Local server start/stop, log view, command input, config component access
│       └── ConfigPanel.svelte  ConfigOverride.ini editor with schema, tooltips, toggles
|
├── src-tauri/
│   ├── src/
│   │   ├── main.rs             Entry point — calls app_lib::run()
│   │   ├── lib.rs              Tauri builder — plugins, managed state, window close hook,
│   │   │                       all command registrations
│   │   ├── config.rs           AppConfig/Server structs, AES-256-GCM encryption,
│   │   │                       keychain key management, manifold.json persistence
│   │   ├── launcher.rs         launch_game command, game_is_running (sysinfo poll)
│   │   ├── server.rs           ServerProcess/ServerState, start/stop MHServerEmu + Apache,
│   │   │                       stdout streaming, Job Object cleanup, apache_is_running
│   │   └── ini.rs              Config.ini/ConfigOverride.ini read/write with diff-only saving
│   ├── capabilities/
│   │   └── default.json        Tauri capability grants (dialog, opener)
│   ├── Cargo.toml
│   └── tauri.conf.json         Window config — title, min size, native decorations
```

---

## Data Flow

### Config persistence
- `manifold.json` stored in OS app data dir via `tauri::path::app_data_dir()`
- Passwords AES-256-GCM encrypted; key stored in OS keychain (Windows Credential Manager)
- Frontend `Server` type never includes `password_enc`/`password_nonce` — encryption
  happens entirely in Rust
- `upsert_server` command takes a plaintext password and encrypts before saving
- Empty password string = keep existing (for edits)

### Server lifecycle
- `start_server` spawns Apache (`../Apache24/bin/httpd.exe` relative to MHServerEmu dir)
  with `APACHE_SERVER_ROOT` env var set, then spawns MHServerEmu with piped stdout/stderr
- Both processes assigned to a Windows Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`
  — hard-killing Manifold kills both child processes automatically
- `server-log` Tauri events streamed from stdout to frontend
- `server-stopped` event fired when MHServerEmu exits (planned or crash)
- Window close hook in `lib.rs` calls `kill_child()` before `app.exit(0)`

### Config editor
- `read_config` reads `Config.ini` (defaults) + `ConfigOverride.ini` (overrides), merges them
- `write_config` diffs new values against defaults — only writes keys that differ
- `reset_config_section` removes all overrides for a given INI section
- Frontend schema in `ConfigPanel.svelte` maps UI fields to INI section/key pairs

---

## Rust Crates

```toml
tauri = "2.10.3"
tauri-plugin-dialog = "2"
tauri-plugin-log = "2"
tauri-plugin-opener = "2"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
aes-gcm = "0.10"
base64 = "0.22"
keyring = "2"
tokio = { version = "1", features = ["process", "io-util", "rt"] }
windows = { version = "0.61", features = ["Win32_System_JobObjects", "Win32_Foundation", "Win32_Security"] }
sysinfo = "..."
```

---

## Known Issues & Outstanding Work

### Apache / local server
- Apache is spawned with `Stdio::null()` so its errors are not captured. Consider piping
  `stderr` to the log view for diagnostics.
- No reverse proxy setup guidance in-app — users need Apache running for `localhost/SiteConfig.xml`
  to resolve. An alternative is the MH exe patcher at `crypto137.github.io/mh-exe-patcher/`
  which allows connecting directly on port 8080 (disables credential encryption, local use only).

### Features not yet implemented
- **Phase 6 polish items** (from plan):
  - First-run onboarding flow (currently silently shows empty state)
  - Build & distribution (`npm run tauri build`)

---

## Development Commands

```cmd
# Dev server
npm run tauri dev

# Type check
npm run check

# Production build
npm run tauri build
```

Config file location (runtime): `%APPDATA%\com.mhmanifold.app\manifold.json`

---

## MHServerEmu Directory Structure Expected

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
│   ├── Data/Game/LiveTuning/     ← directory for Live Tuning JSON files
│   ├── MHServerEmu.exe           ← this is what you point server_exe at
│   ├── Config.ini
│   └── ConfigOverride.ini        (created/managed by Manifold)
└── StartServer.bat
```

The `server_exe` config field should point to `MHServerEmu/MHServerEmu.exe`.
Apache path is derived as `../../Apache24/bin/httpd.exe` relative to the exe.

---

## Tauri Commands Reference

| Command | Module | Description |
|---|---|---|
| `get_config` | config | Load full AppConfig from manifold.json |
| `cmd_save_config` | config | Save AppConfig directly |
| `upsert_server` | config | Add/edit server, encrypts password |
| `delete_server` | config | Remove server by ID |
| `set_active_server` | config | Update active_server_id |
| `set_game_exe` | config | Update game_exe path |
| `set_server_exe` | config | Update server_exe path |
| `launch_game` | launcher | Spawn MarvelHeroesOmega.exe with args |
| `game_is_running` | launcher | Poll sysinfo for MarvelHeroesOmega.exe |
| `start_server` | server | Spawn Apache + MHServerEmu |
| `stop_server` | server | Kill both processes |
| `send_command` | server | Write line to MHServerEmu stdin |
| `server_is_running` | server | Check MHServerEmu child process |
| `apache_is_running` | server | Check Apache child process |
| `read_config` | ini | Parse Config.ini + ConfigOverride.ini |
| `write_config` | ini | Diff-write to ConfigOverride.ini |
| `reset_config_section` | ini | Remove section from ConfigOverride.ini |

---

## Svelte Store Reference (`src/lib/store.ts`)

| Export | Type | Description |
|---|---|---|
| `activeTab` | `writable<Tab>` | Current tab: launch / server / config |
| `serverRunning` | `writable<boolean>` | MHServerEmu process state |
| `gameRunning` | `writable<boolean>` | MarvelHeroesOmega.exe process state |
| `apacheRunning` | `writable<boolean>` | Apache process state |
| `uptimeSec` | `writable<number>` | Server uptime counter (seconds) |
| `appConfig` | `writable<AppConfig>` | Full config including server list |
| `activeServerId` | `writable<string>` | Currently selected server UUID |
| `serverLog` | `writable<LogLine[]>` | Log lines (capped at 2000) |
| `startUptime()` | function | Start uptime timer (no-op if already running) |
| `stopUptime()` | function | Stop and reset uptime timer |
| `appendLog()` | function | Append a line to serverLog with auto-ID |
| `clearLog()` | function | Clear serverLog |
| `loadConfig()` | async function | invoke get_config and populate stores |
| `upsertServer()` | async function | invoke upsert_server |
| `deleteServer()` | async function | invoke delete_server |
| `selectServer()` | async function | Set active server and persist |
| `setGameExe()` | async function | Update game exe path |
| `setServerExe()` | async function | Update server exe path |
