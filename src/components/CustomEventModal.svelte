<script lang="ts">
  import { askConfirm } from '../lib/store'
  import { tip } from '../lib/tooltip'
  import { RATE_ROWS } from '../lib/tuningMeta'
  import {
    EVENT_SLIDERS, WEEK, eventNameError, yearlyMaxDays, dateRange, type CustomEvent, type EventWhen,
  } from '../lib/eventSchedule'
  import PrototypePicker from './PrototypePicker.svelte'

  /** The event to edit, or null for a new one. */
  export let event: CustomEvent | null
  /** Settings in its tuning file that the sliders don't cover. */
  export let extraCount = 0
  export let serverExe: string
  export let onSave: (ev: CustomEvent) => void
  export let onDelete: (() => void) | null = null
  export let onClose: () => void

  const MONTHS = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December']
  const sliders = EVENT_SLIDERS.map(s => RATE_ROWS.find(r => r.setting === s)!)

  const start: CustomEvent = event ?? {
    name: '', when: { kind: 'weekly', days: ['Saturday', 'Sunday'] }, values: {}, gift: '', announce: true,
  }
  let name = start.name
  let kind: EventWhen['kind'] = start.when.kind
  let days: string[] = start.when.kind === 'weekly' ? [...start.when.days] : ['Saturday', 'Sunday']
  let month = start.when.kind === 'yearly' ? start.when.month : 12
  let day = start.when.kind === 'yearly' ? start.when.day : 1
  let length = start.when.kind === 'yearly' ? start.when.days : 7
  let values: Record<string, number> = Object.fromEntries(EVENT_SLIDERS.map(s => [s, start.values[s] ?? 1]))
  let gift = start.gift
  let announce = start.announce
  let tried = false

  // 29 Feb is never offered: the server crashes on it in non-leap years.
  $: monthDays = new Date(Date.UTC(2023, month, 0)).getUTCDate()
  $: if (day > monthDays) day = monthDays
  $: maxLength = yearlyMaxDays(month, day)
  $: if (length > maxLength) length = maxLength

  $: when = (kind === 'daily' ? { kind } : kind === 'weekly' ? { kind, days } : { kind, month, day, days: length }) as EventWhen
  $: current = { name: name.trim(), when, values, gift: gift.trim(), announce } satisfies CustomEvent
  $: nameError = eventNameError(name)
  $: daysError = kind === 'weekly' && days.length === 0 ? 'Pick at least one day.' : ''
  $: dirty = JSON.stringify(current) !== JSON.stringify({ ...start, name: start.name.trim(), values: Object.fromEntries(EVENT_SLIDERS.map(s => [s, start.values[s] ?? 1])) })

  function toggleDay(d: string) {
    days = days.includes(d) ? days.filter(x => x !== d) : [...days, d]
  }

  function save() {
    tried = true
    if (nameError || daysError) return
    onSave({ ...current, values: Object.fromEntries(EVENT_SLIDERS.filter(s => values[s] !== 1).map(s => [s, values[s]])) })
  }

  async function tryClose() {
    if (!dirty || await askConfirm({
      title: 'Discard changes?', message: 'This event has unsaved changes. Discard them?',
      okLabel: 'Discard', cancelLabel: 'Keep editing',
    })) onClose()
  }

  async function remove() {
    if (onDelete && await askConfirm({
      title: `Delete ${start.name}?`,
      message: 'Its schedule entries go, and its tuning file is kept as a .bak copy. Nothing changes until you save.',
      okLabel: 'Delete', cancelLabel: 'Keep it',
    })) onDelete()
  }

  const clamp = (row: typeof sliders[number], v: number) => Math.min(row.limit, Math.max(0, v))
  const pct = (row: typeof sliders[number], v: number) => Math.max(0, Math.min(100, (v - row.min) / (row.max - row.min) * 100))
</script>

<svelte:window on:keydown={(e) => e.key === 'Escape' && tryClose()} />

