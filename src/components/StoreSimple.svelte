<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { openPath } from '@tauri-apps/plugin-opener'
  import { reloadCatalog } from '../lib/serverReload'
  import { appConfig, serverRunning } from '../lib/store'
  import { tip } from '../lib/tooltip'
  import PanelSidebar from './PanelSidebar.svelte'
  import SaleModal from './SaleModal.svelte'
  import AddItemModal from './AddItemModal.svelte'
  import { entryTitle, entryPrice, type CatalogEntryWithMeta } from '../lib/catalogMeta'
  import {
    STORE_SECTIONS, MIN_PRICE, MAX_PRICE, isHidden, setHidden, setPrice, sameEntry, planWrites, reverted,
    displayTitle, startSale, newStoreItem, ADDED_ITEMS_FILE, type NewItem, endSale, type StoreState, type StoreWrites,
  } from '../lib/storeSimple'

  type View = string // a section's Type name, 'all' or 'yours'
  const SECTION_TYPES: string[] = STORE_SECTIONS.map(s => s.type)
  const sectionLabel = (type: string) => STORE_SECTIONS.find(s => s.type === type)?.label ?? (type || 'Other')

  let saved: CatalogEntryWithMeta[] = []
  /** Unsaved edits by SKU. */
  let edits: Record<string, CatalogEntryWithMeta> = {}
  let loaded = false
  let loadError = ''
  let saving = false
  let saveError = ''
  let folderError = ''
  let view: View = 'all'
  let query = ''
  let state: StoreState = { version: 1 }
  let saleOpen = false
  let addOpen = false
  let saleNote = ''

  $: serverExe = $appConfig.server_exe
  $: if (serverExe) load(serverExe)

  async function load(exe: string) {
    loadError = ''
    try {
      const [entries, stored] = await Promise.all([
        invoke<CatalogEntryWithMeta[]>('load_catalog_entries', { serverExe: exe }),
        invoke<StoreState | null>('load_store_state', { serverExe: exe }),
      ])
      saved = entries
      state = stored ?? { version: 1 }
      edits = {}
      loaded = true
    } catch (e) {
      loadError = String(e)
    }
  }

  const isYours = (e: CatalogEntryWithMeta) => (e.stock ? !sameEntry(e, e.stock) : e.from_modified)

  $: rows = saved.map(e => {
    const now = edits[e.SkuId] ?? e
    const changed = !!edits[e.SkuId] && !sameEntry(now, e)
    return { saved: e, now, changed, yours: isYours(e), title: displayTitle(entryTitle(e)) }
  })
  // Hidden stock items stay out of the list; anything you've changed stays in it.
  $: listed = rows.filter(r => r.changed || r.yours || (SECTION_TYPES.includes(r.saved.Type.Name) && !isHidden(r.saved)))
  $: needle = query.trim().toLowerCase()
  $: shown = listed
    .filter(r => view === 'all' ? true : view === 'yours' ? r.yours || r.changed : r.saved.Type.Name === view)
    .filter(r => !needle || r.title.toLowerCase().includes(needle))
    .sort((a, b) => a.title.localeCompare(b.title))
  $: dirty = rows.filter(r => r.changed)

  function edit(e: CatalogEntryWithMeta, next: CatalogEntryWithMeta) {
    edits = { ...edits, [e.SkuId]: next }
    saveError = ''
  }

  function changePrice(r: typeof rows[number], box: HTMLInputElement) {
    const n = Math.round(box.valueAsNumber)
    if (Number.isFinite(n)) edit(r.saved, setPrice(r.now, Math.min(MAX_PRICE, Math.max(MIN_PRICE, n))))
    box.value = String(entryPrice(edits[r.saved.SkuId] ?? r.now)) // show the clamped value
  }

  function revert(r: typeof rows[number]) {
    if (r.changed) {
      const { [r.saved.SkuId]: _, ...rest } = edits
      edits = rest
    } else {
      const stock = reverted(r.saved)
      if (stock) edit(r.saved, stock)
    }
  }

  /** Writes catalog changes, plus the sale record when given, then reloads the list. */
  async function write({ upserts, removes }: StoreWrites, nextState: StoreState | null) {
    if (!serverExe) return false
    saving = true
    saveError = ''
    try {
      await invoke('apply_store_changes', { serverExe, upserts, removes, state: nextState })
      reloadRun++
      reloadError = ''
      await load(serverExe)
      return true
    } catch (e) {
      saveError = String(e)
      return false
    } finally {
      saving = false
    }
  }

  async function save() {
    saleNote = ''
    await write(planWrites(dirty.map(r => r.now)), null)
  }

  // A sale is written at once, together with its record, so End sale always knows what it changed.
  async function beginSale(percent: number, sections: string[]) {
    saleOpen = false
    saleNote = ''
    const { edited, sale } = startSale(saved, percent, sections, new Date().toISOString())
    await write(planWrites(edited), { ...state, sale })
  }

  async function finishSale() {
    if (!state.sale) return
    const { edited, kept } = endSale(saved, state.sale)
    const { sale: _, ...rest } = state
    if (await write(planWrites(edited), rest) && kept.length) {
      saleNote = `${kept.length} ${kept.length === 1 ? 'price was' : 'prices were'} changed during the sale and kept.`
    }
  }

  /** Creates the Multiverse catalog file on first use, then saves the item under the next free SKU. */
  async function addItem(item: NewItem): Promise<string> {
    if (!serverExe) return 'No server set'
    try {
      const files = await invoke<string[]>('list_catalog_files', { serverExe })
      if (!files.includes(ADDED_ITEMS_FILE)) await invoke('create_catalog_file', { serverExe, suffix: 'Multiverse' })
      const sku = await invoke<string>('get_next_sku_id', { serverExe })
      saleNote = ''
      if (!(await write(planWrites([newStoreItem(sku, item)]), null))) return saveError
      addOpen = false
      return ''
    } catch (e) {
      return String(e)
    }
  }

  // Items stock doesn't have can't be reverted, only removed. The first click arms the button.
  let armedRemove = ''
  async function removeAdded(r: typeof rows[number]) {
    if (armedRemove !== r.saved.SkuId) {
      armedRemove = r.saved.SkuId
      return
    }
    armedRemove = ''
    saleNote = ''
    await write({ upserts: [], removes: [{ sku_id: r.saved.SkuId, source_file: r.saved.source_file }] }, null)
  }

  $: onSale = (r: typeof rows[number]) => state.sale?.prices[r.saved.SkuId]?.after === entryPrice(r.saved)

  async function openFolder() {
    folderError = ''
    try {
      await openPath(await invoke<string>('get_mtxstore_dir', { serverExe }))
    } catch (e) {
      folderError = String(e)
    }
  }

  // ── Reload ────────────────────────────────────────────────────────────────
  // Only a failure shows a message. A newer reload or a save drops an older result.

  let reloadError = ''
  let reloadRun = 0

  async function reload() {
    const run = ++reloadRun
    reloadError = ''
    const error = await reloadCatalog()
    if (run === reloadRun) reloadError = error
  }
