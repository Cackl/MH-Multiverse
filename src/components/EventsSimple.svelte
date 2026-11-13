<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { openPath } from '@tauri-apps/plugin-opener'
  import { appConfig, serverRunning, eventTimezoneOffset } from '../lib/store'
  import { tip } from '../lib/tooltip'
  import PanelSidebar from './PanelSidebar.svelte'
  import CustomEventModal from './CustomEventModal.svelte'
  import type { EventDefinition, EventsData, ScheduleData, ScheduleRule } from '../lib/tuningMeta'
  import {
    serverNow, activeEventIds, ruleIsValid, nextInRotation, eventMode, setEventMode, scheduleText,
    CUSTOM_PREFIX, EVENT_SLIDERS, slugFor, putCustomEvent, readCustomEvent, deleteCustomEvent, setCustomEnabled,
    type EventMode, type CustomEvent, type EventEntry,
  } from '../lib/eventSchedule'

  type Group = 'Yours' | 'Weekly rotation' | 'Day of week' | 'Yearly' | 'Every day' | 'Not on the calendar' | 'Other'
  const GROUPS: Group[] = ['Yours', 'Weekly rotation', 'Day of week', 'Yearly', 'Every day', 'Not on the calendar', 'Other']
  const MODE_LABEL: Record<EventMode, string> = { schedule: 'On schedule', off: 'Off', always: 'Always on' }

  let savedDefs: EventDefinition[] = []
  let definitions: EventDefinition[] = []
  /** Custom event tuning files to write on Save, by slug, and the slugs of deleted ones. */
  let pendingFiles: Record<string, EventEntry[]> = {}
  let removedSlugs: string[] = []
  /** The custom event open in the form: its id (null when new), what the form shows, and its file's entries. */
  let editing: { id: string | null; ev: CustomEvent | null; entries: EventEntry[] } | null = null
  let saved: ScheduleRule[] = []
  let rules: ScheduleRule[] = []
  let stock: { definitions: EventDefinition[]; rules: ScheduleRule[] } = { definitions: [], rules: [] }
  let loaded = false
  let loadError = ''
  let saving = false
  let saveError = ''
  let folderError = ''
  /** Set by a save; cleared once the server has (re)started, which is when events load. */
  let pendingRestart = false
  let sawStopped = false

  $: serverExe = $appConfig.server_exe
  $: if (serverExe) load(serverExe)

  async function load(exe: string) {
    loadError = ''
    try {
      const [events, schedule, stockEvents] = await Promise.all([
        invoke<EventsData>('load_events', { serverExe: exe }),
        invoke<ScheduleData>('load_event_schedule', { serverExe: exe }),
        invoke<{ definitions: EventDefinition[]; rules: ScheduleRule[] }>('load_stock_events', { serverExe: exe }),
      ])
      savedDefs = byName(events.definitions)
      definitions = savedDefs
      pendingFiles = {}
      removedSlugs = []
      saved = schedule.rules
      rules = saved
      stock = stockEvents
      loaded = true
    } catch (e) {
      loadError = String(e)
    }
  }

  const byName = (defs: EventDefinition[]) => defs.slice().sort((a, b) => a.display_name.localeCompare(b.display_name))
  const slugOf = (id: string) => id.slice(CUSTOM_PREFIX.length)

  $: if (!$serverRunning) sawStopped = true
  $: if ($serverRunning && sawStopped && pendingRestart) pendingRestart = false

  $: now = serverNow($eventTimezoneOffset)
  $: knownIds = new Set(definitions.map(d => d.id))
  $: stockIds = new Set(stock.definitions.map(d => d.id))
  $: nameOf = (id: string) => definitions.find(d => d.id === id)?.display_name ?? id

  // What the server runs: the saved schedule, not unsaved edits.
  $: running = [...activeEventIds(saved, knownIds, now)].map(nameOf).sort()
  $: nextUp = Object.entries(saved
    .filter(r => r.rule_type === 'WeeklyRotation' && ruleIsValid(r, knownIds))
    .reduce<Record<string, string[]>>((acc, r) => {
      const next = nextInRotation(r, now)
      if (next?.weeks === 1) (acc[r.start_day_of_week!] ??= []).push(nameOf(next.id))
      return acc
    }, {}))

  // Server midnight in local time; the server day starts at UTC minus the offset.
  $: dayStart = new Date(Date.UTC(2000, 0, 1, -$eventTimezoneOffset)).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' })

  function groupOf(id: string): Group {
    if (id.startsWith(CUSTOM_PREFIX)) return 'Yours'
    if (!stockIds.has(id)) return 'Other'
    const types = stock.rules.filter(r => r.events.includes(id)).map(r => r.rule_type)
    if (types.includes('WeeklyRotation')) return 'Weekly rotation'
    if (types.includes('DayOfWeek')) return 'Day of week'
    if (types.includes('SpecialDate') || types.includes('SpecialDateLunar')) return 'Yearly'
    if (types.includes('AlwaysOn')) return 'Every day'
    return 'Not on the calendar'
  }

  /** The choices an event offers. Always on is left out where it adds nothing (already every
   *  day) or would break weekly mission resets (an AlwaysOn rule never changes the instance). */
  function modesFor(def: EventDefinition): EventMode[] {
    const stockRules = stock.rules.filter(r => r.events.includes(def.id))
    const modes: EventMode[] = stockRules.length ? ['schedule', 'off'] : ['off']
    if (!stockRules.some(r => r.rule_type === 'AlwaysOn') && !def.instanced_missions?.length) modes.push('always')
    return modes
  }

  /** Calendar position of a yearly event, so that group reads Jan to Dec; lunar dates sit in early Feb. */
  function dateOrder(id: string): number {
    const r = stock.rules.find(r => r.events.includes(id) && r.start_month != null)
    return !r ? 0 : r.rule_type === 'SpecialDateLunar' ? 205 : r.start_month! * 100 + (r.start_day ?? 0)
  }

  $: rows = definitions.map(def => {
    const stockable = stockIds.has(def.id)
    const mode = eventMode(rules, def.id)
    const defaultMode = eventMode(stock.rules, def.id)
    const custom = def.id.startsWith(CUSTOM_PREFIX)
    const savedDef = savedDefs.find(d => d.id === def.id)
    return {
      def,
      group: groupOf(def.id),
      order: dateOrder(def.id),
      mode,
      defaultMode,
      stockable,
      custom,
      // The form can only show schedules it made; anything reshaped in Advanced stays there.
      editable: custom && readCustomEvent(def, rules, []) !== null,
      active: running.includes(def.display_name),
      changed: mode !== eventMode(saved, def.id) || JSON.stringify(def) !== JSON.stringify(savedDef) || slugOf(def.id) in pendingFiles,
      yours: stockable && mode !== defaultMode,
      // An event that is off still shows when it would run.
      text: scheduleText(mode === 'off' && stockable ? stock.rules : rules, def.id, now),
    }
  })
  $: dirty = rows.filter(r => r.changed).length + removedSlugs.length

  function setMode(id: string, mode: EventMode) {
    rules = setEventMode(rules, stock.rules, id, mode)
    saveError = ''
  }

  async function save() {
    if (!serverExe) return
    saving = true
    saveError = ''
    try {
      const ownedFiles = Object.entries(pendingFiles).map(([slug, entries]) => ({ slug, entries }))
      await invoke('save_event_bundle', { serverExe, definitions, rules, ownedFiles, removedSlugs })
      pendingRestart = true
      sawStopped = !$serverRunning
      await load(serverExe)
    } catch (e) {
      saveError = String(e)
    } finally {
      saving = false
    }
  }

  async function openEditor(def: EventDefinition | null) {
    saveError = ''
    if (!def) {
      editing = { id: null, ev: null, entries: [] }
      return
    }
    let entries = pendingFiles[slugOf(def.id)]
    if (!entries) {
      try {
        entries = await invoke<EventEntry[]>('read_tuning_file', { serverExe, relativePath: def.file_path })
      } catch (e) {
        // A missing file is fine (the form writes a new one); anything else would lose its entries.
        if (!String(e).startsWith('File not found')) {
          saveError = `Can't read ${def.file_path}: ${e}`
          return
        }
        entries = []
      }
    }
    editing = { id: def.id, ev: readCustomEvent(def, rules, entries), entries }
  }

  function saveCustom(ev: CustomEvent) {
    if (!editing) return
    // Deleted ids count as taken: their files are renamed after the new ones are written.
    const taken = new Set([...definitions, ...savedDefs].map(d => d.id).concat(removedSlugs.map(s => CUSTOM_PREFIX + s)))
    const id = editing.id ?? CUSTOM_PREFIX + slugFor(ev.name, taken)
    const out = putCustomEvent(definitions, rules, id, ev, editing.entries)
    definitions = byName(out.definitions)
    rules = out.rules
    pendingFiles = { ...pendingFiles, [slugOf(id)]: out.entries }
    editing = null
  }

  function deleteCustom() {
    if (!editing?.id) return
    const { id } = editing
    const out = deleteCustomEvent(definitions, rules, id)
    definitions = out.definitions
    rules = out.rules
    const { [slugOf(id)]: _, ...rest } = pendingFiles
    pendingFiles = rest
    if (savedDefs.some(d => d.id === id)) removedSlugs = [...removedSlugs, slugOf(id)]
    editing = null
  }

  async function openFolder() {
    folderError = ''
    try {
      await openPath(await invoke<string>('get_live_tuning_dir', { serverExe }))
    } catch (e) {
      folderError = String(e)
    }
  }

  const jumpTo = (group: Group) => document.getElementById(`events-${group.replace(/ /g, '-')}`)?.scrollIntoView({ block: 'start' })
