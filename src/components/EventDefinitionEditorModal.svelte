<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'

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

  interface PrototypeMatch {
    path: string
    blueprint: string
  }

  // ── Props ───────────────────────────────────────────────────────────────────

  export let definition: EventDefinition | null
  export let tuningFiles: TuningFileInfo[]
  export let serverExe: string
  export let onSave: (def: EventDefinition) => void | Promise<void>
  export let onClose: () => void

  // ── Local state ─────────────────────────────────────────────────────────────

  const isNew = definition === null

  let id:           string   = definition?.id           ?? ''
  let display_name: string   = definition?.display_name ?? ''
  let file_path:    string   = definition?.file_path    ?? ''
  let daily_gift:   string   = definition?.daily_gift   ?? ''
  let is_hidden:    boolean  = definition?.is_hidden    ?? false
  let missions:     string[] = [...(definition?.instanced_missions ?? [])]

  let newMission = ''
  let idError    = ''
  let nameError  = ''

  // Prototype search
  let protoSuggestOpen  = false
  let protoSearchResults: PrototypeMatch[] = []
  let protoSearching    = false
  let protoSearchError  = ''
  let protoDebounceTimer: ReturnType<typeof setTimeout> | null = null
  let activeProtoTarget: 'gift' | 'mission' | null = null
  let protoDropdownStyle = ''
  let giftInputEl:    HTMLInputElement
  let missionInputEl: HTMLInputElement

  // ── Derived ─────────────────────────────────────────────────────────────────

  $: fileOptions = tuningFiles

  $: autoEnabledFile = tuningFiles.find(
    f => f.relative_path === file_path && f.was_auto_enabled
  )

  // ── Prototype search ─────────────────────────────────────────────────────────

  async function searchPrototypes(query: string) {
    if (query.length < 2) {
      protoSuggestOpen = false
      protoSearchResults = []
      return
    }
    protoSearching   = true
    protoSearchError = ''
    try {
      protoSearchResults = await invoke<PrototypeMatch[]>('search_prototypes', {
        serverExe,
        query,
      })
      protoSuggestOpen = protoSearchResults.length > 0
    } catch (e) {
      protoSearchError = String(e)
      protoSuggestOpen = false
    } finally {
      protoSearching = false
    }
  }

  function onProtoInput(target: 'gift' | 'mission', value: string, inputEl: HTMLInputElement) {
    activeProtoTarget = target
    if (protoDebounceTimer) clearTimeout(protoDebounceTimer)
    if (value.length < 2) {
      protoSuggestOpen = false
      return
    }
    const rect         = inputEl.getBoundingClientRect()
    protoDropdownStyle = `top:${rect.bottom + 3}px;left:${rect.left}px;width:${rect.width}px;`
    protoDebounceTimer = setTimeout(() => searchPrototypes(value), 200)
  }

  function selectPrototype(path: string) {
    if (activeProtoTarget === 'gift') {
      daily_gift = path
    } else if (activeProtoTarget === 'mission') {
      newMission = path
      addMission()
    }
    protoSuggestOpen  = false
    activeProtoTarget = null
  }

  function onWindowClick(e: MouseEvent) {
    if (protoSuggestOpen && !(e.target as Element).closest('.proto-dropdown')) {
      protoSuggestOpen = false
    }
  }

  // ── Helpers ─────────────────────────────────────────────────────────────────

  function addMission() {
    const m = newMission.trim()
    if (!m || missions.includes(m)) return
    missions   = [...missions, m]
    newMission = ''
    protoSuggestOpen = false
  }

  function removeMission(m: string) {
    missions = missions.filter(x => x !== m)
  }

  function handleSave() {
    idError   = ''
    nameError = ''

    if (!id.trim())           { idError   = 'ID is required.';           return }
    if (!display_name.trim()) { nameError = 'Display name is required.'; return }

    onSave({
      id:                  id.trim(),
      display_name:        display_name.trim(),
      file_path,
      daily_gift:          daily_gift.trim() || null,
      instanced_missions:  missions.length > 0 ? missions : null,
      is_hidden:           is_hidden || null,
    })
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      if (protoSuggestOpen) { protoSuggestOpen = false; return }
      onClose()
    }
  }
</script>

<svelte:window on:keydown={handleKeydown} on:click={onWindowClick} />

<div
  class="modal-backdrop"
  role="presentation"
  on:click={onClose}