</script>

<div class="store">
  <div class="panel-bg"></div>
  <div class="grid-overlay"></div>
  <div class="store-layout">
    <PanelSidebar width="var(--sidebar-narrow)">
      <svelte:fragment slot="header">
        <div class="section-title">Sections</div>
        <button
          class="btn-icon"
          style="margin-left:auto;"
          on:click={openFolder}
          disabled={!serverExe}
          aria-label="Open MTXStore folder"
          use:tip={'Open MTXStore folder'}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
          </svg>
        </button>
      </svelte:fragment>

      <div class="store-side">
        <button class="side-item" class:active={view === 'all'} on:click={() => view = 'all'}><span class="side-name">All</span></button>
        {#each STORE_SECTIONS as s}
          <button class="side-item" class:active={view === s.type} on:click={() => view = s.type}><span class="side-name">{s.label}</span></button>
        {/each}
        <div class="side-label">Changes</div>
        <button class="side-item" class:active={view === 'yours'} on:click={() => view = 'yours'}><span class="side-name">Yours</span></button>
      </div>
    </PanelSidebar>

    <section class="data-main">
      <header class="page-head">
        <h3 class="page-title">{view === 'all' ? 'Store' : view === 'yours' ? 'Yours' : sectionLabel(view)}</h3>
        <div class="page-acts">
          {#if reloadError}<span class="error">{reloadError}</span>{/if}
          <div class="search-wrap store-search">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="11" cy="11" r="8"/>
              <line x1="21" y1="21" x2="16.65" y2="16.65"/>
            </svg>
            <input type="text" class="search-input" placeholder="Search names" aria-label="Search names" bind:value={query}>
          </div>
          <button
            class="btn btn-sm btn-outline"
            on:click={() => addOpen = true}
            disabled={!loaded || saving || dirty.length > 0}
            use:tip={dirty.length ? 'Save or revert your changes first' : 'Add a single item to the store'}
          >Add item</button>
          {#if !state.sale}
            <button
              class="btn btn-sm btn-outline"
              on:click={() => saleOpen = true}
              disabled={!loaded || saving || dirty.length > 0}
              use:tip={dirty.length ? 'Save or revert your changes first' : 'Lower prices by a percentage'}
            >Start sale</button>
          {/if}
          <button
            class="btn btn-sm btn-outline"
            on:click={reload}
            disabled={!$serverRunning}
            use:tip={$serverRunning ? 'Send !server reloadcatalog. Players see changes next time they open the store.' : 'Server not running'}
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="23 4 23 10 17 10"/>
              <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/>
            </svg>
            Reload Store
          </button>
        </div>
      </header>

      <div class="store-body">
        {#if !serverExe}
          <p class="store-msg">Set the server location first.</p>
        {:else if loadError}
          <p class="store-msg error">{loadError}</p>
        {:else if loaded && shown.length === 0}
          <p class="store-msg">{view === 'yours' && !needle ? "You haven't changed anything in the store." : 'Nothing matches.'}</p>
        {:else if loaded}
          {#if state.sale}
            <div class="sale-line">
              <span class="sale-now">On sale: {state.sale.label}</span>
              <button
                class="btn btn-sm btn-outline"
                on:click={finishSale}
                disabled={saving || dirty.length > 0}
                use:tip={dirty.length ? 'Save or revert your changes first' : 'Put the sale prices back'}
              >End sale</button>
            </div>
          {/if}
          {#each shown as r (r.saved.SkuId)}
            {@const hidden = isHidden(r.now)}
            {@const canRevert = r.changed || (r.yours && !!r.saved.stock)}
            <div class="item-row" class:off={hidden}>
              <span class="item-name">
                <span class="item-title">{r.title}</span>
                {#if onSale(r)}<span class="tag" use:tip={`Was ${state.sale?.prices[r.saved.SkuId].before.toLocaleString()} G`}>Sale</span>{/if}
              </span>
              {#if r.changed}
                <span class="item-badge changed">Changed</span>
              {:else if r.yours}
                <span class="item-badge yours">Yours</span>
              {:else}
                <span class="item-badge">Default</span>
              {/if}

              <span class="item-section">
                {#if view === 'all' || view === 'yours'}<span class="tag">{sectionLabel(r.saved.Type.Name)}</span>{/if}
              </span>

              <span class="item-price">
                <input
                  type="number"
                  aria-label="{r.title} price"
                  min={MIN_PRICE}
                  max={MAX_PRICE}
                  step="1"
                  value={entryPrice(r.now)}
                  on:change={(e) => changePrice(r, e.currentTarget)}
                >
                <span class="item-unit">G</span>
              </span>

              <span class="item-show">
                <div
                  class="toggle-switch"
                  class:on={!hidden}
                  role="switch"
                  aria-checked={!hidden}
                  aria-label="Show {r.title} in the store"
                  tabindex="0"
                  on:click={() => edit(r.saved, setHidden(r.now, !hidden))}
                  on:keydown={(e) => e.key === 'Enter' && edit(r.saved, setHidden(r.now, !hidden))}
                ></div>
                <span class="item-shown">{hidden ? 'Hidden' : 'Shown'}</span>
              </span>

              <span class="item-revert">
                {#if canRevert}
                  <button
                    class="btn-icon"
                    on:click={() => revert(r)}
                    aria-label="Revert {r.title}"
                    use:tip={r.changed ? 'Undo this change' : `Revert to stock (${entryPrice(r.saved.stock ?? r.saved).toLocaleString()} G)`}
                  >
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M3 12a9 9 0 1 0 3-6.7"/>
                      <polyline points="3 3 3 9 9 9"/>
                    </svg>
                  </button>
                {:else if !r.saved.stock && r.saved.from_modified}
                  <button
                    class="del-btn"
                    class:armed={armedRemove === r.saved.SkuId}
                    on:click={() => removeAdded(r)}
                    on:mouseleave={() => armedRemove === r.saved.SkuId && (armedRemove = '')}
                    disabled={saving || dirty.length > 0}
                    aria-label="{armedRemove === r.saved.SkuId ? 'Confirm removing' : 'Remove'} {r.title}"
                    use:tip={dirty.length ? 'Save or revert your changes first' : armedRemove === r.saved.SkuId ? 'Click again to remove. This cannot be undone.' : 'Remove from the store'}
                  >
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                      <line x1="18" y1="6" x2="6" y2="18"/>
                      <line x1="6" y1="6" x2="18" y2="18"/>
                    </svg>
                  </button>
                {/if}
              </span>
            </div>
          {/each}
        {/if}
      </div>

      <footer class="panel-footer">
        <div class="store-status">
          {#if saveError}<span class="error">{saveError}</span>
          {:else if folderError}<span class="error">{folderError}</span>
          {:else if dirty.length}<span class="dirty-badge">{dirty.length} unsaved {dirty.length === 1 ? 'change' : 'changes'}</span>
          {:else if saleNote}<span>{saleNote}</span>{/if}
        </div>
        <button
          class="btn btn-sm btn-accent"
          on:click={save}
          use:tip={'Saves to the catalog MODIFIED files'}
          disabled={saving || dirty.length === 0}
        >{saving ? 'Saving...' : 'Save'}</button>
      </footer>
    </section>
  </div>
</div>

{#if addOpen && serverExe}
  <AddItemModal {serverExe} onAdd={addItem} onClose={() => addOpen = false} />
{/if}

{#if saleOpen}
  <SaleModal
    countFor={(percent, sections) => startSale(saved, percent, sections, '').edited.length}
    onStart={beginSale}
    onClose={() => saleOpen = false}
  />
{/if}

<style>
  .store {
    display: flex;
    flex: 1;
    position: relative;
    overflow: hidden;
    min-height: 0;
  }
  .store-layout {
    position: relative;
    z-index: 1;
    flex: 1;
    display: flex;
    min-height: 0;
  }
  .store-side { padding: 6px; }
  .store-search { flex: 0 1 200px; min-width: 120px; }
  .store-body {
    flex: 1;
    overflow-y: auto;
    padding: 4px 20px 22px;
  }
  .store-msg { font-size: 12px; color: var(--text-2); }
  .error { font-size: 12px; color: var(--text-error); }
  .store-status {
    flex: 1;
    min-width: 0;
    font-size: 12px;
    color: var(--text-1);
  }

  /* Name over badge on the left, then section, price, shown, revert. */
  .item-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 92px 104px 96px 28px;
    column-gap: 16px;
    row-gap: 3px;
    align-items: center;
    padding: 8px 0;
    border-bottom: 1px solid var(--border);
  }
  .sale-line {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 0;
    border-bottom: 1px solid var(--border);
    font-size: 12px;
  }
  .sale-now { color: var(--accent); }
  .item-name {
    grid-column: 1;
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
  }
  .item-title {
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
  .item-badge {
    grid-column: 1;
    font-family: var(--font-head);
    font-size: var(--fs-min);
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
    white-space: nowrap;
  }
  .item-badge.yours { color: var(--accent-bright); }
  .item-badge.changed { color: var(--amber-bright); }
  .item-section,
  .item-price,
  .item-show,
  .item-revert { grid-row: 1 / span 2; }
  .item-section { grid-column: 2; }
  .item-price {
    grid-column: 3;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 30px;
    padding: 0 8px 0 4px;
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    background: var(--bg-0);
  }
  .item-price:focus-within { border-color: var(--accent-dim); box-shadow: 0 0 0 2px var(--accent-glow); }
  .item-price input {
    flex: 1;
    min-width: 0;
    padding: 0;
    border: 0;
    background: transparent;
    color: var(--text-0);
    font-family: var(--font-body);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    text-align: right;
    outline: none;
  }
  .item-unit { font-size: 12px; color: var(--text-3); }
  .item-show {
    grid-column: 4;
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .item-shown { font-size: 12px; color: var(--text-1); }
  .item-revert {
    grid-column: 5;
    display: flex;
    justify-content: center;
  }
  .del-btn.armed { border-color: color-mix(in srgb, var(--red) 40%, transparent); color: var(--text-error); background: var(--red-dim); }
  .del-btn:disabled { opacity: 0.35; cursor: not-allowed; }
  .item-row.off .item-title,
  .item-row.off .item-price { opacity: var(--dimmed-opacity); }
</style>
