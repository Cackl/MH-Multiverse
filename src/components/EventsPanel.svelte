<script lang="ts">
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { openPath } from '@tauri-apps/plugin-opener'
  import { appConfig } from '../lib/store'
  import { activeDataTab, tuningFocusFile, confirmLeave, askConfirm } from '../lib/store'
  import { eventTimezoneOffset } from '../lib/store'
  import { serverNow, ruleIsValid, ruleActiveIds, activeEventIds as eventsActiveAt } from '../lib/eventSchedule'
  import type { TuningFileInfo, EventDefinition, EventsData, ScheduleRule, ScheduleData } from '../lib/tuningMeta'
  import PanelSidebar from './PanelSidebar.svelte'
  import EventRuleEditorModal from './EventRuleEditorModal.svelte'
  import EventDefinitionEditorModal from './EventDefinitionEditorModal.svelte'
  import { tip } from '../lib/tooltip'

  // ── Types ───────────────────────────────────────────────────────────────────

  type RuleStatus = 'active' | 'scheduled' | 'disabled'

  interface ActiveEventItem   { kind: 'event';    rule: ScheduleRule; definition: EventDefinition }
  interface ActiveRotationItem { kind: 'rotation'; rule: ScheduleRule }
  type ActiveItem = ActiveEventItem | ActiveRotationItem

  // ── State ───────────────────────────────────────────────────────────────────

  let eventsData:   EventsData | null   = null
  let scheduleData: ScheduleData | null = null
  let tuningFiles:  TuningFileInfo[]    = []

  let loading  = false
  let loadError = ''
  let saving   = false
  let opError  = ''
  let opSuccess = ''

  let selectedRule:       ScheduleRule | null  = null
  let creatingRule        = false
  let editingDefinition:  EventDefinition | null = null
  let creatingDefinition  = false


  // ── Derived ─────────────────────────────────────────────────────────────────

  $: definitions = (eventsData?.definitions ?? []).slice().sort((a, b) => a.display_name.localeCompare(b.display_name))
  $: rules = (scheduleData?.rules ?? []).slice().sort((a, b) => a.name.localeCompare(b.name))
  $: eventsUsingOverride    = eventsData?.using_override   ?? false
  $: scheduleUsingOverride  = scheduleData?.using_override ?? false

  $: definitionById = Object.fromEntries(definitions.map(d => [d.id, d]))

  // Rule groups for sidebar — SpecialDate and SpecialDateLunar share a group
  const GROUP_ORDER  = ['AlwaysOn', 'DayOfWeek', 'WeeklyRotation', 'SpecialDate'] as const
  const GROUP_LABELS: Record<string, string> = {
    AlwaysOn:       'Always on',
    DayOfWeek:      'Day of week',
    WeeklyRotation: 'Weekly rotation',
    SpecialDate:    'Special dates',
  }

  $: ruleGroups = GROUP_ORDER
    .map(key => ({
      key,
      label: GROUP_LABELS[key],
      rules: rules.filter(r =>
        key === 'SpecialDate'
          ? r.rule_type === 'SpecialDate' || r.rule_type === 'SpecialDateLunar'
          : r.rule_type === key
      ),
    }))
    .filter(g => g.rules.length > 0)

  // What the server runs now, from the shared schedule model.
  $: knownIds = new Set(definitions.map(d => d.id))
  $: now = serverNow($eventTimezoneOffset)
  $: activeIds = eventsActiveAt(rules, knownIds, now)
  $: activeEventIds = [...activeIds]

  // Active items for the dashboard
  $: activeItems = rules.flatMap((rule): ActiveItem[] => {
    if (!ruleIsValid(rule, knownIds)) return []
    const ids = ruleActiveIds(rule, now)
    if (ids.length === 0) return []
    if (rule.rule_type === 'WeeklyRotation') return [{ kind: 'rotation', rule }]
    return ids.flatMap(id => definitionById[id] ? [{ kind: 'event' as const, rule, definition: definitionById[id] }] : [])
  })

  // Key for {#key} block — forces rule editor remount on selection change.
  // By object reference: rule names are not unique in real EventSchedule.json.
  $: editorKey = creatingRule ? '__creating__' : selectedRule

  // ── Status helpers ──────────────────────────────────────────────────────────

  function ruleStatus(rule: ScheduleRule): RuleStatus {
    if (!rule.is_enabled) return 'disabled'
    return ruleIsValid(rule, knownIds) && ruleActiveIds(rule, now).length > 0 ? 'active' : 'scheduled'
  }

  function ruleTypeLabel(rule: ScheduleRule): string {
    switch (rule.rule_type) {
      case 'AlwaysOn':         return 'Always on'
      case 'WeeklyRotation':   return 'Weekly rotation'
      case 'DayOfWeek':        return 'Day of week'
      case 'SpecialDate':      return 'Special date'
      case 'SpecialDateLunar': return 'Lunar date'
      default:                 return rule.rule_type
    }
  }

  // ── Data loading ─────────────────────────────────────────────────────────────

  async function loadData() {
    if (!$appConfig.server_exe) return
    loading   = true
    loadError = ''
    try {
      const [evts, sched, tFiles] = await Promise.all([
        invoke<EventsData>('load_events',         { serverExe: $appConfig.server_exe }),
        invoke<ScheduleData>('load_event_schedule', { serverExe: $appConfig.server_exe }),
        invoke<TuningFileInfo[]>('scan_tuning_files', { serverExe: $appConfig.server_exe, applyAutoEnable: true }),
      ])
      eventsData   = evts
      scheduleData = sched
      tuningFiles  = tFiles
    } catch (e) {
      loadError = String(e)
    } finally {
      loading = false
    }
  }

  let openDirError = ''

  async function openLiveTuningDir() {
    if (!$appConfig.server_exe) return
    openDirError = ''
    try {
      const dir = await invoke<string>('get_live_tuning_dir', { serverExe: $appConfig.server_exe })
      await openPath(dir)
    } catch (e) {
      openDirError = String(e)
    }
  }

  // ── Rule operations ──────────────────────────────────────────────────────────

  async function saveRule(updated: ScheduleRule) {
    if (!scheduleData) return
    saving  = true
    opError = ''
    try {
      const newRules = creatingRule
        ? [...rules, updated]
        : rules.map(r => r === selectedRule ? updated : r)
      await invoke('save_schedule_override', {
        serverExe: $appConfig.server_exe,
        rules: newRules,
      })
      scheduleData = { rules: newRules, using_override: true }
      selectedRule = updated
      creatingRule = false
      opSuccess    = 'Saved'
      setTimeout(() => opSuccess = '', 3000)
    } catch (e) {
      opError = String(e)
    } finally {
      saving = false
    }
  }

  function startNewRule() {
    selectedRule = null
    creatingRule = true
  }

  function selectRule(rule: ScheduleRule) {
    selectedRule = rule
    creatingRule = false
  }

  function discardEditor() {
    selectedRule = null
    creatingRule = false
  }

  // ── Definition operations ────────────────────────────────────────────────────

  async function saveDefinition(updated: EventDefinition) {
    if (!eventsData) return
    saving  = true
    opError = ''
    try {
      const newDefs = creatingDefinition
        ? [...definitions, updated]
        : definitions.map(d => d.id === editingDefinition?.id ? updated : d)
      await invoke('save_events_override', {
        serverExe: $appConfig.server_exe,
        definitions: newDefs,
      })
      eventsData        = { definitions: newDefs, using_override: true }
      editingDefinition = null
      creatingDefinition = false
      opSuccess = 'Saved'
      setTimeout(() => opSuccess = '', 3000)
    } catch (e) {
      opError = String(e)
    } finally {
      saving = false
    }
  }

  // ── Override operations ──────────────────────────────────────────────────────

  async function resetEvents() {
    saving = true; opError = ''
    try {
      eventsData = await invoke<EventsData>('reset_events_override', { serverExe: $appConfig.server_exe })
    } catch (e) { opError = String(e) }
    finally { saving = false }
  }

  async function mergeEvents() {
    saving = true; opError = ''
    try {
      eventsData = await invoke<EventsData>('merge_events_override', { serverExe: $appConfig.server_exe })
    } catch (e) { opError = String(e) }
    finally { saving = false }
  }

  async function resetSchedule() {
    saving = true; opError = ''
    try {
      scheduleData = await invoke<ScheduleData>('reset_schedule_override', { serverExe: $appConfig.server_exe })
      discardEditor()  // selection is by object reference; these are new objects
    } catch (e) { opError = String(e) }
    finally { saving = false }
  }

  async function mergeSchedule() {
    saving = true; opError = ''
    try {
      scheduleData = await invoke<ScheduleData>('merge_schedule_override', { serverExe: $appConfig.server_exe })
      discardEditor()  // selection is by object reference; these are new objects
    } catch (e) { opError = String(e) }
    finally { saving = false }
  }

  // Override files, shown at the bottom of the dashboard
  $: overrideRows = [
    { label: 'Events',   file: 'EventsOverride.json',        what: 'events', using: eventsUsingOverride,   reset: resetEvents,   merge: mergeEvents },
    { label: 'Schedule', file: 'EventScheduleOverride.json', what: 'rules',  using: scheduleUsingOverride, reset: resetSchedule, merge: mergeSchedule },
  ]
  type OverrideRow = typeof overrideRows[number]

  async function confirmReset(row: OverrideRow) {
    if (await askConfirm({
      title: `Reset ${row.label}?`,
      message: `${row.file} goes back to the default ${row.what}. Your changes to them are lost.`,
      okLabel: 'Reset', cancelLabel: 'Cancel',
    })) await row.reset()
  }

  async function confirmMerge(row: OverrideRow) {
    if (await askConfirm({
      title: `Merge default ${row.what}?`,
      message: `Adds the default ${row.what} that ${row.file} is missing. Your changes stay.`,
      okLabel: 'Merge', cancelLabel: 'Cancel',
    })) await row.merge()
  }

  onMount(() => { if ($appConfig.server_exe) loadData() })

  async function openInTuning(filePath: string) {
    if (!filePath || !(await confirmLeave())) return
    tuningFocusFile.set(filePath)
    activeDataTab.set('tuning')
  }