>
  <div
    class="modal-card"
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    on:click|stopPropagation
    on:keydown|stopPropagation
  >
    <!-- Header -->
    <div class="modal-header">
      <span class="modal-title">{isNew ? 'New Event Definition' : 'Edit Event Definition'}</span>
      <button class="btn-icon" on:click={onClose} title="Close">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="18" y1="6" x2="6" y2="18"/>
          <line x1="6" y1="6" x2="18" y2="18"/>
        </svg>
      </button>
    </div>

    <!-- Body -->
    <div class="modal-body">

      <!-- ID -->
      <div class="field-group">
        <label class="field-label" for="def-id">ID</label>
        <div class="field-control">
          {#if isNew}
            <input
              id="def-id"
              class="field-input"
              class:input-error={!!idError}
              type="text"
              bind:value={id}
              placeholder="e.g. CosmicChaos"
            />
          {:else}
            <div class="field-readonly">{id}</div>
            <span class="field-hint">ID cannot be changed — it is referenced by schedule rules.</span>
          {/if}
          {#if idError}
            <span class="field-error">{idError}</span>
          {/if}
        </div>
      </div>

      <!-- Display name -->
      <div class="field-group">
        <label class="field-label" for="def-name">Display name</label>
        <div class="field-control">
          <input
            id="def-name"
            class="field-input"
            class:input-error={!!nameError}
            type="text"
            bind:value={display_name}
            placeholder="e.g. Cosmic Chaos"
          />
          {#if nameError}
            <span class="field-error">{nameError}</span>
          {/if}
        </div>
      </div>

      <!-- File path -->
      <div class="field-group">
        <label class="field-label" for="def-filepath">Tuning file</label>
        <div class="field-control">
          <select id="def-filepath" class="field-select" bind:value={file_path}>
            <option value="">— None —</option>
            {#each fileOptions as f}
              <option value={f.relative_path}>{f.canonical_name}</option>
            {/each}
          </select>
          {#if file_path}
            <span class="field-path">{file_path}</span>
          {/if}
          {#if autoEnabledFile}
            <div class="auto-enabled-notice">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <circle cx="12" cy="12" r="10"/>
                <line x1="12" y1="8" x2="12" y2="12"/>
                <line x1="12" y1="16" x2="12.01" y2="16"/>
              </svg>
              {autoEnabledFile.canonical_name} was off — now enabled
            </div>
          {/if}
        </div>
      </div>

      <!-- Daily gift -->
      <div class="field-group">
        <label class="field-label" for="def-gift">Daily gift</label>
        <div class="field-control">
          <input
            id="def-gift"
            class="field-input"
            type="text"
            bind:this={giftInputEl}
            bind:value={daily_gift}
            placeholder="Prototype path — type to search"
            autocomplete="off"
            spellcheck="false"
            on:input={() => onProtoInput('gift', daily_gift, giftInputEl)}
            on:focus={() => { if (daily_gift.length >= 2) onProtoInput('gift', daily_gift, giftInputEl) }}
          />
          {#if protoSearching && activeProtoTarget === 'gift'}
            <div class="proto-hint">Searching…</div>
          {:else if protoSearchError && activeProtoTarget === 'gift'}
            <div class="proto-hint error">{protoSearchError}</div>
          {/if}
        </div>
      </div>

      <!-- Is hidden -->
      <div class="field-group">
        <span class="field-label">Hidden</span>
        <div class="field-control field-row">
          <button
            class="editor-toggle"
            class:on={is_hidden}
            role="switch"
            aria-checked={is_hidden}
            aria-label="Is Hidden"
            on:click={() => is_hidden = !is_hidden}
          ></button>
          <span class="toggle-hint">{is_hidden ? 'Hidden from public schedule' : 'Visible'}</span>
        </div>
      </div>

      <!-- Instanced missions -->
      <div class="field-group field-group-tall">
        <span class="field-label">Instanced missions</span>
        <div class="field-control">
          {#if missions.length > 0}
            <div class="missions-list">
              {#each missions as m}
                <div class="mission-row">
                  <span class="mission-path">{m}</span>
                  <button class="remove-btn" title="Remove" on:click={() => removeMission(m)}>
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <line x1="18" y1="6" x2="6" y2="18"/>
                      <line x1="6" y1="6" x2="18" y2="18"/>
                    </svg>
                  </button>
                </div>
              {/each}
            </div>
          {/if}
          <div class="add-mission-row">
            <input
              class="field-input"
              type="text"
              bind:this={missionInputEl}
              bind:value={newMission}
              placeholder="Prototype path — type to search"
              autocomplete="off"
              spellcheck="false"
              on:input={() => onProtoInput('mission', newMission, missionInputEl)}
              on:focus={() => { if (newMission.length >= 2) onProtoInput('mission', newMission, missionInputEl) }}
              on:keydown={e => e.key === 'Enter' && addMission()}
            />
            <button
              class="btn btn-sm btn-outline"
              disabled={!newMission.trim()}
              on:click={addMission}
            >Add</button>
          </div>
          {#if protoSearching && activeProtoTarget === 'mission'}
            <div class="proto-hint">Searching…</div>
          {:else if protoSearchError && activeProtoTarget === 'mission'}
            <div class="proto-hint error">{protoSearchError}</div>
          {/if}
        </div>
      </div>

    </div><!-- modal-body -->

    <!-- Footer -->
    <div class="modal-footer">
      <button class="btn btn-sm btn-outline" on:click={onClose}>Cancel</button>
      <button class="btn btn-sm btn-accent" on:click={handleSave}>
        {isNew ? 'Create' : 'Save'}
      </button>
    </div>

  </div>
</div>

<!-- Prototype search dropdown — rendered outside modal-card so it can overlay freely -->
{#if protoSuggestOpen}
  <div class="proto-dropdown" style={protoDropdownStyle}>
    {#each protoSearchResults as result}
      <button
        class="proto-option"
        type="button"
        on:mousedown|preventDefault={() => selectPrototype(result.path)}
      >
        <span class="proto-opt-path">{result.path}</span>
        <span class="proto-opt-bp">{result.blueprint}</span>
      </button>
    {/each}
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: var(--z-modal);
    backdrop-filter: blur(2px);
  }

  .modal-card {
    background: var(--bg-2);
    border: 1px solid var(--border-lit);
    border-radius: var(--radius-md);
    width: min(560px, 90vw);
    max-height: 85vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 24px 80px rgba(0, 0, 0, 0.55);
  }

  /* ── Header ── */

  .modal-header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 14px 16px 12px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .modal-title {
    font-family: var(--font-head);
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-1);
    flex: 1;
  }

  /* ── Body ── */

  .modal-body {
    flex: 1;
    overflow-y: auto;
    padding: 4px 0;
  }

  .field-group {
    display: grid;
    grid-template-columns: 130px 1fr;
    align-items: start;
    gap: 10px;
    padding: 10px 16px;
    border-bottom: 1px solid var(--border);
  }
  .field-group:last-child { border-bottom: none; }
  .field-group-tall       { align-items: start; }

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
  }
  .field-select:focus { border-color: var(--accent-dim); }

  .field-readonly {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-1);
    padding: 6px 0;
  }

  .field-hint {
    font-family: var(--font-body);
    font-size: 10px;
    color: var(--text-3);
  }

  .field-path {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-3);
  }

  .field-error {
    font-family: var(--font-body);
    font-size: 11px;
    color: var(--text-error);
  }

  /* ── Auto-enabled notice ── */

  .auto-enabled-notice {
    display: flex;
    align-items: center;
    gap: 6px;
    font-family: var(--font-body);
    font-size: 10px;
    color: var(--accent-bright);
    background: var(--accent-glow);
    border: 1px solid var(--accent-dim);
    border-radius: var(--radius-sm);
    padding: 4px 8px;
    margin-top: 2px;
  }
  .auto-enabled-notice svg { width: 12px; height: 12px; flex-shrink: 0; }

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
  .editor-toggle.on         { background: var(--accent-glow-strong); border-color: var(--accent-dim); }
  .editor-toggle.on::after  { left: 16px; background: var(--accent-bright); }

  .toggle-hint {
    font-family: var(--font-head);
    font-size: 10px;
    letter-spacing: 0.06em;
    color: var(--text-3);
  }

  /* ── Missions list ── */

  .missions-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-bottom: 6px;
  }

  .mission-row {
    display: flex;
    align-items: center;
    gap: 6px;
    background: var(--bg-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 5px 8px;
  }

  .mission-path {
    flex: 1;
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-2);
    word-break: break-all;
    line-height: 1.4;
  }

  .add-mission-row {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .add-mission-row .field-input { flex: 1; }

  .remove-btn {
    width: 20px;
    height: 20px;
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
  .remove-btn:hover { color: var(--text-error); border-color: rgba(220,60,60,0.3); }
  .remove-btn svg   { width: 12px; height: 12px; }

  /* ── Prototype search hint ── */

  .proto-hint {
    font-family: var(--font-body);
    font-size: 10px;
    color: var(--text-3);
  }
  .proto-hint.error { color: var(--text-error); }

  /* ── Prototype search dropdown ── */

  .proto-dropdown {
    position: fixed;
    background: var(--bg-2);
    border: 1px solid var(--border-lit);
    border-radius: var(--radius-sm);
    z-index: calc(var(--z-modal) + 10);
    max-height: 280px;
    overflow-y: auto;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
  }

  .proto-option {
    display: flex;
    flex-direction: column;
    gap: 1px;
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    border-bottom: 1px solid var(--border);
    padding: 6px 10px;
    cursor: pointer;
    transition: background 0.08s;
  }
  .proto-option:last-child { border-bottom: none; }
  .proto-option:hover      { background: var(--bg-3); }

  .proto-opt-path {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-0);
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
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 10px 16px;
    border-top: 1px solid var(--border);
    flex-shrink: 0;
    background: var(--chrome-sunken-bg);
  }
</style>