</script>

<div class="events">
  <div class="panel-bg"></div>
  <div class="grid-overlay"></div>
  <div class="events-layout">
    <PanelSidebar width="var(--sidebar-narrow)">
      <svelte:fragment slot="header">
        <div class="section-title">Sections</div>
        <button
          class="btn-icon"
          style="margin-left:auto;"
          on:click={openFolder}
          disabled={!serverExe}
          aria-label="Open LiveTuning folder"
          use:tip={'Open LiveTuning folder'}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
          </svg>
        </button>
      </svelte:fragment>

      <div class="events-side">
        {#each GROUPS.filter(g => rows.some(r => r.group === g)) as group}
          <button class="side-item" on:click={() => jumpTo(group)}><span class="side-name">{group}</span></button>
        {/each}
      </div>
    </PanelSidebar>

    <section class="data-main">
      <header class="page-head">
        <h3 class="page-title">Events</h3>
        <div class="page-acts">
          <button class="btn btn-sm btn-outline" on:click={() => openEditor(null)} disabled={!loaded}>Make your own</button>
          <span class="day-start" use:tip={`The server's day starts at ${String(-$eventTimezoneOffset).padStart(2, '0')}:00 UTC`}>
            Event days start at {dayStart} your time
          </span>
        </div>
      </header>

      <div class="events-body">
        {#if !serverExe}
          <p class="events-msg">Set the server location first.</p>
        {:else if loadError}
          <p class="events-msg error">{loadError}</p>
        {:else if loaded && definitions.length === 0}
          <p class="events-msg">This server has no events.</p>
        {:else if loaded}
          <div class="now-line">
            <span class="now">Now: {running.length ? running.join(', ') : 'nothing'}</span>
            {#each nextUp as [day, names]}
              <span class="next">Next {day}: {names.join(', ')}</span>
            {/each}
          </div>

          {#each GROUPS as group}
            {@const groupRows = rows.filter(r => r.group === group).sort((a, b) => a.order - b.order)}
            {#if groupRows.length}
              <div class="subsection-title events-group" id="events-{group.replace(/ /g, '-')}">{group}</div>
              {#each groupRows as row (row.def.id)}
                <div class="event-row" class:off={row.mode === 'off'}>
                  <span class="event-name">
                    <span class="event-dot" class:active={row.active} use:tip={row.active ? 'Running now' : 'Not running now'}></span>
                    <span class="event-title">{row.def.display_name}</span>
                    {#if row.def.is_hidden}<span class="tag" use:tip={'Left out of the "Today\'s Events" message'}>Hidden</span>{/if}
                    {#if row.def.daily_gift}<span class="tag" use:tip={'Players get a gift once a day while it runs'}>Gift</span>{/if}
                  </span>

                  {#if row.changed}
                    <span class="event-badge changed">Changed</span>
                  {:else if row.custom}
                    <span class="event-badge yours">Yours</span>
                  {:else if !row.stockable}
                    <span class="event-badge">Advanced</span>
                  {:else if row.yours}
                    <span class="event-badge yours">Yours</span>
                  {:else}
                    <span class="event-badge">Default</span>
                  {/if}

                  <span class="event-when">{row.text}</span>

                  {#if row.custom}
                    <select
                      class="field-select event-mode"
                      aria-label="{row.def.display_name} schedule"
                      value={row.mode === 'off' ? 'off' : 'schedule'}
                      on:change={(e) => rules = setCustomEnabled(rules, row.def.id, e.currentTarget.value !== 'off')}
                    >
                      <option value="schedule">On schedule</option>
                      <option value="off">Off</option>
                    </select>
                  {:else if row.stockable}
                    <select
                      class="field-select event-mode"
                      aria-label="{row.def.display_name} schedule"
                      value={row.mode}
                      on:change={(e) => setMode(row.def.id, e.currentTarget.value as EventMode)}
                    >
                      {#each modesFor(row.def) as mode}<option value={mode}>{MODE_LABEL[mode]}</option>{/each}
                    </select>
                  {:else}
                    <span class="event-mode event-fixed" use:tip={'Not a stock event. Change it in Advanced.'}>{MODE_LABEL[row.mode]}</span>
                  {/if}

                  <span class="event-revert">
                    {#if row.custom}
                      <button
                        class="btn-icon"
                        on:click={() => openEditor(row.def)}
                        disabled={!row.editable}
                        aria-label="Edit {row.def.display_name}"
                        use:tip={row.editable ? 'Edit' : 'Its schedule was changed in Advanced. Edit it there.'}
                      >
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                          <path d="M12 20h9"/>
                          <path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4z"/>
                        </svg>
                      </button>
                    {:else if row.stockable && row.mode !== row.defaultMode}
                      <button
                        class="btn-icon"
                        on:click={() => setMode(row.def.id, row.defaultMode)}
                        aria-label="Revert {row.def.display_name}"
                        use:tip={`Revert to ${MODE_LABEL[row.defaultMode]}`}
                      >
                        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                          <path d="M3 12a9 9 0 1 0 3-6.7"/>
                          <polyline points="3 3 3 9 9 9"/>
                        </svg>
                      </button>
                    {/if}
                  </span>
                </div>
              {/each}
            {/if}
          {/each}
        {/if}
      </div>

      <footer class="panel-footer">
        <div class="events-status">
          {#if saveError}<span class="error">{saveError}</span>
          {:else if folderError}<span class="error">{folderError}</span>
          {:else if dirty}<span class="dirty-badge">{dirty} unsaved {dirty === 1 ? 'change' : 'changes'}</span>
          {:else if pendingRestart}<span>{$serverRunning ? 'Takes effect after a server restart.' : 'Applies next time the server starts.'}</span>{/if}
        </div>
        <button
          class="btn btn-sm btn-accent"
          on:click={save}
          use:tip={'Saves to EventsOverride.json and EventScheduleOverride.json'}
          disabled={saving || dirty === 0}
        >{saving ? 'Saving...' : 'Save'}</button>
      </footer>
    </section>
  </div>
</div>

{#if editing && serverExe}
  <CustomEventModal
    event={editing.ev}
    extraCount={editing.entries.filter(e => !(e.prototype === '' && EVENT_SLIDERS.includes(e.setting))).length}
    {serverExe}
    onSave={saveCustom}
    onDelete={editing.id ? deleteCustom : null}
    onClose={() => editing = null}
  />
{/if}

<style>
  .events {
    display: flex;
    flex: 1;
    position: relative;
    overflow: hidden;
    min-height: 0;
  }
  .events-layout {
    position: relative;
    z-index: 1;
    flex: 1;
    display: flex;
    min-height: 0;
  }
  .events-side { padding: 6px; }
  .events-body {
    flex: 1;
    overflow-y: auto;
    padding: 4px 20px 22px;
  }
  .events-msg { font-size: 12px; color: var(--text-2); }
  .error { font-size: 12px; color: var(--text-error); }
  .events-status {
    flex: 1;
    min-width: 0;
    font-size: 12px;
    color: var(--text-1);
  }
  .day-start { font-size: 12px; color: var(--text-2); white-space: nowrap; }

  .now-line {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 18px;
    padding: 12px 0 0;
    font-size: 12px;
  }
  .now { color: var(--accent); }
  .next { color: var(--text-2); }

  .events-group { margin: 14px 0 0; scroll-margin-top: 4px; }

  /* Name over badge on the left, when it runs, the control, revert on the right. */
  .event-row {
    display: grid;
    grid-template-columns: minmax(0, 1.3fr) minmax(0, 1fr) 124px 28px;
    column-gap: 16px;
    row-gap: 3px;
    align-items: center;
    padding: 8px 0;
    border-bottom: 1px solid var(--border);
  }
  .event-name {
    grid-column: 1;
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
  }
  .event-title {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-head);
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.02em;
    color: var(--text-1);
  }
  .event-dot {
    flex-shrink: 0;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    border: 1px solid var(--border-mid);
  }
  .event-dot.active {
    border-color: var(--accent-bright);
    background: var(--accent-bright);
    box-shadow: 0 0 0 3px var(--accent-glow);
  }
  .event-badge {
    grid-column: 1;
    padding-left: 15px;
    font-family: var(--font-head);
    font-size: var(--fs-min);
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
    white-space: nowrap;
  }
  .event-badge.yours { color: var(--accent-bright); }
  .event-badge.changed { color: var(--amber-bright); }
  .event-when {
    grid-column: 2;
    grid-row: 1 / span 2;
    font-size: 12px;
    color: var(--text-2);
  }
  .event-mode {
    grid-column: 3;
    grid-row: 1 / span 2;
    width: 100%;
  }
  .event-row.off .event-title,
  .event-row.off .event-when { opacity: var(--dimmed-opacity); }
  .event-fixed { font-size: 12px; color: var(--text-2); padding-left: 11px; }
  .event-revert {
    grid-column: 4;
    grid-row: 1 / span 2;
    display: flex;
    justify-content: center;
  }
</style>
