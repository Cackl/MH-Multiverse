<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'

  export let serverExe: string
  /** Called with the new file's name once it exists. */
  export let onCreated: (name: string) => void
  export let onClose: () => void

  let suffix = ''
  let error = ''
  let creating = false

  async function create() {
    if (!suffix.trim() || creating) return
    creating = true
    error = ''
    try {
      onCreated(await invoke<string>('create_catalog_file', { serverExe, suffix: suffix.trim() }))
    } catch (e) {
      error = String(e)
    } finally {
      creating = false
    }
  }
</script>

<svelte:window on:keydown={(e) => e.key === 'Escape' && onClose()} />

<div class="modal-backdrop" role="presentation" on:click={onClose}>
  <div class="modal" role="dialog" aria-modal="true" aria-labelledby="new-file-title" tabindex="-1" on:click|stopPropagation on:keydown|stopPropagation>
    <div class="modal-header">
      <div class="modal-title">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
          <polyline points="14 2 14 8 20 8"/>
          <line x1="12" y1="12" x2="12" y2="18"/>
          <line x1="9" y1="15" x2="15" y2="15"/>
        </svg>
        <span class="modal-display-name" id="new-file-title">New catalog file</span>
      </div>
      <button class="close-btn" aria-label="Close" on:click={onClose}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="18" y1="6" x2="6" y2="18"/>
          <line x1="6" y1="6" x2="18" y2="18"/>
        </svg>
      </button>
    </div>

    <div class="modal-body">
      <label class="field-label" for="new-file-name">File name</label>
      <div class="filename-compose" class:has-error={!!error}>
        <span class="filename-fixed">Catalog</span>
        <!-- svelte-ignore a11y-autofocus -->
        <input
          id="new-file-name"
          type="text"
          class="filename-input"
          placeholder="MyItems"
          bind:value={suffix}
          on:input={() => error = ''}
          on:keydown={e => e.key === 'Enter' && create()}
          spellcheck="false"
          autocomplete="off"
          autofocus
        >
        <span class="filename-fixed">.json</span>
      </div>
      {#if error}
        <span class="filename-error">{error}</span>
      {:else}
        <span class="field-hint">Letters, numbers, _ or -. Add items to it with New entry.</span>
      {/if}
    </div>

    <div class="modal-footer">
      <button class="btn btn-sm btn-outline" on:click={onClose}>Cancel</button>
      <button class="btn btn-sm btn-accent" disabled={!suffix.trim() || creating} on:click={create}>Create</button>
    </div>
  </div>
</div>

<style>
  .modal { width: min(420px, 92vw); }
  .modal-body {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 16px;
  }
  .field-label {
    font-family: var(--font-head);
    font-size: var(--fs-min);
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-2);
  }
  .field-hint { font-size: 12px; line-height: 1.45; color: var(--text-2); }
  .filename-error { font-size: 12px; line-height: 1.45; color: var(--text-error); }
  .filename-compose {
    display: flex;
    align-items: center;
    background: var(--bg-0);
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    overflow: hidden;
    transition: border-color 0.15s;
  }
  .filename-compose:focus-within { border-color: var(--accent-dim); }
  .filename-compose.has-error { border-color: var(--red); }
  .filename-fixed {
    font-size: 12px;
    color: var(--text-3);
    padding: 6px 8px;
    white-space: nowrap;
    background: var(--bg-2);
    user-select: none;
    flex-shrink: 0;
  }
  .filename-input {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: none;
    outline: none;
    box-shadow: none;
    color: var(--text-0);
    font-family: var(--font-body);
    font-size: 12px;
    padding: 6px 8px;
  }
</style>
