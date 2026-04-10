<script lang="ts">
  // ── Types ───────────────────────────────────────────────────────────────────

  interface EventDefinition {
    id: string
    display_name: string
    file_path: string
    daily_gift: string | null
    instanced_missions: string[] | null
    is_hidden: boolean | null
  }

  interface ScheduleRule {
    name: string
    is_enabled: boolean
    rule_type: string
    start_day_of_week: string | null
    start_month: number | null
    start_day: number | null
    duration_days: number | null
    events: string[]
  }

  // ── Props ───────────────────────────────────────────────────────────────────

  export let rule: ScheduleRule | null
  export let allEvents: EventDefinition[]
  export let onSave: (rule: ScheduleRule) => void
  export let onDiscard: () => void
  export let saving = false
  export let onEditDefinition: ((def: EventDefinition) => void) | null = null
  export let onOpenTuning: ((filePath: string) => void) | null = null

  // ── Constants ───────────────────────────────────────────────────────────────

  const DAYS_OF_WEEK = ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday']

  const RULE_TYPES = [
    { value: 'AlwaysOn',         label: 'Always on' },
    { value: 'WeeklyRotation',   label: 'Weekly rotation' },
    { value: 'DayOfWeek',        label: 'Day of week' },
    { value: 'SpecialDate',      label: 'Special date' },
    { value: 'SpecialDateLunar', label: 'Special date (lunar)' },
  ]

  // ── Local editing state (initialised from prop on mount) ────────────────────

  let name:          string  = rule?.name             ?? ''
  let is_enabled:    boolean = rule?.is_enabled        ?? true
  let rule_type:     string  = rule?.rule_type         ?? 'AlwaysOn'
  let start_dow:     string  = rule?.start_day_of_week ?? 'Monday'
  let start_month:   number  = rule?.start_month       ?? 1
  let start_day:     number  = rule?.start_day         ?? 1
  let duration_days: number  = rule?.duration_days     ?? 7
  let events:        string[] = [...(rule?.events      ?? [])]

  let addEventId = ''
  let nameError  = ''

  // ── Derived ─────────────────────────────────────────────────────────────────

  $: needsDow       = rule_type === 'WeeklyRotation' || rule_type === 'DayOfWeek'
  $: needsDate      = rule_type === 'SpecialDate'    || rule_type === 'SpecialDateLunar'
  $: isRotation     = rule_type === 'WeeklyRotation'
  $: availableToAdd = allEvents.filter(e => !events.includes(e.id))

  // ── Helpers ─────────────────────────────────────────────────────────────────

  function eventLabel(id: string): string {
    return allEvents.find(e => e.id === id)?.display_name ?? id
  }

  function definitionFor(id: string): EventDefinition | undefined {
    return allEvents.find(e => e.id === id)
  }

  function addEvent() {
    if (!addEventId || events.includes(addEventId)) return
    events = [...events, addEventId]
    addEventId = ''
  }

  function removeEvent(id: string) {
    events = events.filter(e => e !== id)
  }

  function moveEvent(i: number, dir: -1 | 1) {
    const j = i + dir
    if (j < 0 || j >= events.length) return
    const copy = [...events]
    ;[copy[i], copy[j]] = [copy[j], copy[i]]
    events = copy
  }

  function handleSave() {
    nameError = ''
    if (!name.trim()) {
      nameError = 'Name is required.'
      return
    }
    onSave({
      name:             name.trim(),
      is_enabled,
      rule_type,
      start_day_of_week: needsDow  ? start_dow    : null,
      start_month:       needsDate ? start_month  : null,
      start_day:         needsDate ? start_day    : null,
      duration_days:     needsDate ? duration_days : null,
      events,
    })
  }
</script>

