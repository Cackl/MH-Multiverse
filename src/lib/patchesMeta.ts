// ── Patches payload types ────────────────────────────────────────────────────
// Mirror the Rust structs in patches.rs. Previously re-declared independently
// in PatchesPanel.svelte and PatchEditorModal.svelte.

export interface PatchFileInfo {
  file_name: string
  enabled: boolean
}

export interface PatchEntry {
  Enabled: boolean
  Prototype: string
  Path: string
  Description: string
  ValueType: string
  Value: unknown
}
