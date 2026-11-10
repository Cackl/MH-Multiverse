<script lang="ts">
  import { askConfirm } from '../lib/store'
  import type { PatchEntry } from '../lib/patchesMeta'
  import type { PrototypeMatch } from '../lib/prototypes'
  import PrototypePicker from './PrototypePicker.svelte'

  // ── Props ──────────────────────────────────────────────────────────────────

  /** Existing entry to edit. Null when creating a new entry. */
  export let entry: PatchEntry | null = null
  export let serverExe: string
  export let onClose:  () => void
  export let onSave:   (entry: PatchEntry) => void
  export let onDelete: () => void

  // ── Constants ──────────────────────────────────────────────────────────────

  const VALUE_TYPES = [
    'PrototypeId', 'PrototypeDataRef', 'PrototypeGuid',
    'Integer', 'Float', 'Boolean', 'Enum', 'LocaleStringId',
    'Vector3', 'PrototypeId[]', 'PrototypeDataRef[]', 'Prototype', 'Prototype[]', 'Properties',
  ]

  // ValueTypes where selecting a prototype auto-fills Value from prototype_id.
  const ID_TYPES:   ReadonlySet<string> = new Set(['PrototypeId', 'PrototypeDataRef', 'PrototypeId[]', 'PrototypeDataRef[]'])
  // ValueTypes where selecting a prototype auto-fills Value from prototype_guid.
  const GUID_TYPES: ReadonlySet<string> = new Set(['PrototypeGuid'])
  // ValueTypes that require a JSON textarea instead of a plain text input.
  const COMPLEX:    ReadonlySet<string> = new Set(['Vector3', 'PrototypeId[]', 'PrototypeDataRef[]', 'Prototype', 'Prototype[]', 'Properties'])
  // ValueTypes kept as the file's own JSON text end to end (load_patch_file sends them as a
  // string): parsing in JS would round 64-bit ids and drop repeated Properties keys.
  const RAW_JSON:   ReadonlySet<string> = new Set(['Prototype', 'Prototype[]', 'Properties'])

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

  // Value is edited as a string regardless of underlying type.
  let valueStr = serializeValue(draft.Value, draft.ValueType)

  // Delete confirm
  let deleteConfirming = false

  const initialState = JSON.stringify([draft, valueStr])
  $: dirty = JSON.stringify([draft, valueStr]) !== initialState

  // ── Derived ────────────────────────────────────────────────────────────────

  $: needsComplex  = COMPLEX.has(draft.ValueType)
  $: parseError    = validateValueStr(valueStr, draft.ValueType)
  $: canSave       = !parseError && protoQuery.trim().length > 0

  // ── Value helpers ──────────────────────────────────────────────────────────

  function serializeValue(v: unknown, vt: string): string {
    if (v === null || v === undefined) return ''
    if (RAW_JSON.has(vt) && typeof v === 'string') return v
    if (COMPLEX.has(vt) || typeof v === 'object') return JSON.stringify(v, null, 2)
    return String(v)
  }

  function parseValue(s: string, vt: string): unknown {
    if (vt === 'Boolean')  return s === 'true'
    if (vt === 'Integer')  return parseInt(s, 10)
    if (vt === 'Float')    return parseFloat(s)
    if (RAW_JSON.has(vt))  return s
    if (COMPLEX.has(vt))   { try { return JSON.parse(s) } catch { return s } }
    // ID types stay strings: they are 64-bit and Number() would round anything above
    // 2^53. save_patch_file converts them back to bare integers on write.
    return s
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

  function onProtoInput(value: string) {
    draft.Prototype = value
    lastMatch       = null
  }

  function selectProto(match: PrototypeMatch) {
    draft.Prototype = match.path
    lastMatch       = match

    if (ID_TYPES.has(draft.ValueType))        valueStr = match.prototype_id
    else if (GUID_TYPES.has(draft.ValueType)) valueStr = match.prototype_guid
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


  // ── Close guard ────────────────────────────────────────────────────────────

  async function tryClose() {
    if (!dirty || await askConfirm({
      title: 'Discard changes?',
      message: 'This entry has unsaved changes. Discard them?',
      okLabel: 'Discard',
      cancelLabel: 'Keep editing',
    })) onClose()
  }

  // ── Keyboard / click-outside ───────────────────────────────────────────────

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      if (deleteConfirming) { deleteConfirming = false; return }
      tryClose()
    }
  }

</script>

<svelte:window on:keydown={onKeydown} />

<div
  class="modal-backdrop"
  role="presentation"
  on:click={e => { if (e.target === e.currentTarget) tryClose() }}
>
  <div class="modal" role="dialog" aria-modal="true" tabindex="-1">

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
      <button class="close-btn" aria-label="Close" on:click={tryClose}>
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
          role="switch"
          aria-checked={draft.Enabled}
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
        <PrototypePicker
          id="pe-proto"
          bind:value={protoQuery}
          {serverExe}
          multiline
          placeholder="Search or paste prototype path..."
          onInput={onProtoInput}
          onSelect={selectProto}
        />
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
        <button class="btn btn-sm btn-outline" on:click={tryClose}>Cancel</button>
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
  .modal {
    width: min(800px, 92vw);
  }

  /* ── Header ── */
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
    font-size: var(--fs-min);
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

  .value-feedback { font-size: var(--fs-min); font-family: var(--font-body); }
  .value-error    { color: var(--text-error); }
  .value-hint     { color: var(--text-3); }
  .value-hint em  { color: var(--accent); font-style: normal; }

  /* ── Footer ── */
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
    font-size: var(--fs-min);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-error);
  }
</style>