<div class="rule-editor">
  <div class="editor-scroll">

    <!-- Name -->
    <div class="field-group">
      <label class="field-label" for="rule-name">Name</label>
      <div class="field-control">
        <input
          id="rule-name"
          class="field-input"
          class:input-error={!!nameError}
          type="text"
          bind:value={name}
          placeholder="Rule name"
          disabled={saving}
        />
        {#if nameError}
          <span class="field-error">{nameError}</span>
        {/if}
      </div>
    </div>

    <!-- Type -->
    <div class="field-group">
      <label class="field-label" for="rule-type">Type</label>
      <select id="rule-type" class="field-select" bind:value={rule_type} disabled={saving}>
        {#each RULE_TYPES as rt}
          <option value={rt.value}>{rt.label}</option>
        {/each}
      </select>
    </div>

    <!-- Enabled -->
    <div class="field-group">
      <span class="field-label">Enabled</span>
      <div class="field-control field-row">
        <button
          class="editor-toggle"
          class:on={is_enabled}
          role="switch"
          aria-checked={is_enabled}
          aria-label={is_enabled ? 'Disable rule' : 'Enable rule'}
          disabled={saving}
          on:click={() => is_enabled = !is_enabled}
        ></button>
        <span class="toggle-hint">{is_enabled ? 'Active' : 'Disabled'}</span>
      </div>
    </div>

    <!-- Start day of week (WeeklyRotation + DayOfWeek) -->
    {#if needsDow}
      <div class="field-group">
        <label class="field-label" for="start-dow">
          {isRotation ? 'Rotation starts' : 'Active on'}
        </label>
        <select id="start-dow" class="field-select" bind:value={start_dow} disabled={saving}>
          {#each DAYS_OF_WEEK as day}
            <option value={day}>{day}</option>
          {/each}
        </select>
      </div>
    {/if}

    <!-- Date fields (SpecialDate + SpecialDateLunar) -->
    {#if needsDate}
      <div class="field-group">
        <label class="field-label" for="start-month">Start month</label>
        <input
          id="start-month"
          class="field-input field-input-narrow"
          type="number" min="1" max="12"
          bind:value={start_month}
          disabled={saving}
        />
      </div>
      <div class="field-group">
        <label class="field-label" for="start-day">Start day</label>
        <input
          id="start-day"
          class="field-input field-input-narrow"
          type="number" min="1" max="31"
          bind:value={start_day}
          disabled={saving}
        />
      </div>
      <div class="field-group">
        <label class="field-label" for="duration">Duration (days)</label>
        <input
          id="duration"
          class="field-input field-input-narrow"
          type="number" min="1"
          bind:value={duration_days}
          disabled={saving}
        />
      </div>
    {/if}

    <!-- Events list -->
    <div class="events-section">
      <div class="events-header">
        <span class="field-label">Events</span>
        {#if events.length > 0}
          <span class="events-count">{events.length}</span>
        {/if}
        {#if isRotation}
          <span class="events-hint">Order determines rotation sequence</span>
        {/if}
      </div>

      {#if events.length === 0}
        <div class="events-empty">No events added yet.</div>
      {:else}
        <div class="events-list">
          {#each events as eventId, i (eventId)}
            <div class="event-row">
              <div class="event-info">
                <span class="event-name">{eventLabel(eventId)}</span>
                <span class="event-id">{eventId}</span>
              </div>
              <div class="event-actions">
                {#if onOpenTuning}
                    {@const def = definitionFor(eventId)}
                    {#if def?.file_path}
                    <button
                        class="btn btn-sm btn-outline"
                        title="Open in Live Tuning editor"
                        disabled={saving}
                        on:click={() => onOpenTuning(def.file_path)}
                    >Tuning</button>
                    {/if}
                {/if}
                {#if onEditDefinition}
                    {@const def = definitionFor(eventId)}
                    {#if def}
                    <button
                        class="btn btn-sm btn-outline"
                        title="Edit event definition"
                        disabled={saving}
                        on:click={() => onEditDefinition(def)}
                    >Edit</button>
                    {/if}
                {/if}
                {#if isRotation}
                    <button
                    class="move-btn"
                    title="Move up"
                    disabled={saving || i === 0}
                    on:click={() => moveEvent(i, -1)}
                    >↑</button>
                    <button
                    class="move-btn"
                    title="Move down"
                    disabled={saving || i === events.length - 1}
                    on:click={() => moveEvent(i, 1)}
                    >↓</button>
                {/if}
                <button
                    class="remove-btn"
                    title="Remove"
                    disabled={saving}
                    on:click={() => removeEvent(eventId)}
                >
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <line x1="18" y1="6" x2="6" y2="18"/>
                    <line x1="6" y1="6" x2="18" y2="18"/>
                    </svg>
                </button>
                </div>
            </div>
          {/each}
        </div>
      {/if}

      <!-- Add event row -->
      {#if availableToAdd.length > 0}
        <div class="add-event-row">
          <select
            class="field-select"
            bind:value={addEventId}
            disabled={saving}
          >
            <option value="">— Select event —</option>
            {#each availableToAdd as def}
              <option value={def.id}>{def.display_name}</option>
            {/each}
          </select>
          <button
            class="btn btn-sm btn-outline"
            disabled={saving || !addEventId}
            on:click={addEvent}
          >Add</button>
        </div>
      {:else if allEvents.length > 0 && events.length === allEvents.length}
        <div class="events-empty">All available events added.</div>
      {/if}
    </div>

  </div>

  <!-- Footer -->
  <div class="editor-footer">
    <button class="btn btn-sm btn-outline" disabled={saving} on:click={onDiscard}>
      Discard
    </button>
    <button class="btn btn-sm btn-accent" disabled={saving} on:click={handleSave}>
      {saving ? 'Saving…' : 'Save to override'}
    </button>
  </div>
</div>

<style>
  .rule-editor {
    display: flex;
    flex-direction: column;
    flex: 1;
    overflow: hidden;
  }

  .editor-scroll {
    flex: 1;
    overflow-y: auto;
    padding: 20px 24px;
    display: flex;
    flex-direction: column;
    gap: 0;
  }

  /* ── Fields ── */

  .field-group {
    display: grid;
    grid-template-columns: 140px 1fr;
    align-items: start;
    gap: 10px;
    padding: 10px 0;
    border-bottom: 1px solid var(--border);
  }
  .field-group:last-child { border-bottom: none; }

  .field-label {
    font-family: var(--font-head);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-2);
    padding-top: 6px;
  }

  .field-control {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .field-row {
    flex-direction: row;
    align-items: center;
    gap: 8px;
  }

  .field-input {
    background: var(--bg-0);
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    color: var(--text-0);
    font-family: var(--font-body);
    font-size: 12px;
    padding: 6px 10px;
    outline: none;
    transition: border-color 0.12s;
    width: 100%;
  }
  .field-input:focus       { border-color: var(--accent-dim); }
  .field-input.input-error { border-color: var(--text-error); }
  .field-input:disabled    { opacity: 0.5; cursor: not-allowed; }
  .field-input-narrow      { max-width: 100px; }

  .field-select {
    background: var(--bg-0);
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    color: var(--text-0);
    font-family: var(--font-body);
    font-size: 12px;
    padding: 6px 10px;
    outline: none;
    cursor: pointer;
    width: 100%;
    max-width: 280px;
  }
  .field-select:focus    { border-color: var(--accent-dim); }
  .field-select:disabled { opacity: 0.5; cursor: not-allowed; }

  .field-error {
    font-family: var(--font-body);
    font-size: 11px;
    color: var(--text-error);
  }

  /* ── Toggle ── */

  .editor-toggle {
    width: 32px;
    height: 18px;
    background: var(--bg-0);
    border: 1px solid var(--border-lit);
    border-radius: 9px;
    position: relative;
    cursor: pointer;
    transition: all 0.18s;
    flex-shrink: 0;
    padding: 0;
  }
  .editor-toggle::after {
    content: '';
    width: 12px;
    height: 12px;
    background: var(--text-3);
    border-radius: 50%;
    position: absolute;
    top: 2px;
    left: 2px;
    transition: all 0.18s;
  }
  .editor-toggle.on                { background: var(--accent-glow-strong); border-color: var(--accent-dim); }
  .editor-toggle.on::after         { left: 16px; background: var(--accent-bright); }
  .editor-toggle:disabled          { opacity: 0.4; cursor: not-allowed; }

  .toggle-hint {
    font-family: var(--font-head);
    font-size: 10px;
    letter-spacing: 0.06em;
    color: var(--text-3);
  }

  /* ── Events section ── */

  .events-section {
    padding: 10px 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .events-header {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .events-count {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--accent-bright);
    background: var(--accent-glow);
    border: 1px solid var(--accent-dim);
    border-radius: 2px;
    padding: 1px 5px;
  }

  .events-hint {
    font-family: var(--font-head);
    font-size: 9px;
    letter-spacing: 0.06em;
    color: var(--text-3);
    margin-left: auto;
  }

  .events-empty {
    font-family: var(--font-body);
    font-size: 11px;
    color: var(--text-3);
    padding: 8px 0;
  }

  .events-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .event-row {
    display: flex;
    align-items: center;
    gap: 8px;
    background: var(--bg-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 7px 10px;
  }

  .event-info {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .event-name {
    font-family: var(--font-head);
    font-size: 11px;
    font-weight: 600;
    color: var(--text-0);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .event-id {
    font-family: var(--font-mono);
    font-size: 9px;
    color: var(--text-3);
  }

  .event-actions {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
  }

  .move-btn {
    width: 22px;
    height: 22px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    color: var(--text-3);
    cursor: pointer;
    font-size: 10px;
    line-height: 1;
    transition: all 0.1s;
    flex-shrink: 0;
  }
  .move-btn:hover:not(:disabled) { color: var(--text-1); border-color: var(--border-lit); }
  .move-btn:disabled             { opacity: 0.25; cursor: not-allowed; }

  .remove-btn {
    width: 22px;
    height: 22px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    color: var(--text-3);
    cursor: pointer;
    transition: all 0.1s;
    flex-shrink: 0;
    padding: 0;
  }
  .remove-btn:hover:not(:disabled) { color: var(--text-error); border-color: rgba(220,60,60,0.3); }
  .remove-btn:disabled             { opacity: 0.25; cursor: not-allowed; }
  .remove-btn svg                  { width: 13px; height: 13px; }

  .add-event-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 4px;
  }
  .add-event-row .field-select { flex: 1; max-width: none; }

  /* ── Footer ── */

  .editor-footer {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
    padding: 10px 16px;
    border-top: 1px solid var(--border);
    flex-shrink: 0;
    background: var(--chrome-sunken-bg);
  }
</style>