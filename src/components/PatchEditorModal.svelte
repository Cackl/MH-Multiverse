<script lang="ts">
  import { askConfirm } from '../lib/store'
  import type { PatchEntry } from '../lib/patchesMeta'
  import type { PrototypeMatch } from '../lib/prototypes'
  import { pickedValue } from '../lib/patchesMeta'
  import {
    RECIPES, PARENTS, ID_FIELDS, CHOICE_FIELDS, CHOICE_NOTES, FIXED_FIELDS, LOAD_INT,
    parsePairs, pairsText, emptyKeys, withParent, pathHint, problems, entryJson, parseEntryJson,
    type Pair, type Recipe,
  } from '../lib/patchEntryEdit'
  import PrototypePicker from './PrototypePicker.svelte'

  // ── Props ──────────────────────────────────────────────────────────────────

  /** Existing entry to edit. Null when creating a new entry. */
  export let entry: PatchEntry | null = null
  export let serverExe: string
  /** The file the entry belongs to, shown under the title. */
  export let fileName = ''
  export let onClose:  () => void
  export let onSave:   (entry: PatchEntry) => void
  export let onDelete: () => void

  // ── Constants ──────────────────────────────────────────────────────────────

  const VALUE_TYPES = [
    'Integer', 'Float', 'Boolean', 'String', 'Enum',
    'PrototypeId', 'PrototypeDataRef', 'PrototypeGuid', 'LocaleStringId',
    'PrototypeId[]', 'PrototypeDataRef[]', 'Prototype', 'Prototype[]', 'Vector3', 'Properties',
  ]

  // ValueTypes that require a JSON textarea instead of a plain text input.
  const COMPLEX:    ReadonlySet<string> = new Set(['Vector3', 'PrototypeId[]', 'PrototypeDataRef[]', 'Prototype', 'Prototype[]', 'Properties'])
  // ValueTypes kept as the file's own JSON text end to end (load_patch_file sends them as a
  // string): parsing in JS would round 64-bit ids and drop repeated Properties keys.
  const RAW_JSON:   ReadonlySet<string> = new Set(['Prototype', 'Prototype[]', 'Properties'])

  // ── State ──────────────────────────────────────────────────────────────────

  $: isNew = entry === null
  // A new entry starts on the Start from list.
  let started = entry !== null

  let draft: PatchEntry = entry
    ? { ...entry }
    : { Enabled: true, Prototype: '', Path: '', Description: '', ValueType: 'Integer', Value: '' }

  // The pick whose id or GUID is in Value, so a later pick or a ValueType change can swap it.
  let filledBy: PrototypeMatch | null = null
  // Value still holds what that pick put there (what it would fill into an empty value).
  $: autoFilled = !!filledBy && valueStr.trim() !== '' && valueStr.trim() === pickedValue('', draft.ValueType, filledBy, null)

  let protoQuery = draft.Prototype
  // Value is edited as a string regardless of underlying type.
  let valueStr = serializeValue(draft.Value, draft.ValueType)

  let mode: 'form' | 'json' = 'form'
  let jsonText = ''
  let jsonError = ''

  // Names of ids picked in structured fields, by id.
  let pickedNames: Record<string, string> = {}

  let deleteConfirming = false

  const initialState = JSON.stringify([draft, valueStr])
  $: dirty = started && JSON.stringify([draft, valueStr]) !== initialState

  // ── Derived ────────────────────────────────────────────────────────────────

  $: current = { ...draft, Prototype: protoQuery.trim(), Value: parseValue(valueStr, draft.ValueType) } as PatchEntry
  $: needsComplex = COMPLEX.has(draft.ValueType)
  // Prototype and Properties values as key/value rows; null when the text isn't a flat object.
  $: pairs = (draft.ValueType === 'Prototype' || draft.ValueType === 'Properties') ? parsePairs(valueStr) : null
  $: parent = draft.ValueType === 'Prototype' ? pairs?.find(p => p.key === 'ParentDataRef')?.text ?? '' : ''
  $: knownParent = PARENTS.some(p => p.id === parent)
  $: parseError = validateValueStr(valueStr, draft.ValueType, pairs)
  $: found = problems(current)
  $: hint = pathHint(current)
  $: canSave = started && !parseError && !(mode === 'json' && jsonError) && protoQuery.trim().length > 0
  $: title = draft.Description.trim() || (isNew ? 'New entry' : 'Patch entry')

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

  function validateValueStr(s: string, vt: string, rows: Pair[] | null): string {
    if (rows) {
      const empty = emptyKeys(rows)
      return empty.length ? `Fill in ${empty.join(', ')}.` : ''
    }
    if (!s.trim()) return ''
    if (vt === 'Integer' && isNaN(parseInt(s, 10)))  return 'Expected integer.'
    if (vt === 'Float'   && isNaN(parseFloat(s)))    return 'Expected number.'
    if (COMPLEX.has(vt)) {
      try { JSON.parse(s) } catch { return 'Invalid JSON.' }
    }
    return ''
  }

  // Structured and plain values don't convert, so a change between them starts the value over.
  const kind = (vt: string) => vt === 'Prototype' ? 'struct' : vt === 'Properties' ? 'props' : 'plain'
  let lastType = draft.ValueType
  function onValueTypeChange() {
    if (kind(draft.ValueType) !== kind(lastType)) {
      valueStr = draft.ValueType === 'Prototype' ? pairsText(withParent([], LOAD_INT))
        : draft.ValueType === 'Properties' ? '{}' : ''
      filledBy = null
    } else if (filledBy) {
      // Swap in the matching id or GUID of the pick that filled Value.
      valueStr = pickedValue(valueStr, draft.ValueType, filledBy, filledBy)
    }
    lastType = draft.ValueType
  }

  function setPairs(next: Pair[]) {
    valueStr = pairsText(next)
  }
  function setPair(i: number, change: Partial<Pair>) {
    if (pairs) setPairs(pairs.map((p, j) => j === i ? { ...p, ...change } : p))
  }

  // ── Start from / fixes / JSON ──────────────────────────────────────────────

  function start(r: Recipe) {
    draft = { ...draft, Path: r.entry.Path, ValueType: r.entry.ValueType }
    valueStr = serializeValue(r.entry.Value, r.entry.ValueType)
    lastType = draft.ValueType
    started = true
  }

  function load(e: PatchEntry) {
    draft = { ...e }
    protoQuery = e.Prototype
    valueStr = serializeValue(e.Value, e.ValueType)
    lastType = e.ValueType
    filledBy = null
  }

  function setMode(next: 'form' | 'json') {
    if (next === mode) return
    if (next === 'json') {
      jsonText = entryJson(current)
      jsonError = ''
    } else if (jsonError) {
      return // stay until the JSON reads
    }
    mode = next
  }

  function onJsonInput() {
    const r = parseEntryJson(jsonText)
    if ('error' in r) { jsonError = r.error; return }
    jsonError = ''
    load(r.entry)
  }

  // ── Prototype search ───────────────────────────────────────────────────────

  function onProtoInput(value: string) {
    draft.Prototype = value
  }

  function selectProto(match: PrototypeMatch) {
    draft.Prototype = match.path
    const next = pickedValue(valueStr, draft.ValueType, match, filledBy)
    if (next !== valueStr) filledBy = match
    valueStr = next
  }

  // ── Save / delete ──────────────────────────────────────────────────────────

  function handleSave() {
    if (!canSave) return
    onSave(current)
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
  <div class="modal" role="dialog" aria-modal="true" aria-labelledby="pe-title" tabindex="-1">

    <!-- ── Header ── -->
    <div class="modal-header">
      <div class="modal-title">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M12 2 2 7l10 5 10-5-10-5z"/>
          <path d="m2 17 10 5 10-5"/>
          <path d="m2 12 10 5 10-5"/>
        </svg>
        <span class="modal-display-name" id="pe-title">{title}</span>
        {#if fileName}<span class="pe-sub" title={fileName}>{fileName}</span>{/if}
      </div>
      <div class="pe-head-acts">
        {#if started}
          <div class="seg" role="group" aria-label="Edit as">
            <button class:on={mode === 'form'} aria-pressed={mode === 'form'} on:click={() => setMode('form')}>Form</button>
            <button class:on={mode === 'json'} aria-pressed={mode === 'json'} on:click={() => setMode('json')}>JSON</button>
          </div>
        {/if}
        <button class="close-btn" aria-label="Close" on:click={tryClose}>
          <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5">
            <line x1="3.5" y1="3.5" x2="10.5" y2="10.5"/>
            <line x1="10.5" y1="3.5" x2="3.5" y2="10.5"/>
          </svg>
        </button>
      </div>
    </div>

    <!-- ── Body ── -->
    <div class="modal-body">
      {#if !started}
        <h4 class="fs-h"><span>Start from</span></h4>
        <div class="recipes">
          {#each RECIPES as r}
            <button class="recipe" on:click={() => start(r)}>
              <span>{r.name}</span>
              <small>{r.hint}</small>
            </button>
          {/each}
        </div>
      {:else}
        {#if found.length}
          <div class="probs">
            {#each found as p}
              <div class="prob" class:error={p.level === 'error'}>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  {#if p.level === 'error'}
                    <circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/>
                  {:else}
                    <path d="M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/>
                  {/if}
                </svg>
                <span>{p.message}</span>
                {#if p.fix && p.apply}
                  <button class="btn btn-sm btn-outline" on:click={() => p.apply && load(p.apply(current))}>{p.fix}</button>
                {/if}
              </div>
            {/each}
          </div>
        {/if}

        {#if mode === 'json'}
          <textarea
            class="field-textarea json"
            class:input-error={!!jsonError}
            aria-label="Entry JSON"
            spellcheck="false"
            bind:value={jsonText}
            on:input={onJsonInput}
          ></textarea>
          {#if jsonError}<span class="value-feedback value-error">{jsonError}</span>{/if}
        {:else}
          <h4 class="fs-h"><span>Entry</span></h4>
          <div class="fs-b">
            <div class="fld">
              <span class="fl">Enabled</span>
              <span class="tgl">
                <button
                  aria-label="Enabled"
                  class="toggle-switch"
                  class:on={draft.Enabled}
                  role="switch"
                  aria-checked={draft.Enabled}
                  on:click={() => draft.Enabled = !draft.Enabled}
                ></button>
                <span class="tgl-l">{draft.Enabled ? 'On' : 'Off'}</span>
              </span>
            </div>
            <div class="fld w">
              <label class="fl" for="pe-desc">Description</label>
              <input id="pe-desc" class="field-input" type="text" placeholder="What this entry does" bind:value={draft.Description} spellcheck="false">
            </div>
            <div class="fld w">
              <label class="fl" for="pe-proto">Prototype</label>
              <PrototypePicker
                id="pe-proto"
                bind:value={protoQuery}
                {serverExe}
                placeholder="Search or paste a prototype path"
                onInput={onProtoInput}
                onSelect={selectProto}
              />
            </div>
            <div class="fld w">
              <label class="fl" for="pe-path">Path</label>
              <input
                id="pe-path"
                class="field-input code"
                type="text"
                placeholder="e.g. SummonMaxSimultaneous"
                bind:value={draft.Path}
                spellcheck="false"
              >
              {#if hint}<span class="fh">{hint}</span>{/if}
            </div>
          </div>

          <h4 class="fs-h"><span>Value</span></h4>
          <div class="fs-b">
            <div class="fld">
              <label class="fl" for="pe-vtype">Value type</label>
              <select id="pe-vtype" class="field-select" bind:value={draft.ValueType} on:change={onValueTypeChange}>
                {#each VALUE_TYPES as vt}<option value={vt}>{vt}</option>{/each}
              </select>
            </div>

            {#if draft.ValueType === 'Prototype' && pairs}
              <div class="fld">
                <label class="fl" for="pe-parent">Based on</label>
                <select id="pe-parent" class="field-select" value={parent} on:change={e => pairs && setPairs(withParent(pairs, e.currentTarget.value))}>
                  {#each PARENTS as p}<option value={p.id}>{p.label}</option>{/each}
                  {#if !knownParent}<option value={parent}>Other ({parent || 'none'})</option>{/if}
                </select>
              </div>
              <div class="fld w">
                <span class="fl">Fields</span>
                {#each pairs as p, i}
                  {#if p.key !== 'ParentDataRef'}
                    <div class="kv">
                      <span class="kv-key code">{p.key}</span>
                      {#if ID_FIELDS.includes(p.key)}
                        <div class="kv-pick">
                          <PrototypePicker
                            value={pickedNames[p.text] ?? p.text}
                            {serverExe}
                            compact
                            placeholder="Search the game data"
                            onInput={v => /^\d*$/.test(v.trim()) && setPair(i, { text: v.trim() })}
                            onSelect={m => { pickedNames = { ...pickedNames, [m.prototype_id]: m.display_name || m.leaf }; setPair(i, { text: m.prototype_id }) }}
                          />
                          {#if p.text && pickedNames[p.text]}<span class="fh">ID {p.text}</span>{/if}
                        </div>
                      {:else if CHOICE_FIELDS[p.key]}
                        <div class="kv-pick">
                          <select class="field-select" aria-label={p.key} value={p.text} on:change={e => setPair(i, { text: e.currentTarget.value })}>
                            {#each CHOICE_FIELDS[p.key] as c}<option value={c}>{c}</option>{/each}
                            {#if !CHOICE_FIELDS[p.key].includes(p.text)}<option value={p.text}>Other ({p.text || 'none'})</option>{/if}
                          </select>
                          {#if CHOICE_NOTES[p.text]}<span class="fh">{CHOICE_NOTES[p.text]}</span>{/if}
                        </div>
                      {:else}
                        <input
                          class="field-input num"
                          class:input-error={!p.str && !p.text.trim()}
                          aria-label={p.key}
                          value={p.text}
                          readonly={FIXED_FIELDS.includes(p.key)}
                          on:input={e => setPair(i, { text: e.currentTarget.value })}
                        >
                      {/if}
                    </div>
                  {/if}
                {/each}
                {#if !knownParent}<span class="fh">Writes ParentDataRef {parent}</span>{/if}
              </div>
            {:else if draft.ValueType === 'Properties' && pairs}
              <div class="fld w">
                <span class="fl">Properties</span>
                {#each pairs as p, i}
                  <div class="kv props">
                    <input class="field-input code" aria-label="Property" value={p.key} spellcheck="false" on:input={e => setPair(i, { key: e.currentTarget.value })}>
                    <input
                      class="field-input num"
                      class:input-error={!p.str && !p.text.trim()}
                      aria-label="{p.key} value"
                      value={p.text}
                      on:input={e => setPair(i, { text: e.currentTarget.value })}
                    >
                    <button class="del-btn" aria-label="Remove {p.key}" on:click={() => pairs && setPairs(pairs.filter((_, j) => j !== i))}>
                      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                        <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
                      </svg>
                    </button>
                  </div>
                {/each}
                <button class="btn btn-sm btn-outline add-prop" on:click={() => pairs && setPairs([...pairs, { key: '', text: '', str: false }])}>Add property</button>
              </div>
            {:else}
              <div class="fld w">
                <label class="fl" for="pe-value">Value</label>
                {#if draft.ValueType === 'Boolean'}
                  <select id="pe-value" class="field-select" bind:value={valueStr}>
                    <option value="true">true</option>
                    <option value="false">false</option>
                  </select>
                {:else if needsComplex}
                  <textarea id="pe-value" class="field-textarea" class:input-error={!!parseError} rows="5" bind:value={valueStr} spellcheck="false"></textarea>
                {:else}
                  <input id="pe-value" type="text" class="field-input num" class:input-error={!!parseError} bind:value={valueStr} spellcheck="false">
                {/if}
                {#if filledBy && autoFilled && !parseError}
                  <span class="fh">Filled from {filledBy.display_name || filledBy.leaf}, {draft.ValueType === 'PrototypeGuid' ? 'GUID' : 'runtime ID'}</span>
                {/if}
              </div>
            {/if}
          </div>
          {#if parseError}<span class="value-feedback value-error">{parseError}</span>{/if}
        {/if}
      {/if}
    </div>

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
        {:else if started}
          <button class="btn btn-sm btn-outline" on:click={() => started = false}>Start over</button>
        {/if}
      </div>
      <div class="footer-actions">
        <button class="btn btn-sm btn-outline" on:click={tryClose}>Cancel</button>
        <button class="btn btn-sm btn-accent" disabled={!canSave} on:click={handleSave}>
          {isNew ? 'Add entry' : 'Save'}
        </button>
      </div>
    </div>

  </div>
</div>

<style>
  .modal { width: min(640px, 92vw); max-height: 85vh; }
  .modal-title { flex: 1; min-width: 0; }
  .modal-display-name { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; min-width: 0; }
  .pe-sub {
    font-size: 12px;
    color: var(--text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }
  .pe-head-acts { display: flex; align-items: center; gap: 8px; flex-shrink: 0; margin-left: 12px; }

  /* Form / JSON */
  .seg { display: inline-flex; gap: 2px; }
  .seg button {
    height: 24px;
    padding: 0 9px;
    border: 1px solid transparent;
    border-radius: 2px;
    background: transparent;
    color: var(--text-3);
    font-family: var(--font-head);
    font-size: var(--fs-min);
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    cursor: pointer;
  }
  .seg button:hover { color: var(--text-1); }
  .seg button.on { color: var(--accent-bright); border-color: var(--accent-dim); background: var(--accent-glow); }

  .modal-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 16px 20px 18px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  /* Section heading: accent caps with a hairline */
  .fs-h {
    margin: 4px 0 0;
    display: flex;
    align-items: center;
    gap: 10px;
    font-family: var(--font-head);
    font-size: var(--fs-min);
    font-weight: 600;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--accent);
  }
  .fs-h::after { content: ''; flex: 1; height: 1px; background: var(--border); }
  .fs-b { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px 14px; min-width: 0; }
  .fld { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  .fld.w { grid-column: 1 / -1; }
  .fl {
    font-family: var(--font-head);
    font-size: var(--fs-min);
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-2);
  }
  .fh { font-size: 12px; color: var(--text-3); line-height: 1.45; }
  .tgl { display: inline-flex; align-items: center; gap: 9px; height: 30px; }
  .tgl-l { font-size: 12px; color: var(--text-2); }

  .field-input {
    height: 30px;
    background: var(--bg-0);
    border: 1px solid var(--border-mid);
    color: var(--text-0);
    font-family: var(--font-body);
    font-size: 12px;
    padding: 0 10px;
    border-radius: var(--radius-sm);
    outline: none;
    width: 100%;
    min-width: 0;
  }
  .field-input:focus { border-color: var(--accent-dim); }
  .field-input.code { font-family: var(--font-mono); }
  .field-input.num { font-variant-numeric: tabular-nums; }
  .field-input[readonly] { background: var(--chrome-sunken-bg); color: var(--text-2); border-style: dashed; }
  .field-input.input-error, .field-textarea.input-error { border-color: var(--red); }
  .field-select { width: 100%; }

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
    line-height: 1.5;
  }
  .field-textarea:focus { border-color: var(--accent-dim); }
  .field-textarea.json { min-height: 300px; font-size: 12px; line-height: 1.55; white-space: pre; }

  /* Structured value rows */
  .kv { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1.4fr); gap: 8px; align-items: center; }
  .kv.props { grid-template-columns: minmax(0, 1.4fr) minmax(0, 1fr) 22px; }
  .kv-key { font-family: var(--font-mono); font-size: 12px; color: var(--text-1); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .kv-pick { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
  .add-prop { align-self: flex-start; }

  /* Start from */
  .recipes { display: flex; flex-direction: column; border: 1px solid var(--border); border-radius: var(--radius-sm); overflow: hidden; }
  .recipe {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
    padding: 9px 12px;
    border: 0;
    border-bottom: 1px solid var(--border);
    background: transparent;
    color: var(--text-0);
    font-family: var(--font-body);
    font-size: 13px;
    text-align: left;
    cursor: pointer;
  }
  .recipe:last-child { border-bottom: 0; }
  .recipe:hover { background: var(--bg-2); }
  .recipe small {
    max-width: 45%;
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Problems */
  .probs { display: flex; flex-direction: column; gap: 8px; }
  .prob {
    display: grid;
    grid-template-columns: 15px minmax(0, 1fr);
    gap: 6px 8px;
    padding: 9px 11px;
    border: 1px solid color-mix(in srgb, var(--amber) 40%, transparent);
    border-radius: var(--radius-sm);
    background: var(--amber-dim);
    font-size: 12px;
    line-height: 1.5;
    color: var(--text-1);
  }
  .prob svg { width: 15px; height: 15px; margin-top: 1px; color: var(--amber-bright); }
  .prob.error { border-color: color-mix(in srgb, var(--red) 40%, transparent); background: var(--red-dim); }
  .prob.error svg { color: var(--text-error); }
  .prob .btn { grid-column: 2; justify-self: start; }

  .value-feedback { font-size: 12px; }
  .value-error { color: var(--text-error); }

  /* Footer */
  .footer-left { display: flex; align-items: center; gap: 8px; }
  .footer-actions { display: flex; align-items: center; gap: 8px; margin-left: auto; }
  .delete-label {
    font-family: var(--font-head);
    font-size: var(--fs-min);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-error);
  }
</style>
