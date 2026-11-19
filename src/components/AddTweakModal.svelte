<script lang="ts" context="module">
  import type { PowerCooldown, SummonCap } from '../lib/gameTweaks'
  export type NewTweak =
    | { kind: 'cooldown'; power: PowerCooldown; name: string }
    | { kind: 'summon'; cap: SummonCap; name: string }
</script>

<script lang="ts">
  import PrototypePicker from './PrototypePicker.svelte'
  import Dropdown from './Dropdown.svelte'
  import type { PrototypeMatch } from '../lib/prototypes'
  import {
    PLAYER_POWERS, COOLDOWN_MIN, COOLDOWN_MAX, COOLDOWN_STEP, COOLDOWN_DEFAULT, clampCooldown,
    SUMMON_MIN, SUMMON_MAX, SUMMON_DEFAULT, clampSummon,
  } from '../lib/gameTweaks'

  export let serverExe: string
  export let onAdd: (tweak: NewTweak) => void
  export let onClose: () => void

  const KINDS = [
    { id: 'cooldown', label: 'Power cooldown', classFilter: 'PowerPrototype', search: 'Search player powers' },
    { id: 'summon', label: 'Summon cap', classFilter: 'SummonPowerPrototype', search: 'Search summon powers' },
  ] as const

  let kind: NewTweak['kind'] = 'cooldown'
  let query = ''
  let picked: PrototypeMatch | null = null
  let pct = COOLDOWN_DEFAULT
  let max = SUMMON_DEFAULT

  $: spec = KINDS.find(k => k.id === kind)!

  const label = (r: PrototypeMatch) => (r.display_name && r.display_name !== r.path ? r.display_name : r.leaf || r.path)

  function changeKind() {
    query = ''
    picked = null
  }

  function add() {
    if (!picked) return
    const name = label(picked)
    onAdd(kind === 'cooldown'
      ? { kind, power: { id: picked.prototype_id, pct: clampCooldown(pct) }, name }
      : { kind, cap: { path: picked.path, max: clampSummon(max) }, name })
  }
</script>

<svelte:window on:keydown={(e) => e.key === 'Escape' && onClose()} />

<div class="modal-backdrop" role="presentation" on:click={onClose}>
  <div class="modal" role="dialog" aria-modal="true" aria-labelledby="tweak-title" tabindex="-1" on:click|stopPropagation on:keydown|stopPropagation>
    <div class="modal-header">
      <div class="modal-title">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="12" y1="5" x2="12" y2="19"/>
          <line x1="5" y1="12" x2="19" y2="12"/>
        </svg>
        <span class="modal-display-name" id="tweak-title">New entry</span>
      </div>
      <button class="close-btn" aria-label="Close" on:click={onClose}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="18" y1="6" x2="6" y2="18"/>
          <line x1="6" y1="6" x2="18" y2="18"/>
        </svg>
      </button>
    </div>

    <div class="modal-body">
      <div class="field-group field-group-center">
        <label class="field-label" for="tweak-kind">Type</label>
        <Dropdown id="tweak-kind" label="Type" value={kind} options={KINDS.map(k => ({ value: k.id, label: k.label }))}
          onChange={v => { kind = v as NewTweak['kind']; changeKind() }} --dd-width="200px" />
      </div>

      <div class="field-group">
        <label class="field-label" for="tweak-power">Power</label>
        <div class="field-control">
          {#key kind}
            <PrototypePicker
              id="tweak-power"
              bind:value={query}
              {serverExe}
              blueprintHint={PLAYER_POWERS}
              classFilter={spec.classFilter}
              placeholder={spec.search}
              onInput={() => picked = null}
              onSelect={(m) => picked = m}
            />
          {/key}
          {#if picked}<span class="field-hint">{label(picked)}</span>{/if}
        </div>
      </div>

      {#if kind === 'cooldown'}
        <div class="field-group field-group-center">
          <label class="field-label" for="tweak-pct">Reduce by</label>
          <div class="field-control field-inline">
            <input id="tweak-pct" class="field-input num" type="number" min={COOLDOWN_MIN} max={COOLDOWN_MAX} step={COOLDOWN_STEP} bind:value={pct}>
            <span class="field-hint">%</span>
          </div>
        </div>
      {:else}
        <div class="field-group field-group-center">
          <label class="field-label" for="tweak-max">Max at once</label>
          <input id="tweak-max" class="field-input num" type="number" min={SUMMON_MIN} max={SUMMON_MAX} step="1" bind:value={max}>
        </div>
      {/if}
    </div>

    <div class="modal-footer">
      <button class="btn btn-sm btn-outline" on:click={onClose}>Cancel</button>
      <button class="btn btn-sm btn-accent" disabled={!picked} on:click={add}>Add entry</button>
    </div>
  </div>
</div>

<style>
  .modal { width: min(560px, 92vw); }
  .modal-body { padding: 4px 0; }
  .field-group {
    display: grid;
    grid-template-columns: 110px 1fr;
    gap: 10px;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
  }
  .field-group:last-child { border-bottom: none; }
  .field-label {
    font-family: var(--font-head);
    font-size: var(--fs-min);
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-2);
    padding-top: 7px;
  }
  .field-control { display: flex; flex-direction: column; gap: 6px; min-width: 0; }
  .field-inline { flex-direction: row; align-items: center; gap: 8px; }
  .field-group-center { align-items: center; }
  .field-group-center .field-label { padding-top: 0; }
  .field-hint { font-size: 12px; color: var(--text-2); }
  .field-input {
    background: var(--bg-0);
    border: 1px solid var(--border-mid);
    color: var(--text-0);
    font-family: var(--font-body);
    font-size: 12px;
    padding: 6px 8px;
    border-radius: var(--radius-sm);
    outline: none;
  }
  .field-input:focus { border-color: var(--accent-dim); }
  .field-input.num { width: 110px; font-variant-numeric: tabular-nums; }
</style>