<div class="modal-backdrop" role="presentation" on:click={tryClose}>
  <div class="modal" role="dialog" aria-modal="true" aria-labelledby="custom-event-title" tabindex="-1" on:click|stopPropagation on:keydown|stopPropagation>
    <div class="modal-header">
      <div class="modal-title">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <rect x="3" y="4" width="18" height="18" rx="2"/>
          <line x1="16" y1="2" x2="16" y2="6"/>
          <line x1="8" y1="2" x2="8" y2="6"/>
          <line x1="3" y1="10" x2="21" y2="10"/>
        </svg>
        <span class="modal-display-name" id="custom-event-title">{event ? 'Edit event' : 'Make your own event'}</span>
      </div>
      <button class="close-btn" aria-label="Close" on:click={tryClose}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="18" y1="6" x2="6" y2="18"/>
          <line x1="6" y1="6" x2="18" y2="18"/>
        </svg>
      </button>
    </div>

    <div class="modal-body">
      <div class="field-group">
        <label class="field-label" for="ce-name">Name</label>
        <div class="field-control">
          <input id="ce-name" class="field-input" class:input-error={tried && !!nameError} type="text" maxlength="40" bind:value={name} placeholder="Double XP Weekend" />
          {#if tried && nameError}<span class="field-error">{nameError}</span>{/if}
        </div>
      </div>

      <div class="field-group">
        <label class="field-label" for="ce-when">When</label>
        <div class="field-control">
          <select id="ce-when" class="field-select" bind:value={kind}>
            <option value="daily">Every day</option>
            <option value="weekly">Weekly</option>
            <option value="yearly">Every year</option>
          </select>
          {#if kind === 'weekly'}
            <div class="day-row" role="group" aria-label="Days">
              {#each WEEK as d}
                <button class="day-btn" class:on={days.includes(d)} aria-pressed={days.includes(d)} on:click={() => toggleDay(d)}>{d.slice(0, 3)}</button>
              {/each}
              <button class="btn btn-sm btn-outline" on:click={() => days = ['Saturday', 'Sunday']}>Weekend</button>
            </div>
            {#if tried && daysError}<span class="field-error">{daysError}</span>{/if}
          {:else if kind === 'yearly'}
            <div class="date-row">
              <select class="field-select" aria-label="Month" bind:value={month}>
                {#each MONTHS as m, i}<option value={i + 1}>{m}</option>{/each}
              </select>
              <select class="field-select date-day" aria-label="Day" bind:value={day}>
                {#each Array.from({ length: monthDays }, (_, i) => i + 1) as d}<option value={d}>{d}</option>{/each}
              </select>
              <span class="field-text">for</span>
              <input class="field-input date-len" type="number" aria-label="Number of days" min="1" max={maxLength} bind:value={length}
                on:change={() => length = Math.min(maxLength, Math.max(1, Math.round(length || 1)))} />
              <span class="field-text">{length === 1 ? 'day' : 'days'}</span>
            </div>
            <span class="field-hint">Repeats every year, {dateRange(month, day, length)}. The server ends yearly events on 31 Dec.</span>
          {/if}
        </div>
      </div>

      <div class="field-group">
        <span class="field-label">While it runs</span>
        <div class="field-control">
          <span class="field-hint">1× leaves the Server Rates value alone. Anything else replaces it while the event runs.</span>
          {#each sliders as row}
            <div class="slider-row">
              <span class="slider-name">
                {row.label}
                {#if row.tip}<button class="info-btn" aria-label="About {row.label}" use:tip={row.tip}>?</button>{/if}
              </span>
              <span class="range-track" style="--mark:{pct(row, 1)}">
                <input type="range" aria-label={row.label} min={row.min} max={row.max} step={row.step} bind:value={values[row.setting]}
                  style="--fill:{pct(row, values[row.setting])}%" />
              </span>
              <span class="slider-box">
                <input type="number" aria-label="{row.label} value" min="0" max={row.limit} step="any" value={values[row.setting]}
                  on:change={(e) => { values[row.setting] = clamp(row, e.currentTarget.valueAsNumber || 0); e.currentTarget.value = String(values[row.setting]) }} />
                <span class="slider-unit">×</span>
              </span>
            </div>
          {/each}
          {#if extraCount}<span class="field-hint">{extraCount} more {extraCount === 1 ? 'setting is' : 'settings are'} set in Advanced and kept.</span>{/if}
        </div>
      </div>

      <div class="field-group">
        <label class="field-label" for="ce-gift">Daily gift</label>
        <div class="field-control">
          <PrototypePicker id="ce-gift" bind:value={gift} {serverExe} classFilter="ItemPrototype" placeholder="Item (type to search)" />
          <span class="field-hint">Given once a day, when a player logs in.</span>
        </div>
      </div>

      <div class="field-group field-group-center">
        <span class="field-label">Announce</span>
        <div class="field-control field-inline">
          <div class="toggle-switch" class:on={announce} role="switch" aria-checked={announce} aria-label="Announce to players" tabindex="0"
            on:click={() => announce = !announce} on:keydown={(e) => e.key === 'Enter' && (announce = !announce)}></div>
          <span class="field-text">Name it in the "Today's Events" message</span>
        </div>
      </div>
    </div>

    <div class="modal-footer">
      {#if onDelete}<button class="btn btn-sm btn-outline delete-btn" on:click={remove}>Delete</button>{/if}
      <button class="btn btn-sm btn-outline" on:click={tryClose}>Cancel</button>
      <button class="btn btn-sm btn-accent" on:click={save}>{event ? 'Done' : 'Add'}</button>
    </div>
  </div>
</div>

<style>
  .modal {
    width: min(600px, 92vw);
    max-height: 85vh;
  }
  .modal-body {
    flex: 1;
    overflow-y: auto;
    padding: 4px 0;
  }
  .field-group {
    display: grid;
    grid-template-columns: 110px 1fr;
    gap: 10px;
    padding: 10px 16px;
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
  .field-control {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .field-inline { flex-direction: row; align-items: center; gap: 10px; }
  /* A row with a toggle, not a text box: centre the label on it. */
  .field-group-center { align-items: center; }
  .field-group-center .field-label { padding-top: 0; margin-bottom: 0; }
  .field-input {
    background: var(--bg-0);
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    color: var(--text-0);
    font-family: var(--font-body);
    font-size: 12px;
    padding: 6px 10px;
    outline: none;
    width: 100%;
  }
  .field-input:focus { border-color: var(--accent-dim); }
  .field-input.input-error { border-color: var(--text-error); }
  .field-error { font-size: 12px; color: var(--text-error); }
  .field-hint { font-size: 12px; line-height: 1.45; color: var(--text-2); }
  .field-text { font-size: 12px; color: var(--text-1); }

  .day-row, .date-row { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
  .day-btn {
    width: 44px;
    height: 28px;
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    background: var(--bg-0);
    color: var(--text-2);
    font-family: var(--font-head);
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
  }
  .day-btn.on { border-color: var(--accent); background: var(--accent-glow-strong); color: var(--accent); }
  .date-row .field-select { width: auto; }
  .date-len { width: 64px; font-variant-numeric: tabular-nums; }

  .slider-row {
    display: grid;
    grid-template-columns: minmax(0, 1.2fr) minmax(0, 1fr) 76px;
    align-items: center;
    gap: 12px;
  }
  .slider-name {
    display: flex;
    align-items: center;
    gap: 6px;
    font-family: var(--font-head);
    font-size: 12px;
    font-weight: 600;
    color: var(--text-1);
  }
  .slider-box {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    height: 28px;
    padding: 0 8px 0 4px;
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    background: var(--bg-0);
  }
  .slider-box:focus-within { border-color: var(--accent-dim); }
  .slider-box input {
    flex: 1;
    min-width: 0;
    border: 0;
    padding: 0;
    background: transparent;
    color: var(--text-0);
    font-family: var(--font-body);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    text-align: right;
    outline: none;
  }
  .slider-unit { font-size: 12px; color: var(--text-3); }
  .delete-btn { margin-right: auto; }
  .delete-btn:hover:not(:disabled) { border-color: var(--text-error); color: var(--text-error); }
</style>
