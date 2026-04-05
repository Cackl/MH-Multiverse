<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'

  // ── Props ──────────────────────────────────────────────────────────────────

  /** Existing entry to edit. Null when creating a new entry. */
  export let entry: PatchEntry | null = null
  export let serverExe: string
  export let onClose:  () => void
  export let onSave:   (entry: PatchEntry) => void
  export let onDelete: () => void

  // ── Types ──────────────────────────────────────────────────────────────────

  interface PatchEntry {
    Enabled:     boolean
    Prototype:   string
    Path:        string
    Description: string
    ValueType:   string
    Value:       unknown
  }

  interface PrototypeMatch {
    path:           string
    blueprint:      string
    display_name:   string
    leaf:           string
    prototype_id:   string
    prototype_guid: string
  }

  // ── Constants ──────────────────────────────────────────────────────────────

  const VALUE_TYPES = [
    'PrototypeId', 'PrototypeDataRef', 'PrototypeGuid',
    'Integer', 'Float', 'Boolean', 'Enum', 'LocaleStringId',
    'Vector3', 'PrototypeId[]', 'PrototypeDataRef[]', 'Prototype[]', 'Properties',
  ]

  // ValueTypes where selecting a prototype auto-fills Value from prototype_id.
  const ID_TYPES:   ReadonlySet<string> = new Set(['PrototypeId', 'PrototypeDataRef', 'PrototypeId[]', 'PrototypeDataRef[]'])
  // ValueTypes where selecting a prototype auto-fills Value from prototype_guid.
  const GUID_TYPES: ReadonlySet<string> = new Set(['PrototypeGuid'])
  // ValueTypes that require a JSON textarea instead of a plain text input.
  const COMPLEX:    ReadonlySet<string> = new Set(['Vector3', 'PrototypeId[]', 'PrototypeDataRef[]', 'Prototype[]', 'Properties'])

  // ── State ──────────────────────────────────────────────────────────────────

  $: isNew = entry === null

  let draft: PatchEntry = entry
    ? { ...entry }
    : { Enabled: true, Prototype: '', Path: '', Description: '', ValueType: 'PrototypeId', Value: 0 }

  // Last prototype selected from search — retained so Value can be re-derived
  // if the user changes ValueType after choosing a prototype.
  let lastMatch: PrototypeMatch | null = null

  // Prototype search
  let protoQuery    = draft.Prototype
  let protoResults: PrototypeMatch[] = []
  let protoSearching = false
  let protoDropOpen  = false
  let protoDebounce: ReturnType<typeof setTimeout> | null = null
  let protoInputEl:  HTMLTextAreaElement

  // Value is edited as a string regardless of underlying type.
  let valueStr = serializeValue(draft.Value, draft.ValueType)

  // Delete confirm
  let deleteConfirming = false

  // ── Derived ────────────────────────────────────────────────────────────────

  $: needsComplex  = COMPLEX.has(draft.ValueType)
  $: parseError    = validateValueStr(valueStr, draft.ValueType)
  $: canSave       = !parseError && protoQuery.trim().length > 0

  // ── Value helpers ──────────────────────────────────────────────────────────

  function serializeValue(v: unknown, vt: string): string {
    if (v === null || v === undefined) return ''
    if (COMPLEX.has(vt) || typeof v === 'object') return JSON.stringify(v, null, 2)
    return String(v)
  }

  function parseValue(s: string, vt: string): unknown {
    if (vt === 'Boolean')  return s === 'true'
    if (vt === 'Integer')  return parseInt(s, 10)
    if (vt === 'Float')    return parseFloat(s)
    if (COMPLEX.has(vt))   { try { return JSON.parse(s) } catch { return s } }
    // ID types: numeric where possible. Values > MAX_SAFE_INTEGER lose JS precision —
    // use prototype search auto-fill to avoid manual entry of large IDs.
    const n = Number(s)
    return isNaN(n) ? s : n
  }

  function validateValueStr(s: string, vt: string): string {
    if (!s.trim()) return ''
    if (vt === 'Integer' && isNaN(parseInt(s, 10)))  return 'Expected integer.'
    if (vt === 'Float'   && isNaN(parseFloat(s)))    return 'Expected number.'
    if (COMPLEX.has(vt)) {
      try { JSON.parse(s) } catch { return 'Invalid JSON.' }
    }
    return ''
  }

  // When ValueType changes, re-derive Value from lastMatch if applicable.
  function onValueTypeChange() {
    if (!lastMatch) return
    if (ID_TYPES.has(draft.ValueType))   valueStr = lastMatch.prototype_id
    else if (GUID_TYPES.has(draft.ValueType)) valueStr = lastMatch.prototype_guid
  }

  // ── Prototype search ───────────────────────────────────────────────────────

  function onProtoInput() {
    protoQuery      = protoQuery.replace(/[\r\n]/g, '')
    draft.Prototype = protoQuery
    lastMatch       = null
    if (protoDebounce) clearTimeout(protoDebounce)
    if (protoQuery.trim().length < 2) { protoDropOpen = false; protoResults = []; return }
    protoDebounce = setTimeout(runSearch, 220)
  }

  async function runSearch() {
    protoSearching = true
    try {
      protoResults  = await invoke<PrototypeMatch[]>('search_prototypes', { serverExe, query: protoQuery })
      protoDropOpen = protoResults.length > 0
    } catch {
      protoResults  = []
      protoDropOpen = false
    } finally {
      protoSearching = false
    }
  }

  function selectProto(match: PrototypeMatch) {
    protoQuery      = match.path
    draft.Prototype = match.path
    lastMatch       = match
    protoDropOpen   = false

    if (ID_TYPES.has(draft.ValueType))        valueStr = match.prototype_id
    else if (GUID_TYPES.has(draft.ValueType)) valueStr = match.prototype_guid

    protoInputEl?.focus()
  }

  // ── Save / delete ──────────────────────────────────────────────────────────

  function handleSave() {
    if (!canSave) return
    onSave({ ...draft, Prototype: protoQuery.trim(), Value: parseValue(valueStr, draft.ValueType) })
  }

  function handleDelete() {
    if (!deleteConfirming) { deleteConfirming = true; return }
    onDelete()
  }

  // ── Keyboard / click-outside ───────────────────────────────────────────────

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      if (protoDropOpen)    { protoDropOpen = false; return }
      if (deleteConfirming) { deleteConfirming = false; return }
      onClose()
    }
  }

  function onWindowClick(e: MouseEvent) {
    if (protoDropOpen && !(e.target as Element).closest('.proto-wrap')) {
      protoDropOpen = false
    }
  }
