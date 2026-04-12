# CHANGELOG
---
<br>

---
# Version 1.2.2 Release

## What's Changed

### Offline Patch Support Bugfix & Console Enhancements
- Introduced a new `Patched Client` toggle under Settings > Launch Options. The flag is used by `launcher.rs`, which should now pass in the correct siteconfigurl argument (with `/Dashboard/SiteConfig.xml` instead of just `SiteConfig.xml` after `localhost`)
- Added Port pre-check
  - New block in `start_server` function utilises `TcpListener::connect` to probe the Config/ConfigOverride specified Webfrontend port
  - If the port is in use, raises a Fatal log message with description
- Log messages for server and apache starts update to match style of regular MHServerEmu entries
- Log filter now defaults to threshold mode, e.g Info shows Warn, Error etc. Can be switched back via a new setting in Config.

### Other
- Added button in Events page to open the LiveTuning folder directly, for parity with the other pages under Data.
- Changed generated bundle folder name to `MH-Multiverse-Bundles`, but this will be revisted later for patched client configurations to ensure the files can be read properly
- Created CHANGELOG.md under `docs/`, and moved ARCHITECTURE.md there.

<br>

---
# Version 1.2.1 Release

## What's Changed

### Server Profile Improvements
- Fixed a class of bugs where user-entered schemes (http://) or path suffixes in the host field would corrupt the game's siteconfigurl on launch and/or cause a mismatch with the Dashboard links. Host names are now normalised to prevent issues and reduce server setup confusion
- Added a Local toggle to the server modal — local servers derive the correct port and dashboard path from Config.ini automatically at launch time rather than requiring manual host entry
- Added a Use HTTPS toggle for remote servers

### Server Modal & Launch Panel UX
- Added tooltips to new/edit Server modal to help guide setup
- Added a Home button to the launch bar for remote servers, linking to the server's root URL alongside the existing Dashboard button
- Dashboard URL now reads DashboardUrlPath from Config.ini rather than hardcoding /Dashboard/
- Server card and detail view now display "Local Server" instead of a blank or incorrect host string for local profiles

<br>

---
# Version 1.2.0 Release

## What's Changed

### Events Overhaul
- New tab in Data: "Events"
- `EventPanel`, `EventRuleEditorModal` and `EventDefinitionEditorModal` components created for managing the new Events system
- Events page's side panel shows a break down of active/inactive scheduling rules, which can be clicked on to open a detailed view
- Events dashboard shows "Active Now" events, and a full list of Event definitions from the JSON files under `LiveTuning/Events/`
- Updates to rules and event definitions are saved to `EventsOverride.json` and `EventsScheduleOverride.json`. Buttons are available to either reset the files to default, or if MHServerEmu updates contain new events in `Events.json`, merge the changes instead.

### Player Management
- New PlayersPanel and PlayerCard components created
- Searchable list of logged in players added to the Server tab, and accessible via a "Players" button
- PlayerCard shows basic user information retrieved from `Account.db`, and provides quick access to moderation commands like kick, ban, whitelist and set user account level

### Other
- Added two new themes with better contrast until a full sweep of the UI is conducted

<br>

---
# Version 1.0.0 Release

There was a growing list of enhancements I wanted to get to before the v1.0.0 release, but given the included features so far and my best efforts to eliminate bugs, I thought it'd be best to share with the community first. I'll be getting straight back to the updates after this!

---

## Overview

### Game Launching
- Multi-server profile management with per-server credentials
- Encrypted password storage via OS keychain (Windows Credential Manager)
- Auto-login support - email and password passed as command-line arguments
- Configurable launch flags: skip startup movies, skip motion comics, no sound, client logging, custom resolution, robocopy, no-Steam mode

### Local Server Management
- Start and stop MHServerEmu with stdout/stderr log streaming to an in-app console
- Structured log parsing (timestamp, level, category, message) with level-based colouring and filtering
- Interactive command input with autocomplete drawn from the MHServerEmu command list
- Timed shutdown with configurable delay and broadcast message
- Independent Apache start/stop for players running in offline mode without the reverse proxy
- Windows Job Object integration - child processes are killed automatically if MH Multiverse crashes or is force-closed

### Server Configuration (INI Editor)
- Visual editor for MHServerEmu's `Config.ini` / `ConfigOverride.ini` with grouped sections, tooltips, and type-appropriate controls (toggles, dropdowns, numeric inputs)
- Diff-only saving - only values that differ from `Config.ini` defaults are written to `ConfigOverride.ini`
- Per-section reset to defaults
- Currently displays a subset of the full `Config.ini` options for simplicity, though more may be added in future

### Live Tuning Editor
- Scan, create, edit, and toggle `LiveTuningData*.json` files in `Data/Game/LiveTuning`
- Enable/disable tuning files via `OFF_` filename prefix convention
- Tag-based organisation (Core, Event, Custom) with favourites pinning
- Category-aware setting enum validation (Global, World Entity, Power, Region, Loot, etc.)
- Prototype path picker backed by Calligraphy.sip for prototype-scoped tuning entries

### Data Patching Editor
- Scan, create, edit, and toggle `PatchData*.json` files in `Data/Game/Patches`
- Enable/disable patch files by moving between `Patches/` and `Patches/Off/`
- Per-entry enable/disable, prototype path, field path, value type, and value editing
- Prototype picker and value type dropdown matching MHServerEmu's supported patch value types

### MTX Store Catalog Editor
- Load, create, edit, and delete catalog entries across `Catalog*.json` files in `Data/Game/MTXStore`
- Non-destructive editing - saves always write to `*MODIFIED.json` sidecar files; base catalog files are never modified
- Automatic `.bak` snapshots before every write
- Prototype item picker with display name resolution (embedded + custom override maps)
- Type and modifier assignment matching MHServerEmu's catalog type system
- Bundle HTML page generation for in-game store display, with customisable CSS

### Server Updates & Backups
- One-click update from MHServerEmu nightly builds (download, extract, install)
- Configurable backup targets (INI files, LiveTuning directory, account database, full Data directory)
- Automatic pre-update backup with post-update restore of user-modified files
- Manual backup creation, restore, and deletion with manifest tracking

### Application Settings
- Game and server executable path configuration with file browser
- Five colour themes

### Calligraphy.sip Integration
- Binary pak reader for `Calligraphy.sip` - parses blueprint and prototype directory records
- Prototype search by path and display name, filtered by category/blueprint
- Runtime prototype ID and GUID resolution for tuning, patching, and store editors
- Cached per server executable path, automatically rebuilds when the server changes

---