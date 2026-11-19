<script lang="ts">
  import { onDestroy } from 'svelte'
  import Dropdown from './Dropdown.svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { ITEM_CATEGORIES } from '../lib/catalogMeta'
  import { STORE_SECTIONS, MIN_PRICE, MAX_PRICE, type NewItem } from '../lib/storeSimple'
  import type { PrototypeMatch } from '../lib/prototypes'

  export let serverExe: string
  /** Saves the item. Resolves with an error message, or '' once it's saved. */
  export let onAdd: (item: NewItem) => Promise<string>
  export let onClose: () => void

  // Bundles need a store page, which Simple doesn't make.
  const SECTIONS = STORE_SECTIONS.filter(s => s.type !== 'Bundle')
  const ALL_ITEMS = ITEM_CATEGORIES.find(c => c.ClassFilter === 'ItemPrototype')!

  let query = ''
  let results: PrototypeMatch[] = []
  let searching = false
  let searchError = ''
  let picked: PrototypeMatch | null = null

  let name = ''
  let section: string = 'Boost'
  let price = 100
  let quantity = 1
  let description = ''
  let saving = false
  let error = ''

  const label = (r: PrototypeMatch) => (r.display_name && r.display_name !== r.path ? r.display_name : r.leaf || r.path)

  let debounce: ReturnType<typeof setTimeout> | null = null
  let searchRun = 0
  function onQuery() {
    if (debounce) clearTimeout(debounce)
    debounce = setTimeout(search, 250)
  }
  async function search() {
    const run = ++searchRun
    searching = true
    searchError = ''
    try {
      const found = await invoke<PrototypeMatch[]>('search_prototypes', {
        serverExe,
        query,
        categoryPath: ALL_ITEMS.Path,
        isInventoryType: ALL_ITEMS.IsInventoryType,
        classFilter: ALL_ITEMS.ClassFilter,
        blueprintHint: ALL_ITEMS.Path,
      })
      if (run === searchRun) results = found
    } catch (e) {
      if (run === searchRun) searchError = String(e)
    } finally {
      if (run === searchRun) searching = false
    }
  }
  search()
  onDestroy(() => debounce && clearTimeout(debounce))

  function pick(r: PrototypeMatch) {
    // The name follows the picked item until you type your own.
    if (!name.trim() || (picked && name === label(picked))) name = label(r)
    picked = r
  }

  $: priceOk = Number.isInteger(price) && price >= MIN_PRICE && price <= MAX_PRICE
  $: quantityOk = Number.isInteger(quantity) && quantity >= 1
  $: ready = !!picked && !!name.trim() && priceOk && quantityOk && !saving

  async function add() {
    if (!picked || !ready) return
    saving = true
    error = await onAdd({ itemId: picked.prototype_id, name, section, price, quantity, description })
    saving = false
  }
</script>

<svelte:window on:keydown={(e) => e.key === 'Escape' && !saving && onClose()} />

<div class="modal-backdrop" role="presentation" on:click={() => !saving && onClose()}>
  <div class="modal" role="dialog" aria-modal="true" aria-labelledby="add-title" tabindex="-1" on:click|stopPropagation on:keydown|stopPropagation>
    <div class="modal-header">
      <div class="modal-title">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="12" y1="5" x2="12" y2="19"/>
          <line x1="5" y1="12" x2="19" y2="12"/>
        </svg>
        <span class="modal-display-name" id="add-title">Add an item</span>
      </div>
      <button class="close-btn" aria-label="Close" on:click={onClose} disabled={saving}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="18" y1="6" x2="6" y2="18"/>
          <line x1="6" y1="6" x2="18" y2="18"/>
        </svg>
      </button>
    </div>

    <div class="modal-body">
      <div class="field-group">
        <label class="field-label" for="add-search">Item</label>
        <div class="field-control">
          <input id="add-search" class="field-input" type="text" placeholder="Search items" bind:value={query} on:input={onQuery} spellcheck="false" autocomplete="off">
          <div class="results" role="listbox" aria-label="Items">
            {#if searchError}
              <div class="results-note error">{searchError}</div>
            {:else if searching && !results.length}
              <div class="results-note">Searching...</div>
            {:else if !results.length}
              <div class="results-note">No items match.</div>
            {:else}
              {#each results as r (r.prototype_id)}
                <button
                  class="result"
                  class:selected={picked?.prototype_id === r.prototype_id}
                  role="option"
                  aria-selected={picked?.prototype_id === r.prototype_id}
                  on:click={() => pick(r)}
                >{label(r)}</button>
              {/each}
            {/if}
          </div>
        </div>
      </div>

      <div class="field-group field-group-center">
        <label class="field-label" for="add-name">Name</label>
        <input id="add-name" class="field-input" type="text" placeholder="Shown in the store" bind:value={name} spellcheck="false" autocomplete="off">
      </div>

      <div class="field-group field-group-center">
        <label class="field-label" for="add-section">Section</label>
        <Dropdown id="add-section" label="Section" value={section} options={SECTIONS.map(s => ({ value: s.type, label: s.label }))}
          onChange={v => section = v} />
      </div>

      <div class="field-group field-group-center">
        <label class="field-label" for="add-price">Price</label>
        <div class="field-control field-inline">
          <input id="add-price" class="field-input num" type="number" min={MIN_PRICE} max={MAX_PRICE} step="1" bind:value={price}>
          <span class="field-hint">G</span>
        </div>
      </div>

      <div class="field-group field-group-center">
        <label class="field-label" for="add-qty">Quantity</label>
        <input id="add-qty" class="field-input num" type="number" min="1" step="1" bind:value={quantity}>
      </div>

      <div class="field-group field-group-center">
        <label class="field-label" for="add-desc">Description</label>
        <input id="add-desc" class="field-input" type="text" placeholder="Optional" bind:value={description} spellcheck="false" autocomplete="off">
      </div>
    </div>

    <div class="modal-footer">
      {#if error}<span class="error footer-error">{error}</span>{/if}
      <button class="btn btn-sm btn-outline" on:click={onClose} disabled={saving}>Cancel</button>
      <button class="btn btn-sm btn-accent" disabled={!ready} on:click={add}>{saving ? 'Adding...' : 'Add to store'}</button>
    </div>
  </div>
</div>

<style>
  .modal {
    width: min(560px, 92vw);
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
    padding-top: 2px;
  }
  .field-control {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  .field-inline { flex-direction: row; align-items: center; gap: 12px; }
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
    width: 100%;
    min-width: 0;
  }
  .field-input:focus { border-color: var(--accent-dim); }
  .field-input.num { width: 110px; font-variant-numeric: tabular-nums; }
  .results {
    height: 180px;
    overflow-y: auto;
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    background: var(--bg-0);
  }
  .results-note { padding: 8px 10px; font-size: 12px; color: var(--text-2); }
  .result {
    display: block;
    width: 100%;
    text-align: left;
    padding: 5px 10px;
    font-family: var(--font-body);
    font-size: 12px;
    color: var(--text-1);
    background: none;
    border: none;
    cursor: pointer;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .result:hover { background: var(--bg-2); color: var(--text-0); }
  .result.selected { background: var(--accent-glow); color: var(--accent-bright); }
  .error { font-size: 12px; color: var(--text-error); }
  .footer-error { margin-right: auto; }
</style>