</script>

<div class="events-panel">
  <div class="panel-bg"></div>
  <div class="grid-overlay"></div>
  <div class="events-layout">

    <!-- ── Sidebar ── -->
    <PanelSidebar width="var(--sidebar-wide)">
      <svelte:fragment slot="header">
        <div class="section-title">Schedule Rules</div>
        <button
          class="btn-icon"
          style="margin-left:auto;"
          aria-label="New schedule rule"
          use:tip={'New schedule rule'}
          disabled={!$appConfig.server_exe || loading}
          on:click={startNewRule}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="12" y1="5" x2="12" y2="19"/>
            <line x1="5" y1="12" x2="19" y2="12"/>
          </svg>
        </button>
        <button
          class="btn-icon"
          aria-label="Reload events and schedule"
          use:tip={'Reload events and schedule'}
          disabled={loading}
          on:click={loadData}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="23 4 23 10 17 10"/>
            <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/>
          </svg>
        </button>
        <button
          class="btn-icon"
          on:click={openLiveTuningDir}
          aria-label="Open LiveTuning folder"
          use:tip={'Open LiveTuning folder'}
          disabled={!$appConfig.server_exe}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
          </svg>
        </button>
      </svelte:fragment>

      <!-- Rule list -->
      <div class="sidebar-body">
        {#if !$appConfig.server_exe}
          <div class="file-notice">Set server exe in Settings to load events.</div>
        {:else if loading}
          <div class="file-notice">Loading...</div>
        {:else if loadError}
          <div class="file-notice error">{loadError}</div>
        {:else if openDirError}
          <div class="file-notice error">{openDirError}</div>
        {:else if rules.length === 0}
          <div class="file-notice">No schedule rules found.</div>
        {:else}
          <div class="rule-list">
            {#each ruleGroups as group (group.key)}
              <div class="side-label">{group.label}</div>
              {#each group.rules as rule (rule)}
                {@const status = ruleStatus(rule)}
                <div
                  class="side-item"
                  class:active={selectedRule === rule && !creatingRule}
                  class:off={!rule.is_enabled}
                  role="button"
                  tabindex="0"
                  on:click={() => selectRule(rule)}
                  on:keydown={e => e.key === 'Enter' && selectRule(rule)}
                >
                  <span class="status-dot dot-{status}"></span>
                  <span class="side-name">{rule.name}</span>
                </div>
              {/each}
            {/each}
          </div>
        {/if}

      </div>
    </PanelSidebar>

    <section class="data-main">

      <header class="page-head">
        {#if selectedRule !== null || creatingRule}
          <button class="btn-icon" aria-label="Back to all events" use:tip={'Back to all events'} on:click={discardEditor}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="15 18 9 12 15 6"/>
            </svg>
          </button>
          <h3 class="page-title">{creatingRule ? 'New Rule' : selectedRule?.name}</h3>
          {#if !creatingRule && selectedRule}<span class="tag">{ruleTypeLabel(selectedRule)}</span>{/if}
        {:else}
          <h3 class="page-title">Events</h3>
        {/if}
        <div class="page-acts">
          {#if opError}<span class="feedback-error">{opError}</span>{/if}
          {#if opSuccess}<span class="feedback-ok">{opSuccess}</span>{/if}
          {#if $appConfig.server_exe && !loading && !loadError && selectedRule === null && !creatingRule}
            <button class="btn btn-sm btn-outline" on:click={() => { creatingDefinition = true; editingDefinition = null }}>New event</button>
          {/if}
        </div>
      </header>

      {#if !$appConfig.server_exe}
        <div class="empty-state">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/>
            <circle cx="12" cy="12" r="3"/>
          </svg>
          <span class="empty-state-label">No server configured</span>
          <span class="empty-state-sub">Set the server exe path in Settings.</span>
        </div>

      {:else if loading}
        <div class="empty-state">
          <span class="empty-state-label">Loading…</span>
        </div>

      {:else if loadError}
        <div class="empty-state">
          <span class="empty-state-label error">Load error</span>
          <span class="empty-state-sub">{loadError}</span>
        </div>

      {:else if selectedRule !== null || creatingRule}

        <div class="editor-wrap">
          {#key editorKey}
            <EventRuleEditorModal
                rule={creatingRule ? null : selectedRule}
                allEvents={definitions}
                activeEventIds={activeEventIds}
                onSave={saveRule}
                onDiscard={discardEditor}
                onEditDefinition={(def) => { editingDefinition = def; creatingDefinition = false }}
                onOpenTuning={openInTuning}
                {saving}
            />
          {/key}
        </div>

      {:else}

        <!-- Dashboard view -->
        <div class="dashboard-scroll">

          <!-- Active now -->
          <div class="dash-section">
            <div class="subsection-title">Running now</div>

            {#if activeItems.length === 0}
              <div class="dash-empty">Nothing is running now.</div>
            {:else}
              <div class="active-grid">
                {#each activeItems as item}
                  {#if item.kind === 'rotation'}
                    {@const rotationActiveId = ruleActiveIds(item.rule, now)[0]}
                    {@const rotationActiveName = rotationActiveId ? (definitionById[rotationActiveId]?.display_name ?? rotationActiveId) : null}
                    <div class="active-card event-card"
                        role="button"
                        tabindex="0"
                        use:tip={'Edit this rule'}
                        on:click={() => selectRule(item.rule)}
                        on:keydown={e => e.key === 'Enter' && selectRule(item.rule)}>
                      <div class="active-card-top">
                        <span class="status-dot dot-active"></span>
                        <span class="active-card-name">{item.rule.name}</span>
                      </div>
                      {#if rotationActiveName}
                        <span class="active-card-now">Now: {rotationActiveName}</span>
                      {/if}
                    </div>
                  {:else}
                    <div
                      class="active-card event-card"
                      role="button"
                      tabindex="0"
                      use:tip={`Edit this event. Runs from the ${item.rule.name} rule.`}
                      on:click={() => { editingDefinition = item.definition; creatingDefinition = false }}
                      on:keydown={e => e.key === 'Enter' && (editingDefinition = item.definition)}
                    >
                      <div class="active-card-top">
                        <span class="status-dot dot-active"></span>
                        <span class="active-card-name">{item.definition.display_name}</span>
                        {#if item.definition.daily_gift}<span class="tag" use:tip={'Players get a gift once a day while it runs'}>Gift</span>{/if}
                      </div>
                    </div>
                  {/if}
                {/each}
              </div>
            {/if}
          </div>

          <!-- All events -->
          <div class="dash-section">
            <div class="subsection-title">All events</div>

              {#if definitions.length === 0}
                <div class="dash-empty">No event definitions found.</div>
              {:else}
                <div class="def-list">
                  {#each definitions as def (def.id)}
                    {@const isActive = activeIds.has(def.id)}
                    <div class="def-row">
                      <span class="status-dot dot-{isActive ? 'active' : 'disabled'}"></span>
                      <div class="def-info">
                        <span class="def-name" use:tip={def.file_path || 'No tuning file'}>{def.display_name}</span>
                      </div>
                      {#if def.is_hidden}
                        <span class="tag" use:tip={'Left out of the "Today\'s Events" message'}>Hidden</span>
                      {/if}
                      {#if def.file_path}
                        <button
                          class="btn btn-sm btn-outline def-edit-btn"
                          use:tip={'Open in Live Tuning editor'}
                          on:click={() => openInTuning(def.file_path)}
                        >Tuning</button>
                      {/if}
                      <button
                        class="btn btn-sm btn-outline def-edit-btn"
                        on:click={() => { editingDefinition = def; creatingDefinition = false }}
                      >Edit</button>
                    </div>
                  {/each}
                </div>
              {/if}
          </div>

          <!-- Override files -->
          <div class="dash-section">
            <div class="subsection-title">Override files</div>
            <div class="def-list">
              {#each overrideRows as row (row.label)}
                <div class="def-row">
                  <div class="def-info"><span class="def-name">{row.label}</span></div>
                  <span class="tag" use:tip={row.file}>{row.using ? 'Override' : 'Default'}</span>
                  <button class="btn btn-sm btn-outline" disabled={saving} use:tip={`Replace ${row.file} with the default ${row.what}`} on:click={() => confirmReset(row)}>Reset</button>
                  <button class="btn btn-sm btn-outline" disabled={saving} use:tip={`Add the default ${row.what} that ${row.file} is missing`} on:click={() => confirmMerge(row)}>Merge</button>
                </div>
              {/each}
            </div>
          </div>

        </div><!-- dashboard-scroll -->
      {/if}

    </section>

  </div><!-- events-layout -->
</div>

<!-- Definition editor modal (floating) -->
{#if editingDefinition !== null || creatingDefinition}
  <EventDefinitionEditorModal
    definition={creatingDefinition ? null : editingDefinition}
    {tuningFiles}
    onSave={(def) => saveDefinition(def)}
    serverExe={$appConfig.server_exe}
    onClose={() => { editingDefinition = null; creatingDefinition = false }}
  />
{/if}

<style>
  /* ── Shell ── */

  .events-panel {
    display: flex;
    flex: 1;
    flex-direction: column;
    position: relative;
    overflow: hidden;
    min-height: 0;
  }

  .events-layout {
    position: relative;
    z-index: 1;
    flex: 1;
    display: flex;
    overflow: hidden;
    min-height: 0;
  }

  /* ── Sidebar internals ── */

  .sidebar-body {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  .file-notice {
    padding: 16px 14px;
    font-family: var(--font-head);
    font-size: var(--fs-min);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-2);
  }
  .file-notice.error {
    color: var(--text-error);
    text-transform: none;
    font-family: var(--font-body);
    letter-spacing: 0;
  }

  .rule-list {
    flex: 1;
    overflow-y: auto;
    padding: 6px;
  }
  .rule-list > .side-label:first-child { margin-top: 0; border-top: none; }
  /* Advanced sidebar size, matching the Patches and Store file rows. */
  .side-item { gap: 8px; padding: 7px 10px; min-height: 42px; font-size: 11px; }
  .side-item.off { opacity: var(--dimmed-opacity); }


  /* ── Status dots ── */

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    border: 1px solid var(--border-mid);
    flex-shrink: 0;
  }
  .dot-active    { border-color: var(--accent-bright); background: var(--accent-bright); box-shadow: 0 0 0 3px var(--accent-glow); }
  .dot-scheduled { border-color: var(--accent); }

  .editor-wrap {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  /* ── Dashboard ── */

  .dashboard-scroll {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0;
  }

  .dash-section {
    border-bottom: 1px solid var(--border);
    padding: 16px;
  }
  .dash-section:last-child { border-bottom: none; }

  .dash-empty {
    font-family: var(--font-body);
    font-size: 11px;
    color: var(--text-3);
    padding: 4px 0 8px;
  }

  /* ── Active event grid ── */

  .active-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: 10px;
  }

  .active-card {
    background: var(--bg-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    transition: border-color 0.14s;
  }

  .event-card {
    cursor: pointer;
  }
  .event-card:hover { border-color: var(--border-lit); }
  .active-card-top {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .active-card-name {
    flex: 1;
    min-width: 0;
    font-family: var(--font-head);
    font-size: 12px;
    font-weight: 700;
    color: var(--text-0);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .active-card-now {
    font-size: 11px;
    color: var(--accent-bright);
  }


  /* ── All events list ── */

  .def-list {
    display: flex;
    flex-direction: column;
    gap: 3px;
    margin-top: 4px;
  }

  .def-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 10px;
    background: var(--bg-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    transition: border-color 0.1s;
  }
  .def-row:hover { border-color: var(--border-lit); }

  .def-info {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .def-name {
    font-family: var(--font-head);
    font-size: 11px;
    font-weight: 600;
    color: var(--text-1);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .def-edit-btn { flex-shrink: 0; }
</style>