# MH Multiverse test harness

Rust unit tests for `src-tauri/`, plus Vitest unit tests for `src/lib/`.
No external setup is required — every fixture here is either a small
hand-crafted file or built at test time (including the `Calligraphy.sip` and
`Account.db` fixtures, which are synthesized in code rather than checked in,
since the real files are large binary formats not worth committing).

## Running

```bash
npm run test:rust   # cargo test, from src-tauri/
npm run test        # vitest, frontend lib/ tests
npm run test:all    # both
```

## Layout

- `fixtures/` — small JSON/INI files used by the Rust tests (Config.ini,
  a LiveTuning tree with root/OFF_/subdirectory/event-owned files, a Patches
  tree, an MTXStore catalog + MODIFIED sidecar, and a sample account export).
- `rust/test_*.rs` — one file per `src-tauri/src/*.rs` module. Each is spliced
  into its module via `#[cfg(test)] #[path = "..."] mod tests;` at the bottom
  of the corresponding source file, so tests can call private functions
  directly (`use super::*;`) instead of only the `#[tauri::command]`-wrapped
  public API.
- `frontend/*.test.ts` — one file per `src/lib/*.ts` module with actual logic
  (files that are pure static data, like `configMeta.ts`/`serverCommands.ts`,
  have nothing to unit test and are skipped). They run in a plain `node`
  environment with no DOM, so UI behaviour worth testing (for example the
  unsaved-changes guard and confirm requests in `store.ts`) lives in `src/lib`
  and the Svelte files stay thin callers.
- `frontend/setup.ts` — stubs the Tauri `invoke`/`listen` modules and
  `localStorage` for every frontend test; override `invoke` per test with
  `vi.mock()` when a test needs a specific reply.
- Some modules keep their tests beside the code instead of in `rust/`:
  inline in `app_update.rs`, and in `tests.rs` files under `overlay/` and
  `profile_import/`.

## What's deliberately not covered

- **Tauri command dispatch.** Commands whose signature takes `tauri::State<T>`
  or `AppHandle` (`search_prototypes`, `resolve_display_name`,
  `generate_bundle_html`, `import_account`, `run_update`, everything in
  `server.rs`) can't be called from a unit test — `tauri::State` has no public
  constructor outside a running app. Where the command is a thin wrapper
  around testable inner logic (e.g. `import_account` → `do_add_with_overrides`
  / `do_replace`), the inner logic is tested directly instead.
- **Calligraphy.sip parsing realism.** `test_calligraphy.rs` builds a
  synthetic `.sip` in the exact binary layout the parser expects, rather than
  requiring a real ~80MB file. This exercises every code path (including the
  truncated/bad-signature error paths) deterministically, but can't catch a
  real MHServerEmu file format diverging from what the Rust code assumes —
  only a real fixture would catch that.
- **OS keychain interaction.** `test_config.rs`'s encrypt/decrypt tests hit
  the real Windows Credential Manager entry the running app uses (service
  `mh-multiverse`). Fine locally; would need mocking to run in a keychain-less
  CI environment.
- Svelte component rendering, CSS/theming, and MHServerEmu's own behaviour —
  out of scope for this harness (see the plan doc this was built from,
  `Future Updates/MH-Multiverse-Test-Harness-Plan-2026-09-05.md`, for the
  reasoning and E2E notes for future work).

## When to add a test

- Fixing a bug: write the test first (red), fix the code (green).
- New Tauri command: add a Rust unit test for the inner logic it delegates to.
- New file format handling: add a round-trip test against a fixture.
- New UI behaviour with real logic in it: put the logic in `src/lib/*.ts` and
  test it there; Svelte components can't be rendered by this harness.
- Bundle slugs now come from the frontend only (the Rust `slugify()` and its
  parity test were removed), so there is no cross-language copy to keep in sync.
