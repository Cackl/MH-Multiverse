<script lang="ts">
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { appConfig } from '../lib/store'
  import { activeDataTab, tuningFocusFile } from '../lib/store'
  import { get } from 'svelte/store'
  import { eventTimezoneOffset } from '../lib/store'
  import PanelSidebar from './PanelSidebar.svelte'
  import EventRuleEditorModal from './EventRuleEditorModal.svelte'
  import EventDefinitionEditorModal from './EventDefinitionEditorModal.svelte'

  // ── Types ───────────────────────────────────────────────────────────────────

  interface TuningFileInfo {
    canonical_name: string
    enabled: boolean
    toggleable: boolean
    relative_path: string
    event_id: string | null
    was_auto_enabled: boolean
  }

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

  interface EventsData {
    definitions: EventDefinition[]
    using_override: boolean
  }

  interface ScheduleData {
    rules: ScheduleRule[]
    using_override: boolean
  }

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
  let allEventsExpanded   = true
  let activeEventIds: string[] = []

  type PendingAction = 'resetEvents' | 'mergeEvents' | 'resetSchedule' | 'mergeSchedule'
  let pendingAction: PendingAction | null = null

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

  // Active items for the dashboard
  $: activeItems = (() => {
    const items: ActiveItem[] = []
    const now = getAdjustedNow()
    const activeIds = getActiveEventIds(now)

    for (const rule of rules) {
      if (!isActiveNow(rule, now)) continue

      if (rule.rule_type === 'WeeklyRotation') {
        items.push({ kind: 'rotation', rule })
        continue
      }

      for (const id of rule.events) {
        if (!activeIds.has(id)) continue
        const def = definitionById[id]
        if (def) items.push({ kind: 'event', rule, definition: def })
      }
    }

    return items
  })()

  $: activeEventIds = rules.length >= 0 ? Array.from(getActiveEventIds(getAdjustedNow())) : []

  // Key for {#key} block — forces rule editor remount on selection change
  $: editorKey = creatingRule ? '__creating__' : (selectedRule?.name ?? '')

  // ── Status helpers ──────────────────────────────────────────────────────────

  function dayIndex(day: string): number {
    return ['Sunday','Monday','Tuesday','Wednesday','Thursday','Friday','Saturday'].indexOf(day)
  }

  function isActiveNow(rule: ScheduleRule, now: Date): boolean {
    if (!rule.is_enabled) return false
    switch (rule.rule_type) {
      case 'AlwaysOn':
      case 'WeeklyRotation':
        return true
      case 'DayOfWeek':
        return rule.start_day_of_week != null
          && now.getUTCDay() === dayIndex(rule.start_day_of_week)  // was getDay()
      case 'SpecialDate':
      case 'SpecialDateLunar': {
        if (rule.start_month == null || rule.start_day == null || rule.duration_days == null) return false
        const y = now.getUTCFullYear()  // was getFullYear()
        for (const yr of [y - 1, y]) {
          const start = new Date(Date.UTC(yr, rule.start_month - 1, rule.start_day))  // was new Date(yr, ...)
          const end   = new Date(start.getTime() + rule.duration_days * 86400000)
          if (now >= start && now < end) return true
        }
        return false
      }
      default: return false
    }
  }

  function ruleStatus(rule: ScheduleRule): RuleStatus {
    if (!rule.is_enabled) return 'disabled'
    const now = getAdjustedNow()
    return isActiveNow(rule, now) ? 'active' : 'scheduled'
  }

  function ruleTypeLabel(rule: ScheduleRule): string {
    switch (rule.rule_type) {
      case 'AlwaysOn':         return 'Always on'
      case 'WeeklyRotation':   return 'Weekly rotation'
      case 'DayOfWeek':        return `${rule.start_day_of_week ?? 'Day'} only`
      case 'SpecialDate':      return 'Special date'
      case 'SpecialDateLunar': return 'Lunar date'
      default:                 return rule.rule_type
    }
  }

  function getAdjustedNow(): Date {
    const offset = get(eventTimezoneOffset)
    return new Date(Date.now() + offset * 60 * 60 * 1000)
    
  }

  function getWeeklyRotationEvent(events: string[], startDayOfWeek: number, now: Date): string | null {
    if (!events.length) return null

    const epoch = new Date(Date.UTC(2000, 0, 2 + startDayOfWeek))  // was new Date(2000, 0, ...)
    const diffDays = Math.floor((now.getTime() - epoch.getTime()) / 86400000)
    const weekNumber = Math.floor(diffDays / 7)
    const index = ((weekNumber % events.length) + events.length) % events.length

    return events[index] ?? null
  }

  function getActiveEventIds(now: Date): Set<string> {
    const ids = new Set<string>()

    for (const rule of rules) {
      if (!isActiveNow(rule, now)) continue

      if (rule.rule_type === 'WeeklyRotation') {
        const startDay = rule.start_day_of_week != null ? dayIndex(rule.start_day_of_week) : 0
        const activeId = getWeeklyRotationEvent(rule.events, startDay, now)
        if (activeId) ids.add(activeId)
      } else {
        for (const id of rule.events) {
          if (id) ids.add(id)
        }
      }
    }

    return ids
  }

  function definitionActive(def: EventDefinition): boolean {
    const now = getAdjustedNow()
    return getActiveEventIds(now).has(def.id)
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
        invoke<TuningFileInfo[]>('scan_tuning_files', { serverExe: $appConfig.server_exe }),
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

  // ── Rule operations ──────────────────────────────────────────────────────────

  async function saveRule(updated: ScheduleRule) {
    if (!scheduleData) return
    saving  = true
    opError = ''
    try {
      const newRules = creatingRule
        ? [...rules, updated]
        : rules.map(r => r.name === selectedRule?.name ? updated : r)
      await invoke('save_schedule_override', {
        serverExe: $appConfig.server_exe,
        rules: newRules,
      })
      scheduleData = { rules: newRules, using_override: true }
      selectedRule = updated
      creatingRule = false
      opSuccess    = 'Schedule saved.'
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
      opSuccess = 'Events saved.'
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
    } catch (e) { opError = String(e) }
    finally { saving = false }
  }

  async function mergeSchedule() {
    saving = true; opError = ''
    try {
      scheduleData = await invoke<ScheduleData>('merge_schedule_override', { serverExe: $appConfig.server_exe })
    } catch (e) { opError = String(e) }
    finally { saving = false }
  }

  onMount(() => { if ($appConfig.server_exe) loadData() })

  function openInTuning(filePath: string) {
    if (!filePath) return
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
          title="New schedule rule"
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
          title="Reload events and schedule"
          disabled={loading}
          on:click={loadData}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="23 4 23 10 17 10"/>
            <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/>
          </svg>
        </button>
      </svelte:fragment>

      <!-- Rule list -->
      <div class="sidebar-body">
        {#if !$appConfig.server_exe}
          <div class="sidebar-notice">Set server exe in Settings.</div>
        {:else if loading}
          <div class="sidebar-notice">Loading…</div>
        {:else if loadError}
          <div class="sidebar-notice error">{loadError}</div>
        {:else if rules.length === 0}
          <div class="sidebar-notice">No rules found. Check Events.json exists.</div>
        {:else}
          <div class="rule-list">
            {#each ruleGroups as group (group.key)}
              <div class="rule-group-label">{group.label}</div>
              {#each group.rules as rule (rule.name)}
                {@const status = ruleStatus(rule)}
                <div
                  class="rule-item"
                  class:active={selectedRule?.name === rule.name && !creatingRule}
                  class:rule-disabled={!rule.is_enabled}
                  role="button"
                  tabindex="0"
                  on:click={() => selectRule(rule)}
                  on:keydown={e => e.key === 'Enter' && selectRule(rule)}
                >
                  <span class="status-dot dot-{status}"></span>
                  <span class="rule-name">{rule.name}</span>
                </div>
              {/each}
            {/each}
          </div>
        {/if}

        <!-- Override status -->
        {#if $appConfig.server_exe && !loading}
          <div class="override-section">
            <div class="override-title">Override status</div>

            {#if pendingAction}
            <div class="confirm-prompt">
                {pendingAction === 'resetEvents'   ? 'Reset Events to default?' :
                pendingAction === 'mergeEvents'   ? 'Merge defaults into Events?' :
                pendingAction === 'resetSchedule' ? 'Reset Schedule to default?' :
                                                    'Merge defaults into Schedule?'}
            </div>
            {/if}

            <div class="override-row">
            <span class="override-label">Events</span>
            <span class="override-badge" class:is-override={eventsUsingOverride}>
                {eventsUsingOverride ? 'Override' : 'Default'}
            </span>
            {#if pendingAction === 'resetEvents' || pendingAction === 'mergeEvents'}
                <button
                class="btn btn-sm btn-accent override-btn"
                disabled={saving}
                on:click={async () => {
                    const action = pendingAction
                    pendingAction = null
                    if (action === 'resetEvents') await resetEvents()
                    else await mergeEvents()
                }}
                >Yes</button>
                <button
                class="btn btn-sm btn-outline override-btn"
                disabled={saving}
                on:click={() => pendingAction = null}
                >Cancel</button>
            {:else}
                <button
                class="btn btn-sm btn-outline override-btn"
                disabled={saving || !!pendingAction}
                title="Overwrite EventsOverride.json with default Events.json"
                on:click={() => pendingAction = 'resetEvents'}
                >Reset</button>
                <button
                class="btn btn-sm btn-outline override-btn"
                disabled={saving || !!pendingAction}
                title="Add any missing default events to the override"
                on:click={() => pendingAction = 'mergeEvents'}
                >Merge</button>
            {/if}
            </div>

            <div class="override-row">
            <span class="override-label">Schedule</span>
            <span class="override-badge" class:is-override={scheduleUsingOverride}>
                {scheduleUsingOverride ? 'Override' : 'Default'}
            </span>
            {#if pendingAction === 'resetSchedule' || pendingAction === 'mergeSchedule'}
                <button
                class="btn btn-sm btn-accent override-btn"
                disabled={saving}
                on:click={async () => {
                    const action = pendingAction
                    pendingAction = null
                    if (action === 'resetSchedule') await resetSchedule()
                    else await mergeSchedule()
                }}
                >Yes</button>
                <button
                class="btn btn-sm btn-outline override-btn"
                disabled={saving}
                on:click={() => pendingAction = null}
                >Cancel</button>
            {:else}
                <button
                class="btn btn-sm btn-outline override-btn"
                disabled={saving || !!pendingAction}
                title="Overwrite EventScheduleOverride.json with default"
                on:click={() => pendingAction = 'resetSchedule'}
                >Reset</button>
                <button
                class="btn btn-sm btn-outline override-btn"
                disabled={saving || !!pendingAction}
                title="Add any missing default rules to the override"
                on:click={() => pendingAction = 'mergeSchedule'}
                >Merge</button>
            {/if}
            </div>

            {#if opError}
              <div class="override-error">{opError}</div>
            {/if}
            {#if opSuccess}
              <div class="override-ok">{opSuccess}</div>
            {/if}
          </div>
        {/if}
      </div>
    </PanelSidebar>

    <!-- ── Content pane ── -->
    <div class="content-pane">

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
          <span class="empty-state-label" style="color:var(--text-error);">Load error</span>
          <span class="empty-state-sub">{loadError}</span>
        </div>

      {:else if selectedRule !== null || creatingRule}

        <!-- Rule editor view -->
        <div class="editor-header">
          <button class="back-btn" title="Back to dashboard" on:click={discardEditor}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="15 18 9 12 15 6"/>
            </svg>
          </button>
          <span class="editor-title">
            {creatingRule ? 'New rule' : selectedRule?.name}
          </span>
          {#if !creatingRule && selectedRule}
            {@const status = ruleStatus(selectedRule)}
            <span class="status-dot dot-{status}" style="margin-left:4px;"></span>
            <span class="editor-type-label">{ruleTypeLabel(selectedRule)}</span>
          {/if}
        </div>
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
            <div class="dash-section-header">
              <span class="dash-section-title">Active now</span>
              <span class="dash-count">{activeItems.length}</span>
            </div>

            {#if activeItems.length === 0}
              <div class="dash-empty">
                No events are currently active.
              </div>
            {:else}
              <div class="active-grid">
                {#each activeItems as item}
                  {#if item.kind === 'rotation'}
                    {@const rotationActiveId = getWeeklyRotationEvent(item.rule.events, item.rule.start_day_of_week != null ? dayIndex(item.rule.start_day_of_week) : 0, getAdjustedNow())}
                    {@const rotationActiveName = rotationActiveId ? (definitionById[rotationActiveId]?.display_name ?? rotationActiveId) : null}
                    <div class="active-card rotation-card event-card"
                        role="button"
                        tabindex="0"
                        title="Edit schedule rule"
                        on:click={() => selectRule(item.rule)}
                        on:keydown={e => e.key === 'Enter' && selectRule(item.rule)}>
                      <div class="active-card-top">
                        <span class="status-dot dot-active"></span>
                        <span class="active-card-name">{item.rule.name}</span>
                      </div>
                      <div class="active-card-meta">
                        <!-- <span class="active-card-type">Weekly rotation active</span> -->
                        {#if rotationActiveName}
                          <span class="active-card-type">Now: {rotationActiveName}</span>
                        {/if}
                        <span class="active-card-sub">{item.rule.events.length} events in rotation</span>
                      </div>
                    </div>
                  {:else}
                    <div
                      class="active-card event-card"
                      role="button"
                      tabindex="0"
                      title="Edit definition"
                      on:click={() => { editingDefinition = item.definition; creatingDefinition = false }}
                      on:keydown={e => e.key === 'Enter' && (editingDefinition = item.definition)}
                    >
                      <div class="active-card-top">
                        <span class="status-dot dot-active"></span>
                        <span class="active-card-name">{item.definition.display_name}</span>
                      </div>
                      <div class="active-card-meta">
                        {#if item.definition.daily_gift}
                          <span class="active-card-gift">
                            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                              <polyline points="20 12 20 22 4 22 4 12"/>
                              <rect x="2" y="7" width="20" height="5"/>
                              <line x1="12" y1="22" x2="12" y2="7"/>
                              <path d="M12 7H7.5a2.5 2.5 0 0 1 0-5C11 2 12 7 12 7z"/>
                              <path d="M12 7h4.5a2.5 2.5 0 0 0 0-5C13 2 12 7 12 7z"/>
                            </svg>
                            Daily gift
                          </span>
                        {/if}
                        <span class="active-card-rule">{item.rule.name}</span>
                      </div>
                    </div>
                  {/if}
                {/each}
              </div>
            {/if}
          </div>

          <!-- All events -->
          <div class="dash-section">
            <div class="dash-section-header">
              <button
                class="dash-expand-btn"
                on:click={() => allEventsExpanded = !allEventsExpanded}
              >
                <svg
                  viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
                  class:rotated={allEventsExpanded}
                >
                  <polyline points="9 18 15 12 9 6"/>
                </svg>
                <span class="dash-section-title">All events</span>
                <span class="dash-count">{definitions.length}</span>
              </button>
              <button
                class="btn btn-sm btn-outline"
                style="margin-left:auto;"
                on:click={() => { creatingDefinition = true; editingDefinition = null }}
              >
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:11px;height:11px;">
                  <line x1="12" y1="5" x2="12" y2="19"/>
                  <line x1="5" y1="12" x2="19" y2="12"/>
                </svg>
                New definition
              </button>
            </div>

            {#if allEventsExpanded}
              {#if definitions.length === 0}
                <div class="dash-empty">No event definitions found.</div>
              {:else}
                <div class="def-list">
                  {#each definitions as def (def.id)}
                    {@const isActive = definitionActive(def)}
                    <div class="def-row">
                      <span class="status-dot dot-{isActive ? 'active' : 'disabled'}"></span>
                      <div class="def-info">
                        <span class="def-name">{def.display_name}</span>
                        <span class="def-path">{def.file_path || '—'}</span>
                      </div>
                      {#if def.is_hidden}
                        <span class="def-badge">Hidden</span>
                      {/if}
                      {#if def.file_path}
                        <button
                          class="btn btn-sm btn-outline def-edit-btn"
                          title="Open in Live Tuning editor"
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
            {/if}
          </div>

        </div><!-- dashboard-scroll -->
      {/if}

    </div><!-- content-pane -->

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

  .panel-bg {
    position: absolute;
    inset: 0;
    background: linear-gradient(180deg, var(--panel-grad-top) 0%, transparent var(--panel-grad-mid));
    pointer-events: none;
  }

  .grid-overlay {
    position: absolute;
    inset: 0;
    background-image:
      linear-gradient(var(--border) 1px, transparent 1px),
      linear-gradient(90deg, var(--border) 1px, transparent 1px);
    background-size: 32px 32px;
    opacity: 0.18;
    pointer-events: none;
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

  .sidebar-notice {
    padding: 16px 14px;
    font-family: var(--font-head);
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-2);
  }
  .sidebar-notice.error {
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

  .rule-group-label {
    font-family: var(--font-head);
    font-size: 9px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-2);
    padding: 10px 14px 4px;
    border-top: 1px solid var(--border);
    margin-top: 4px;
  }
  .rule-group-label:first-child { border-top: none; margin-top: 0; }

  .rule-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 10px;
    min-height: 38px;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: all 0.1s;
    margin-bottom: 2px;
  }
  .rule-item:hover         { background: var(--bg-3); border-color: var(--border-mid); }
  .rule-item.active        { background: var(--accent-glow); border-color: var(--accent-dim); }
  .rule-item.active .rule-name { color: var(--accent-bright); }
  .rule-item.rule-disabled { opacity: 0.5; }

  .rule-name {
    font-family: var(--font-head);
    font-size: 11px;
    font-weight: 600;
    color: var(--text-1);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
  }

  /* ── Status dots ── */

  .status-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .dot-active    { background: var(--green-bright); box-shadow: 0 0 5px var(--green-bright); }
  .dot-scheduled { background: var(--amber-bright); }
  .dot-disabled  { background: var(--border-lit); }

  /* ── Override section ── */

  .override-section {
    border-top: 1px solid var(--border);
    padding: 10px 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    flex-shrink: 0;
  }

  .override-title {
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-3);
    padding-bottom: 4px;
  }

  .override-row {
    display: flex;
    align-items: center;
    gap: 5px;
  }

  .override-label {
    font-family: var(--font-head);
    font-size: 10px;
    color: var(--text-2);
    min-width: 52px;
  }

  .override-badge {
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    padding: 1px 5px;
    border-radius: 2px;
    border: 1px solid var(--border-mid);
    color: var(--text-3);
    background: none;
    flex-shrink: 0;
  }
  .override-badge.is-override {
    color: var(--accent-bright);
    border-color: var(--accent-dim);
    background: var(--accent-glow);
  }

  .override-btn { padding: 2px 7px; font-size: 9px; margin-left: auto; }
  .override-btn + .override-btn { margin-left: 0; }

  .override-error {
    font-family: var(--font-body);
    font-size: 10px;
    color: var(--text-error);
    padding-top: 2px;
  }

  .override-ok {
    font-family: var(--font-body);
    font-size: 10px;
    color: var(--green-bright);
    padding-top: 2px;
  }

  .confirm-prompt {
    font-family: var(--font-head);
    font-size: 9px;
    letter-spacing: 0.04em;
    color: var(--text-1);
  }

  /* ── Content pane ── */

  .content-pane {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    min-width: 0;
    background: var(--bg-1);
  }

  /* ── Empty state ── */

  .empty-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 32px;
    color: var(--text-2);
  }
  .empty-state svg          { width: 24px; height: 24px; color: var(--text-3); }
  .empty-state-label        { font-family: var(--font-head); font-size: 13px; font-weight: 600; color: var(--text-1); }
  .empty-state-sub          { font-family: var(--font-body); font-size: 11px; color: var(--text-3); text-align: center; }

  /* ── Rule editor header ── */

  .editor-header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
    min-height: 53px;
    flex-shrink: 0;
  }

  .back-btn {
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    color: var(--text-2);
    cursor: pointer;
    transition: all 0.1s;
    flex-shrink: 0;
    padding: 0;
  }
  .back-btn:hover   { color: var(--text-0); border-color: var(--border-lit); }
  .back-btn svg     { width: 16px; height: 16px; }

  .editor-title {
    font-family: var(--font-head);
    font-size: 12px;
    font-weight: 700;
    color: var(--text-0);
  }

  .editor-type-label {
    font-family: var(--font-head);
    font-size: 9px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
  }

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

  .dash-section-header {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 12px;
  }

  .dash-section-title {
    font-family: var(--font-head);
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-2);
  }

  .dash-count {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-3);
    background: var(--bg-3);
    border: 1px solid var(--border-mid);
    border-radius: 2px;
    padding: 1px 5px;
  }

  .dash-empty {
    font-family: var(--font-body);
    font-size: 11px;
    color: var(--text-3);
    padding: 4px 0 8px;
  }

  /* ── Expand button for All events ── */

  .dash-expand-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    background: none;
    border: none;
    cursor: pointer;
    padding: 0;
  }
  .dash-expand-btn svg {
    width: 14px;
    height: 14px;
    color: var(--text-3);
    transition: transform 0.15s;
    flex-shrink: 0;
  }
  .dash-expand-btn svg.rotated { transform: rotate(90deg); }

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
  .rotation-card.event-card:hover { border-color: var(--accent-bright); }

  .rotation-card {
    border-color: var(--accent-dim);
    background: linear-gradient(140deg, var(--accent-glow) 0%, var(--bg-2) 60%);
  }

  .active-card-top {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .active-card-name {
    font-family: var(--font-head);
    font-size: 12px;
    font-weight: 700;
    color: var(--text-0);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .active-card-meta {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .active-card-type {
    font-family: var(--font-head);
    font-size: 9px;
    letter-spacing: 0.06em;
    color: var(--accent-bright);
  }

  .active-card-sub {
    font-family: var(--font-body);
    font-size: 10px;
    color: var(--text-3);
  }

  .active-card-sub-event {
    font-family: var(--font-body);
    font-size: 10px;
    color: var(--text-2);
  }

  .active-card-gift {
    display: flex;
    align-items: center;
    gap: 4px;
    font-family: var(--font-head);
    font-size: 9px;
    letter-spacing: 0.06em;
    color: var(--text-2);
  }
  .active-card-gift svg { width: 10px; height: 10px; flex-shrink: 0; }

  .active-card-rule {
    font-family: var(--font-head);
    font-size: 9px;
    letter-spacing: 0.06em;
    color: var(--text-1);
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

  .def-path {
    font-family: var(--font-mono);
    font-size: 9px;
    color: var(--text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .def-badge {
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-3);
    border: 1px solid var(--border-mid);
    border-radius: 2px;
    padding: 1px 5px;
    flex-shrink: 0;
  }

  .def-edit-btn { flex-shrink: 0; }
</style>