</script>

<svelte:window on:keydown={onKeydown} on:click={onWindowClick} />

<div
  class="modal-backdrop"
  role="presentation"
  on:click={e => { if (e.target === e.currentTarget) onClose() }}
>
  <div class="editor-modal" role="dialog" aria-modal="true" tabindex="-1">

    <!-- ── Header ── -->
    <div class="modal-header">
      <div class="modal-title">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
          <polyline points="14 2 14 8 20 8"/>
          <line x1="16" y1="13" x2="8" y2="13"/>
          <line x1="16" y1="17" x2="8" y2="17"/>
        </svg>
        <span class="modal-display-name">{isNew ? 'New Patch Entry' : 'Edit Patch Entry'}</span>
      </div>
      <button class="close-btn" aria-label="Close" on:click={onClose}>
        <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5">
          <line x1="3.5" y1="3.5" x2="10.5" y2="10.5"/>
          <line x1="10.5" y1="3.5" x2="3.5" y2="10.5"/>
        </svg>
      </button>
    </div>

    <!-- ── Form body ── -->
    <div class="modal-body">

      <!-- Enabled -->
      <div class="field-row">
        <span class="field-label">Enabled</span>
        <button
          aria-label="Enabled"
          class="toggle-switch"
          class:on={draft.Enabled}
          on:click={() => draft.Enabled = !draft.Enabled}
        ></button>
      </div>

      <!-- Description -->
      <div class="field-row field-row-top">
        <label class="field-label" for="pe-desc">Description</label>
        <textarea
          id="pe-desc"
          class="field-textarea field-textarea-sm"
          rows="2"
          placeholder="Human-readable note"
          bind:value={draft.Description}
          on:input={() => { draft.Description = draft.Description.replace(/[\r\n]/g, '') }}
          spellcheck="false"
        ></textarea>
      </div>

      <!-- Prototype + search -->
      <div class="field-row field-row-top">
        <label class="field-label" for="pe-proto">Prototype</label>
        <div class="proto-wrap">
          <div class="proto-input-row">
            <textarea
              id="pe-proto"
              class="field-textarea field-textarea-sm"
              rows="2"
              placeholder="Search or paste prototype path..."
              bind:value={protoQuery}
              bind:this={protoInputEl}
              on:input={onProtoInput}
              autocomplete="off"
              spellcheck="false"
            ></textarea>
            {#if protoSearching}
              <span class="proto-hint">Searching...</span>
            {/if}
          </div>

          {#if protoDropOpen}
            <div class="proto-dropdown">
              {#each protoResults as match (match.path)}
                <button class="proto-option" on:click={() => selectProto(match)}>
                  <span class="proto-opt-name">
                    {match.display_name !== match.path ? match.display_name : match.leaf}
                  </span>
                  <span class="proto-opt-path">{match.path}</span>
                  <span class="proto-opt-bp">{match.blueprint}</span>
                </button>
              {/each}
            </div>
          {/if}
        </div>
      </div>

      <!-- Path -->
      <div class="field-row field-row-top">
        <label class="field-label" for="pe-path">Path</label>
        <textarea
          id="pe-path"
          class="field-textarea field-textarea-sm"
          rows="2"
          placeholder="e.g. Choices[0].Weight"
          bind:value={draft.Path}
          on:input={() => { draft.Path = draft.Path.replace(/[\r\n]/g, '') }}
          spellcheck="false"
        ></textarea>
      </div>

      <!-- Value Type -->
      <div class="field-row">
        <label class="field-label" for="pe-vtype">Value Type</label>
        <select
          id="pe-vtype"
          class="field-select"
          bind:value={draft.ValueType}
          on:change={onValueTypeChange}
        >
          {#each VALUE_TYPES as vt}
            <option value={vt}>{vt}</option>
          {/each}
        </select>
      </div>

      <!-- Value -->
      <div class="field-row field-row-top">
        <label class="field-label" for="pe-value">Value</label>
        <div class="value-wrap">
          {#if draft.ValueType === 'Boolean'}
            <select id="pe-value" class="field-select" bind:value={valueStr}>
              <option value="true">true</option>
              <option value="false">false</option>
            </select>
          {:else if needsComplex}
            <textarea
              id="pe-value"
              class="field-textarea"
              class:input-error={!!parseError}
              rows="5"
              bind:value={valueStr}
              spellcheck="false"
            ></textarea>
          {:else}
            <input
              id="pe-value"
              type="text"
              class="field-input"
              class:input-error={!!parseError}
              bind:value={valueStr}
              spellcheck="false"
            />
          {/if}

          {#if parseError}
            <span class="value-feedback value-error">{parseError}</span>
          {:else if lastMatch && (ID_TYPES.has(draft.ValueType) || GUID_TYPES.has(draft.ValueType))}
            <span class="value-feedback value-hint">
              Auto-filled from <em>{lastMatch.leaf}</em>
              · {GUID_TYPES.has(draft.ValueType) ? 'GUID' : 'runtime ID'}
            </span>
          {/if}
        </div>
      </div>

    </div><!-- modal-body -->

    <!-- ── Footer ── -->
    <div class="modal-footer">
      <div class="footer-left">
        {#if !isNew}
          {#if deleteConfirming}
            <span class="delete-label">Delete this entry?</span>
            <button class="btn btn-sm btn-red" on:click={handleDelete}>Confirm</button>
            <button class="btn btn-sm btn-outline" on:click={() => deleteConfirming = false}>Cancel</button>
          {:else}
            <button class="btn btn-sm btn-red" on:click={handleDelete}>Delete</button>
          {/if}
        {/if}
      </div>
      <div class="footer-actions">
        <button class="btn btn-sm btn-outline" on:click={onClose}>Cancel</button>
        <button
          class="btn btn-sm btn-accent"
          disabled={!canSave}
          on:click={handleSave}
        >
          {isNew ? 'Add Entry' : 'Save'}
        </button>
      </div>
    </div>

  </div>
</div>

<style>
  /* ── Backdrop / shell ── */
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.72);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: var(--z-modal);
    backdrop-filter: blur(3px);
  }

  .editor-modal {
    background: var(--bg-1);
    border: 1px solid var(--border-lit);
    border-radius: var(--radius-md);
    width: min(800px, 92vw);
    margin-top: 38px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    box-shadow:
      0 32px 100px rgba(0, 0, 0, 0.6),
      0 0 0 1px rgba(255, 255, 255, 0.03) inset;
  }

  /* ── Header ── */
  .modal-header {
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-shrink: 0;
    gap: 12px;
    min-height: 52px;
  }

  .modal-title {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .modal-title svg { width: 16px; height: 16px; color: var(--accent); flex-shrink: 0; }

  .modal-display-name {
    font-family: var(--font-head);
    font-size: 13px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-0);
  }

  .close-btn {
    width: 28px; height: 28px;
    display: flex; align-items: center; justify-content: center;
    background: transparent;
    border: 1px solid transparent;
    color: var(--text-2); cursor: pointer;
    border-radius: var(--radius-sm);
    transition: all 0.12s;
    flex-shrink: 0;
  }
  .close-btn:hover { color: var(--text-0); background: var(--bg-3); border-color: var(--border-mid); }
  .close-btn svg { width: 14px; height: 14px; }

  /* ── Body ── */
  .modal-body {
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-height: min(580px, 50vh);
    overflow-y: auto;
    
  }

  /* ── Field rows ── */
  .field-row {
    display: grid;
    grid-template-columns: 90px 1fr;
    align-items: center;
    gap: 10px;
  }

  /* For rows whose right cell can grow taller (prototype dropdown, textarea) */
  .field-row-top { align-items: start; }
  .field-row-top .field-label { padding-top: 7px; }

  .field-label {
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--text-2);
    justify-self: end;
    margin-bottom: 0;
  }

  .field-input {
    background: var(--bg-0);
    border: 1px solid var(--border-mid);
    color: var(--text-0);
    font-family: var(--font-mono);
    font-size: 12px;
    padding: 6px 10px;
    border-radius: var(--radius-sm);
    outline: none;
    width: 100%;
    transition: border-color 0.14s;
  }
  .field-input:focus { border-color: var(--accent-dim); }
  .field-input::placeholder { color: var(--text-3); font-family: var(--font-body); }
  .field-input.input-error { border-color: var(--red); }

  .field-select {
    background: var(--bg-0);
    border: 1px solid var(--border-mid);
    color: var(--text-1);
    font-family: var(--font-body);
    font-size: 12px;
    padding: 6px 10px;
    border-radius: var(--radius-sm);
    outline: none;
    width: 100%;
    cursor: pointer;
    transition: border-color 0.14s;
  }
  .field-select:focus { border-color: var(--accent-dim); }
  .field-select option { background: var(--bg-2); }

  .field-textarea {
    background: var(--bg-0);
    border: 1px solid var(--border-mid);
    color: var(--text-0);
    font-family: var(--font-mono);
    font-size: 11px;
    padding: 6px 10px;
    border-radius: var(--radius-sm);
    resize: vertical;
    outline: none;
    width: 100%;
    transition: border-color 0.14s;
    line-height: 1.5;
  }
  .field-textarea:focus { border-color: var(--accent-dim); }
  .field-textarea.input-error { border-color: var(--red); }

  /* Single-logical-line textareas — wrap but don't allow manual resize */
  .field-textarea-sm {
    resize: none;
    line-height: 1.5;
  }

  /* ── Value wrapper ── */
  .value-wrap {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .value-feedback { font-size: 10px; font-family: var(--font-body); }
  .value-error    { color: var(--text-error); }
  .value-hint     { color: var(--text-3); }
  .value-hint em  { color: var(--accent-dim); font-style: normal; }

  /* ── Prototype search ── */
  .proto-wrap {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .proto-input-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  /* .proto-input-row .field-input    { flex: 1; } */
  .proto-input-row .field-textarea { flex: 1; }

  .proto-hint {
    font-family: var(--font-head);
    font-size: 9px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
    white-space: nowrap;
    flex-shrink: 0;
  }

  .proto-dropdown {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    right: 0;
    background: var(--bg-2);
    border: 1px solid var(--border-lit);
    border-radius: var(--radius-sm);
    z-index: var(--z-dropdown);
    max-height: 240px;
    overflow-y: auto;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
  }

  .proto-option {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    border-bottom: 1px solid var(--border);
    padding: 7px 10px;
    cursor: pointer;
    transition: background 0.08s;
  }
  .proto-option:last-child { border-bottom: none; }
  .proto-option:hover { background: var(--bg-3); }

  .proto-opt-name {
    font-family: var(--font-head);
    font-size: 11px;
    font-weight: 600;
    color: var(--text-0);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .proto-opt-path {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .proto-opt-bp {
    font-family: var(--font-head);
    font-size: 9px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--accent-dim);
  }

  /* ── Footer ── */
  .modal-footer {
    padding: 12px 16px;
    border-top: 1px solid var(--border);
    background: var(--chrome-footer-bg);
    display: flex;
    align-items: center;
    gap: 10px;
    flex-shrink: 0;
    min-height: 52px;
  }

  .footer-left {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .footer-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-left: auto;
  }

  .delete-label {
    font-family: var(--font-head);
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-error);
  }
</style>