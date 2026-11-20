<script lang="ts">
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { openPath } from '@tauri-apps/plugin-opener'
  import { appConfig, serverRunning } from '../../../lib/store'
  import { reloadCatalog } from '../../../lib/serverReload'
  import {
    typeNames,
    modifiersForType,
    buildModifiers,
    entryTitle,
    entryPrice,
    removeKind,
    stockVersion,
    type CatalogEntryWithMeta,
  } from '../../../lib/catalogMeta'
  import PanelSidebar from '../../PanelSidebar.svelte'
  import Dropdown from '../../Dropdown.svelte'
  import StoreEditorModal from './StoreEditorModal.svelte'
  import NewFileModal from '../NewFileModal.svelte'
  import { tip } from '../../../lib/tooltip'

  // ── State ──────────────────────────────────────────────────────────────────

  let entries: CatalogEntryWithMeta[] = []
  let catalogFiles: string[] = []
  /** Files switched off: they sit in MTXStore\Off, which the server doesn't read. */
  let offFiles: string[] = []
  let switchError = ''
  let creatingFile = false
  let loading = false
  let loadError = ''
  let openDirError = ''
  // Only a failed reload shows a message. A newer reload drops an older result.
  let reloadError = ''
  let reloadRun = 0
  async function reload() {
    const run = ++reloadRun
    reloadError = ''
    const error = await reloadCatalog()
    if (run === reloadRun) reloadError = error
  }

  // Filters — price uses strings to avoid Svelte's numeric coercion on empty inputs
  let fileFilter   = ''
  let typeFilter   = ''
  let searchQuery  = ''
  let priceMinStr  = ''
  let priceMaxStr  = ''
  let modifiedOnly = false

  // Sort
  let sortCol: 'sku' | 'name' | 'type' | 'price' = 'name'
  let sortDir: 'asc' | 'desc' = 'asc'

  // Selection
  let selectedSkus = new Set<string>()
  let headerCheckEl: HTMLInputElement

  // Modals
  let editingEntry: CatalogEntryWithMeta | null = null
  let creatingNew = false

  // Batch operations
  type BatchMode = null | 'price' | 'add-mod' | 'remove-mod'
  let batchMode: BatchMode = null
  let batchPriceStr = ''
  let batchModifier = ''
  let batchRunning  = false
  let batchError    = ''
  let batchSuccess  = ''
  /** Which removal is waiting for Confirm. */
  let batchConfirm: null | 'revert' | 'delete' = null

  // ── Derived: filter pipeline ───────────────────────────────────────────────

  $: priceMinN = priceMinStr === '' ? null : Number(priceMinStr)
  $: priceMaxN = priceMaxStr === '' ? null : Number(priceMaxStr)
  $: searchLower = searchQuery.toLowerCase()
  $: anyFilterActive = fileFilter !== '' || typeFilter !== '' || searchQuery !== '' ||
    priceMinStr !== '' || priceMaxStr !== '' || modifiedOnly

  $: filteredEntries = entries.filter(e => {
    if (fileFilter && e.source_file !== fileFilter) return false
    if (typeFilter && e.Type.Name !== typeFilter) return false
    if (modifiedOnly && !e.from_modified) return false
    if (priceMinN !== null && !isNaN(priceMinN) && entryPrice(e) < priceMinN) return false
    if (priceMaxN !== null && !isNaN(priceMaxN) && entryPrice(e) > priceMaxN) return false
    if (searchLower) {
      const title = entryTitle(e).toLowerCase()
      const sku = e.SkuId
      return title.includes(searchLower) || sku.includes(searchLower)
    }
    return true
  })

  $: sortedEntries = [...filteredEntries].sort((a, b) => {
    let cmp = 0
    switch (sortCol) {
      case 'sku': {
        const ai = BigInt(a.SkuId)
        const bi = BigInt(b.SkuId)
        cmp = ai < bi ? -1 : ai > bi ? 1 : 0
        break
      }
      case 'name':  cmp = entryTitle(a).localeCompare(entryTitle(b)); break
      case 'type':  cmp = a.Type.Name.localeCompare(b.Type.Name); break
      case 'price': cmp = entryPrice(a) - entryPrice(b); break
    }
    return sortDir === 'asc' ? cmp : -cmp
  })

  // ── Derived: sidebar stats ─────────────────────────────────────────────────

  $: fileStats = catalogFiles.map(f => ({
    name: f,
    total:    entries.filter(e => e.source_file === f).length,
    modified: entries.filter(e => e.source_file === f && e.from_modified).length,
  }))

  /** "CatalogBundle.json" shows as "Bundle"; the full name is on the tooltip. */
  const shortName = (f: string) => f.replace(/^Catalog/, '').replace(/\.json$/, '') || f

  // Types present within the current file filter (ignoring type filter itself
  // so the dropdown doesn't collapse while a type is selected)
  $: activeTypes = typeNames().filter(t => {
    const scope = fileFilter ? entries.filter(e => e.source_file === fileFilter) : entries
    return scope.some(e => e.Type.Name === t)
  })

  // ── Derived: selection ─────────────────────────────────────────────────────

  $: allVisibleSelected  = sortedEntries.length > 0 && sortedEntries.every(e => selectedSkus.has(e.SkuId))
  $: someVisibleSelected = sortedEntries.some(e => selectedSkus.has(e.SkuId))
  $: totalSelectedCount  = selectedSkus.size
  // Stock rows are in neither list: stock files are never written.
  $: revertTargets = entries.filter(e => selectedSkus.has(e.SkuId) && removeKind(e) === 'revert')
  $: deleteTargets = entries.filter(e => selectedSkus.has(e.SkuId) && removeKind(e) === 'delete')
  $: confirmCount  = (batchConfirm === 'revert' ? revertTargets : deleteTargets).length

  // Imperatively set indeterminate; Svelte has no binding for this property.
  $: if (headerCheckEl) {
    headerCheckEl.indeterminate = someVisibleSelected && !allVisibleSelected
  }

  // ── Derived: modifiers available for batch operations ──────────────────────

  // When a type filter is active, scope to that type's modifiers. Otherwise
  // show the union of all types so mixed selections can still be operated on.
  $: batchModifiers = (() => {
    const scope = typeFilter ? [typeFilter] : typeNames()
    const seen = new Set<string>()
    const result: string[] = []
    for (const t of scope) {
      for (const m of modifiersForType(t)) {
        if (!seen.has(m)) { seen.add(m); result.push(m) }
      }
    }
    return result
  })()

  // ── Sort ───────────────────────────────────────────────────────────────────

  function toggleSort(col: typeof sortCol) {
    if (sortCol === col) sortDir = sortDir === 'asc' ? 'desc' : 'asc'
    else { sortCol = col; sortDir = 'asc' }
  }

  // ── Selection ──────────────────────────────────────────────────────────────

  function toggleSelectAll() {
    if (allVisibleSelected) {
      for (const e of sortedEntries) selectedSkus.delete(e.SkuId)
    } else {
      for (const e of sortedEntries) selectedSkus.add(e.SkuId)
    }
    selectedSkus = selectedSkus
  }

  function toggleSelectRow(skuId: string) {
    if (selectedSkus.has(skuId)) selectedSkus.delete(skuId)
    else selectedSkus.add(skuId)
    selectedSkus = selectedSkus
  }

  function clearSelection() {
    selectedSkus = new Set()
    resetBatch()
  }

  function resetBatch() {
    batchMode = null
    batchPriceStr = ''
    batchError = ''
    batchSuccess = ''
    batchConfirm = null
    batchRunning = false
  }

  function setBatchMode(mode: BatchMode) {
    batchMode = mode
    batchError = ''
    batchSuccess = ''
    batchConfirm = null
    // Default the modifier select to the first available option when opening
    // an add/remove modifier operation so the Apply button is immediately usable.
    if ((mode === 'add-mod' || mode === 'remove-mod') && !batchModifier) {
      batchModifier = batchModifiers[0] ?? ''
    }
  }

  // ── Load ───────────────────────────────────────────────────────────────────

  async function load() {
    if (!$appConfig.server_exe) return
    loading = true
    loadError = ''
    clearSelection()
    try {
      const [files, off, loaded] = await Promise.all([
        invoke<string[]>('list_catalog_files', { serverExe: $appConfig.server_exe }),
        invoke<string[]>('list_off_catalog_files', { serverExe: $appConfig.server_exe }),
        invoke<CatalogEntryWithMeta[]>('load_catalog_entries', { serverExe: $appConfig.server_exe }),
      ])
      catalogFiles = files
      offFiles = off
      entries = loaded
      if (fileFilter && !files.includes(fileFilter)) fileFilter = ''
    } catch (e) {
      loadError = String(e)
    } finally {
      loading = false
    }
  }

  /** Moves the file and its MODIFIED file into MTXStore\Off (off) or back (on), then reloads. */
  async function setFileEnabled(file: string, enabled: boolean) {
    switchError = ''
    try {
      await invoke('set_catalog_file_enabled', { serverExe: $appConfig.server_exe, file, enabled })
      await load()
    } catch (e) {
      switchError = String(e)
    }
  }

  /** Shows a file the New catalog file modal just made. */
  async function handleFileCreated(name: string) {
    creatingFile = false
    await load()
    fileFilter = name
    typeFilter = ''
  }

  async function openMtxStoreDir() {
    if (!$appConfig.server_exe) return
    openDirError = ''
    try {
      const dir = await invoke<string>('get_mtxstore_dir', { serverExe: $appConfig.server_exe })
      await openPath(dir)
    } catch (e) {
      openDirError = String(e)
    }
  }

  // ── Single-entry callbacks ─────────────────────────────────────────────────

  function handleSaved(updated: CatalogEntryWithMeta) {
    const idx = entries.findIndex(e => e.SkuId === updated.SkuId)
    if (idx !== -1) {
      entries = entries.map((e, i) => i === idx ? updated : e)
      if (editingEntry?.SkuId === updated.SkuId) editingEntry = updated
    } else {
      entries = [...entries, updated]
    }
  }

  function handleDeleted(skuId: string) {
    entries = entries.filter(e => e.SkuId !== skuId)
    selectedSkus.delete(skuId)
    selectedSkus = selectedSkus
    if (editingEntry?.SkuId === skuId) { editingEntry = null; editOpenError = '' }
  }

  /** Shown by the edit modal that opens after a new entry is saved. */
  let editOpenError = ''

  function handleCreated(created: CatalogEntryWithMeta, pageError = '') {
    entries = [...entries, created]
    creatingNew = false
    editOpenError = pageError
    editingEntry = created
  }

  // ── Batch: set price ───────────────────────────────────────────────────────

  async function applyBatchPrice() {
    const price = Number(batchPriceStr)
    if (batchPriceStr === '' || isNaN(price) || price < 0) {
      batchError = 'Enter a valid price.'
      return
    }
    batchRunning = true
    batchError   = ''
    batchSuccess = ''
    const targets = entries.filter(e => selectedSkus.has(e.SkuId))
    let count = 0
    try {
      for (const orig of targets) {
        const newEntry = {
          ...orig,
          LocalizedEntries: orig.LocalizedEntries.map(loc => ({ ...loc, ItemPrice: price })),
        }
        await invoke('save_catalog_entry', {
          serverExe:  $appConfig.server_exe,
          entry:      newEntry,
          targetFile: orig.source_file,
        })
        handleSaved({ ...newEntry, source_file: orig.source_file, from_modified: true })
        count++
      }
      batchSuccess = `Updated ${count} entr${count !== 1 ? 'ies' : 'y'}.`
      batchMode    = null
      batchPriceStr = ''
    } catch (e) {
      batchError = String(e)
    } finally {
      batchRunning = false
    }
  }

  // ── Batch: add / remove modifier ───────────────────────────────────────────

  async function applyBatchModifier(action: 'add' | 'remove') {
    if (!batchModifier) { batchError = 'Select a modifier.'; return }
    batchRunning = true
    batchError   = ''
    batchSuccess = ''
    const targets = entries.filter(e => selectedSkus.has(e.SkuId))
    let count = 0
    try {
      for (const orig of targets) {
        const currentNames = orig.TypeModifiers.map(m => m.Name)
        const has = currentNames.includes(batchModifier)
        // Skip entries where the modifier is already in the desired state.
        if (action === 'add' ? has : !has) continue
        const newNames = action === 'add'
          ? [...currentNames, batchModifier]
          : currentNames.filter(n => n !== batchModifier)
        const newEntry = {
          ...orig,
          TypeModifiers: buildModifiers(newNames, orig.Type.Order),
        }
        await invoke('save_catalog_entry', {
          serverExe:  $appConfig.server_exe,
          entry:      newEntry,
          targetFile: orig.source_file,
        })
        handleSaved({ ...newEntry, source_file: orig.source_file, from_modified: true })
        count++
      }
      const verb   = action === 'add' ? 'Added to' : 'Removed from'
      batchSuccess = `${verb} ${count} entr${count !== 1 ? 'ies' : 'y'}.`
      batchMode    = null
    } catch (e) {
      batchError = String(e)
    } finally {
      batchRunning = false
    }
  }

  // ── Batch: revert to stock / delete ────────────────────────────────────────

  // Both drop MODIFIED rows, in one write: a reverted entry falls back to stock, a deleted one is gone.
  async function confirmBatchRemove() {
    const kind = batchConfirm
    const targets = kind === 'revert' ? revertTargets : deleteTargets
    batchRunning = true
    batchError   = ''
    batchSuccess = ''
    try {
      await invoke('apply_store_changes', {
        serverExe: $appConfig.server_exe,
        upserts:   [],
        removes:   targets.map(e => ({ sku_id: e.SkuId, source_file: e.source_file })),
        state:     null,
      })
      for (const e of targets) {
        if (kind === 'revert') handleSaved(stockVersion(e))
        else handleDeleted(e.SkuId)
      }
      const n = `${targets.length} entr${targets.length !== 1 ? 'ies' : 'y'}`
      batchSuccess = kind === 'revert' ? `Reverted ${n} to stock.` : `Deleted ${n}.`
      batchConfirm = null
    } catch (e) {
      batchError = String(e)
    } finally {
      batchRunning = false
    }
  }

  onMount(() => {
    if ($appConfig.server_exe) load()
  })
</script>

<div class="store-panel">
  <div class="panel-bg"></div>
  <div class="grid-overlay"></div>
  <div class="store-layout">

    <!-- ── Sidebar ── -->
    <PanelSidebar width="var(--sidebar-wide)">
      <svelte:fragment slot="header">
        <div class="section-title">Catalog Files</div>
        <button
          class="btn-icon"
          on:click={() => creatingFile = true}
          aria-label="New catalog file"
          use:tip={'New catalog file'}
          disabled={!$appConfig.server_exe}
          style="margin-left: auto;"
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="12" y1="5" x2="12" y2="19"/>
            <line x1="5" y1="12" x2="19" y2="12"/>
          </svg>
        </button>
        <button
          class="btn-icon"
          on:click={load}
          aria-label="Reload catalog files"
          use:tip={'Reload catalog files'}
          disabled={loading}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="23 4 23 10 17 10"/>
            <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/>
          </svg>
        </button>
        <button
          class="btn-icon"
          on:click={openMtxStoreDir}
          aria-label="Open MTXStore folder"
          use:tip={'Open MTXStore folder'}
          disabled={!$appConfig.server_exe}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
          </svg>
        </button>
      </svelte:fragment>

      {#if entries.length > 0}
        <div class="stats-row">
          <div class="stat">
            <span class="stat-value">{entries.length}</span>
            <span class="stat-label">total</span>
          </div>
          <div class="stat-sep"></div>
          <div class="stat">
            <span class="stat-value">{entries.filter(e => e.from_modified).length}</span>
            <span class="stat-label">modified</span>
          </div>
        </div>
      {/if}

      <div class="file-list">
        {#if !$appConfig.server_exe}
          <div class="file-notice">Set server exe in Settings to load catalog files.</div>
        {:else if loading}
          <div class="file-notice">Loading...</div>
        {:else if loadError}
          <div class="file-notice error">{loadError}</div>
        {:else if openDirError}
          <div class="file-notice error">{openDirError}</div>
        {:else if catalogFiles.length === 0 && offFiles.length === 0}
          <div class="file-notice">No Catalog*.json files found in MTXStore.</div>
        {:else}
          <div
            class="file-item"
            class:active={fileFilter === ''}
            role="button"
            tabindex="0"
            on:click={() => fileFilter = ''}
            on:keydown={e => e.key === 'Enter' && (fileFilter = '')}
          >
            <div class="file-info">
              <span class="file-name">All Files</span>
              <span class="file-sub">{entries.length} entries</span>
            </div>
          </div>

          {#each fileStats as stat (stat.name)}
            <div
              class="file-item"
              class:active={fileFilter === stat.name}
              role="button"
              tabindex="0"
              on:click={() => { fileFilter = stat.name; typeFilter = '' }}
              on:keydown={e => e.key === 'Enter' && (fileFilter = stat.name, typeFilter = '')}
            >
              <div class="file-info">
                <span class="file-name">{stat.name.replace(/^Catalog/, '').replace(/\.json$/, '') || stat.name}</span>
                <span class="file-sub">
                  {stat.total} entr{stat.total !== 1 ? 'ies' : 'y'}
                  {#if stat.modified > 0}
                    · <span class="modified-count">{stat.modified} modified</span>
                  {/if}
                </span>
              </div>
              <div
                class="toggle-switch on"
                role="switch"
                aria-checked="true"
                aria-label="{stat.name} on"
                tabindex="0"
                use:tip={'On. Switch off to stop the server loading this file and your changes to it.'}
                on:click|stopPropagation={() => setFileEnabled(stat.name, false)}
                on:keydown|stopPropagation={e => e.key === 'Enter' && setFileEnabled(stat.name, false)}
              ></div>
            </div>
          {/each}

          {#if offFiles.length}
            <div class="side-label">Switched off</div>
            {#each offFiles as name (name)}
              <div class="file-item off">
                <div class="file-info">
                  <span class="file-name" use:tip={`${name}, kept in MTXStore\\Off`}>
                    {name.replace(/^Catalog/, '').replace(/\.json$/, '') || name}
                  </span>
                </div>
                <div
                  class="toggle-switch"
                  role="switch"
                  aria-checked="false"
                  aria-label="{name} off"
                  tabindex="0"
                  use:tip={'Off. Switch on to load this file again.'}
                  on:click={() => setFileEnabled(name, true)}
                  on:keydown={e => e.key === 'Enter' && setFileEnabled(name, true)}
                ></div>
              </div>
            {/each}
          {/if}
          {#if switchError}
            <div class="file-notice error">{switchError}</div>
          {/if}
        {/if}
      </div>
    </PanelSidebar>

    <!-- ── Content pane ── -->
    <section class="data-main">
      <header class="page-head">
        <h3 class="page-title">{fileFilter ? shortName(fileFilter) : 'All files'}</h3>
        <div class="page-acts">
          {#if reloadError}<span class="reload-error">{reloadError}</span>{/if}
          <button
            class="btn btn-sm btn-outline"
            on:click={() => { creatingNew = true; editingEntry = null }}
            disabled={!$appConfig.server_exe || catalogFiles.length === 0}
          >New entry</button>
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

      <!-- ── Toolbar ── -->
      <div class="toolbar">
        <div class="search-wrap">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="11" cy="11" r="8"/>
            <line x1="21" y1="21" x2="16.65" y2="16.65"/>
          </svg>
          <input
            type="text"
            class="search-input"
            placeholder="Search name or SKU..."
            bind:value={searchQuery}
          >
        </div>

        <div class="toolbar-sep"></div>

        <Dropdown label="Type filter" value={typeFilter} onChange={v => typeFilter = v}
          options={[{ value: '', label: 'All types' }, ...activeTypes.map(t => ({ value: t, label: t }))]}
          --dd-color="var(--text-1)" --dd-pad="5px 8px" --dd-min-width="0" />

        <div class="toolbar-sep"></div>

        <div class="price-range">
          <span class="price-label">G</span>
          <input
            type="text"
            inputmode="numeric"
            class="price-input"
            placeholder="Min"
            bind:value={priceMinStr}
          >
          <span class="price-sep">–</span>
          <input
            type="text"
            inputmode="numeric"
            class="price-input"
            placeholder="Max"
            bind:value={priceMaxStr}
          >
        </div>

        <div class="toolbar-sep"></div>

        <button
          class="filter-chip chip-amber"
          class:active={modifiedOnly}
          on:click={() => modifiedOnly = !modifiedOnly}
        >Modified</button>

        <span class="entry-count">
          {sortedEntries.length}{anyFilterActive ? ` of ${entries.length}` : ''} entr{sortedEntries.length !== 1 ? 'ies' : 'y'}
        </span>
      </div>

      <!-- ── Table area ── -->
      <div class="table-scroll">

        {#if !$appConfig.server_exe}
          <div class="empty-state">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:20px;height:20px;color:var(--text-2)">
              <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"/>
              <circle cx="12" cy="12" r="3"/>
            </svg>
            <span class="empty-state-label">No server configured</span>
            <span class="empty-state-sub">Set the server exe path in Settings to load catalog files.</span>
          </div>

        {:else if loading}
          <div class="empty-state">
            <span class="empty-state-label">Loading...</span>
          </div>

        {:else if loadError}
          <div class="empty-state">
            <span class="empty-state-label error">Load error</span>
            <span class="empty-state-sub">{loadError}</span>
          </div>

        {:else if entries.length === 0}
          <div class="empty-state">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
              <path d="M6 2L3 6v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V6l-3-4z"/>
              <line x1="3" y1="6" x2="21" y2="6"/>
              <path d="M16 10a4 4 0 0 1-8 0"/>
            </svg>
            <span class="empty-state-label">No catalog entries</span>
            <span class="empty-state-sub">No Catalog*.json files were found in the MTXStore directory.</span>
          </div>

        {:else if sortedEntries.length === 0}
          <div class="empty-state">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
              <circle cx="11" cy="11" r="8"/>
              <line x1="21" y1="21" x2="16.65" y2="16.65"/>
            </svg>
            <span class="empty-state-label">No results</span>
            <span class="empty-state-sub">No entries match the current filter.</span>
          </div>

        {:else}
          <table class="catalog-table">
            <thead>
              <tr>
                <th class="col-check">
                  <input
                    type="checkbox"
                    bind:this={headerCheckEl}
                    checked={allVisibleSelected}
                    on:change={toggleSelectAll}
                  >
                </th>
                <th class="col-sku sortable" on:click={() => toggleSort('sku')}>
                  SKU
                  <span class="sort-ind" class:active={sortCol === 'sku'}>
                    {sortCol === 'sku' ? (sortDir === 'asc' ? '↑' : '↓') : '↕'}
                  </span>
                </th>
                <th class="col-name sortable" on:click={() => toggleSort('name')}>
                  Name
                  <span class="sort-ind" class:active={sortCol === 'name'}>
                    {sortCol === 'name' ? (sortDir === 'asc' ? '↑' : '↓') : '↕'}
                  </span>
                </th>
                <th class="col-type sortable" on:click={() => toggleSort('type')}>
                  Type
                  <span class="sort-ind" class:active={sortCol === 'type'}>
                    {sortCol === 'type' ? (sortDir === 'asc' ? '↑' : '↓') : '↕'}
                  </span>
                </th>
                <th class="col-price sortable" on:click={() => toggleSort('price')}>
                  Price
                  <span class="sort-ind" class:active={sortCol === 'price'}>
                    {sortCol === 'price' ? (sortDir === 'asc' ? '↑' : '↓') : '↕'}
                  </span>
                </th>
                <th class="col-source">Source</th>
                <th class="col-actions"></th>
              </tr>
            </thead>
            <tbody>
              {#each sortedEntries as entry (entry.SkuId)}
                {@const title    = entryTitle(entry)}
                {@const price    = entryPrice(entry)}
                {@const selected = selectedSkus.has(entry.SkuId)}
                <tr
                  class:row-selected={selected}
                  on:click={() => toggleSelectRow(entry.SkuId)}
                >
                  <td class="col-check" on:click|stopPropagation>
                    <input
                      type="checkbox"
                      checked={selected}
                      on:change={() => toggleSelectRow(entry.SkuId)}
                    >
                  </td>
                  <td class="col-sku">{entry.SkuId}</td>
                  <td class="col-name" use:tip={title}>{title}</td>
                  <td class="col-type">
                    {#if entry.Type.Name}<span class="tag">{entry.Type.Name}</span>{/if}
                  </td>
                  <td class="col-price">{price.toLocaleString()} G</td>
                  <td class="col-source">
                    <div class="source-cell">
                      <span class="source-name" use:tip={entry.source_file}>{shortName(entry.source_file)}</span>
                      {#if entry.from_modified}
                        <span class="modified">Modified</span>
                      {/if}
                    </div>
                  </td>
                  <td class="col-actions" on:click|stopPropagation>
                    <button
                      class="btn btn-sm btn-outline row-edit-btn"
                      on:click={() => editingEntry = entry}
                    >Edit</button>
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        {/if}

      </div><!-- /table-scroll -->

      <!-- ── Batch bar (footer, so rows never move) ── -->
      {#if totalSelectedCount > 0}
        <div class="batch-bar">

          {#if batchConfirm}
            <span class="batch-confirm-msg">
              {#if batchConfirm === 'revert'}
                Revert <strong>{confirmCount}</strong> entr{confirmCount !== 1 ? 'ies' : 'y'} to stock? Your changes to them are removed.
              {:else}
                Delete <strong>{confirmCount}</strong> entr{confirmCount !== 1 ? 'ies' : 'y'}? This cannot be undone.
              {/if}
            </span>
            <div class="batch-actions" style="margin-left: auto;">
              {#if batchError}
                <span class="batch-feedback error">{batchError}</span>
              {/if}
              <button
                class="btn btn-sm btn-outline"
                on:click={() => batchConfirm = null}
                disabled={batchRunning}
              >Cancel</button>
              <button
                class="btn btn-sm {batchConfirm === 'revert' ? 'btn-accent' : 'btn-red'}"
                on:click={confirmBatchRemove}
                disabled={batchRunning || confirmCount === 0}
              >{batchConfirm === 'revert'
                ? (batchRunning ? 'Reverting...' : 'Confirm Revert')
                : (batchRunning ? 'Deleting...' : 'Confirm Delete')}</button>
            </div>

          {:else if batchMode === 'price'}
            <span class="batch-label">{totalSelectedCount} selected</span>
            <button class="batch-clear" on:click={clearSelection} use:tip={'Clear selection'}>×</button>
            <div class="batch-sep"></div>
            <span class="batch-action-label">New price (G):</span>
            <input
              type="text"
              inputmode="numeric"
              class="batch-inline-input"
              placeholder="0"
              bind:value={batchPriceStr}
              disabled={batchRunning}
            >
            <button
              class="btn btn-sm btn-accent"
              on:click={applyBatchPrice}
              disabled={batchRunning || batchPriceStr === ''}
            >{batchRunning ? 'Applying...' : 'Apply'}</button>
            <button
              class="btn btn-sm btn-outline"
              on:click={() => setBatchMode(null)}
              disabled={batchRunning}
            >Cancel</button>
            {#if batchError}<span class="batch-feedback error">{batchError}</span>{/if}
            {#if batchSuccess}<span class="batch-feedback ok">{batchSuccess}</span>{/if}

          {:else if batchMode === 'add-mod' || batchMode === 'remove-mod'}
            <span class="batch-label">{totalSelectedCount} selected</span>
            <button class="batch-clear" on:click={clearSelection} use:tip={'Clear selection'}>×</button>
            <div class="batch-sep"></div>
            <span class="batch-action-label">
              {batchMode === 'add-mod' ? 'Add modifier:' : 'Remove modifier:'}
            </span>
            <Dropdown label="Modifier" value={batchModifier} disabled={batchRunning} onChange={v => batchModifier = v}
              options={batchModifiers.length ? batchModifiers.map(m => ({ value: m, label: m })) : [{ value: '', label: 'No modifiers available' }]}
              --dd-color="var(--text-1)" --dd-pad="4px 8px" --dd-min-width="0" />
            <button
              class="btn btn-sm btn-accent"
              on:click={() => applyBatchModifier(batchMode === 'add-mod' ? 'add' : 'remove')}
              disabled={batchRunning || !batchModifier}
            >{batchRunning ? 'Applying...' : (batchMode === 'add-mod' ? 'Add' : 'Remove')}</button>
            <button
              class="btn btn-sm btn-outline"
              on:click={() => setBatchMode(null)}
              disabled={batchRunning}
            >Cancel</button>
            {#if batchError}<span class="batch-feedback error">{batchError}</span>{/if}
            {#if batchSuccess}<span class="batch-feedback ok">{batchSuccess}</span>{/if}

          {:else}
            <span class="batch-label">{totalSelectedCount} selected</span>
            <button class="batch-clear" on:click={clearSelection} use:tip={'Clear selection'}>×</button>
            <div class="batch-sep"></div>
            <div class="batch-actions">
              <button class="btn btn-sm btn-outline" on:click={() => setBatchMode('price')}>Set Price</button>
              <button class="btn btn-sm btn-outline" on:click={() => setBatchMode('add-mod')}>Add Modifier</button>
              <button class="btn btn-sm btn-outline" on:click={() => setBatchMode('remove-mod')}>Remove Modifier</button>
              {#if revertTargets.length}
                <button
                  class="btn btn-sm btn-outline"
                  on:click={() => { batchConfirm = 'revert'; batchError = ''; batchSuccess = '' }}
                  use:tip={'Remove your changes from the selected stock items'}
                >Revert to stock</button>
              {/if}
              {#if deleteTargets.length}
                <button
                  class="btn btn-sm btn-red"
                  on:click={() => { batchConfirm = 'delete'; batchError = ''; batchSuccess = '' }}
                  use:tip={'Delete the selected items you added. Stock items are skipped.'}
                >Delete</button>
              {/if}
            </div>
            {#if batchSuccess}
              <span class="batch-feedback ok">{batchSuccess}</span>
            {/if}
          {/if}

        </div>
      {/if}

    </section>

  </div><!-- /store-layout -->
</div>

<!-- Edit existing entry -->
{#if editingEntry}
  <StoreEditorModal
    entry={editingEntry}
    serverExe={$appConfig.server_exe}
    catalogFiles={catalogFiles}
    allEntries={entries}
    initialError={editOpenError}
    onClose={() => { editingEntry = null; editOpenError = '' }}
    onSaved={handleSaved}
    onDeleted={handleDeleted}
  />
{/if}

{#if creatingFile}
  <NewFileModal
    title="New catalog file"
    prefix="Catalog"
    placeholder="MyItems"
    hint="Letters, numbers, _ or -. Add items to it with New entry."
    create={suffix => invoke<string>('create_catalog_file', { serverExe: $appConfig.server_exe, suffix })}
    onCreated={handleFileCreated}
    onClose={() => creatingFile = false}
  />
{/if}

<!-- Create new entry -->
{#if creatingNew}
  <StoreEditorModal
    entry={null}
    serverExe={$appConfig.server_exe}
    catalogFiles={catalogFiles}
    allEntries={entries}
    defaultFile={fileFilter}
    onClose={() => creatingNew = false}
    onSaved={handleCreated}
    onDeleted={() => {}}
  />
{/if}

<style>
  /* ── Panel shell ── */
  .store-panel {
    display: flex;
    flex: 1;
    flex-direction: column;
    position: relative;
    overflow: hidden;
    min-height: 0;
  }

  .store-layout {
    position: relative;
    z-index: 1;
    flex: 1;
    display: flex;
    overflow: hidden;
    min-height: 0;
  }

  /* ── Sidebar internals ── */
  .stats-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 14px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .stat         { display: flex; align-items: baseline; gap: 3px; }
  .stat-sep     { width: 1px; height: 12px; background: var(--border-mid); }

  .stat-value {
    font-family: var(--font-mono);
    font-size: 13px;
    color: var(--accent-bright);
  }

  .stat-label {
    font-family: var(--font-head);
    font-size: var(--fs-min);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
  }

  .file-list {
    flex: 1;
    overflow-y: auto;
    padding: 6px;
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

  .file-item {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 7px 10px;
    min-height: 42px;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: all 0.1s;
    margin-bottom: 2px;
  }
  .file-item:hover  { background: var(--bg-3); border-color: var(--border-mid); }
  .file-item.active { background: var(--accent-glow); border-color: var(--accent-dim); }

  .file-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    flex: 1;
  }

  .file-name {
    font-family: var(--font-head);
    font-size: 11px;
    font-weight: 600;
    color: var(--text-1);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .file-item.active .file-name { color: var(--accent-bright); }

  .file-sub {
    font-family: var(--font-body);
    font-size: var(--fs-min);
    color: var(--text-3);
  }

  .modified-count { color: var(--amber-bright); }

  .file-item.off { cursor: default; }
  .file-item.off:hover { background: none; border-color: transparent; }
  .file-item.off .file-name { color: var(--text-3); }

  /* ── Toolbar ── */
  .toolbar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 16px 8px 20px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    flex-wrap: wrap;
  }

  .search-wrap { min-width: 100px; max-width: 240px; }

  .toolbar-sep {
    width: 1px;
    height: 18px;
    background: var(--border-mid);
    flex-shrink: 0;
    margin: 0 2px;
  }


  /* Price range */
  .price-range {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
  }

  .price-label {
    font-family: var(--font-body);
    font-size: 11px;
    color: var(--text-3);
    flex-shrink: 0;
  }

  .price-sep {
    font-family: var(--font-body);
    font-size: 12px;
    color: var(--text-3);
    flex-shrink: 0;
  }

  .price-input {
    width: 68px;
    /* Override global input */
    padding: 5px 7px;
    font-size: 12px;
    flex-shrink: 0;
  }
  .price-input::placeholder { font-size: 11px; }

  .entry-count {
    font-family: var(--font-head);
    font-size: var(--fs-min);
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-3);
    white-space: nowrap;
    margin-left: auto;
    flex-shrink: 0;
  }

  /* ── Batch bar ── */
  .batch-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 16px 7px 20px;
    border-top: 1px solid var(--accent-dim);
    background: var(--accent-glow);
    flex-shrink: 0;
    min-height: 44px;
    flex-wrap: wrap;
  }

  .batch-label {
    font-family: var(--font-head);
    font-size: var(--fs-min);
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--accent-bright);
    white-space: nowrap;
    flex-shrink: 0;
  }

  .batch-clear {
    width: 18px;
    height: 18px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: 1px solid var(--accent-dim);
    border-radius: 50%;
    color: var(--accent-bright);
    cursor: pointer;
    font-size: 13px;
    line-height: 1;
    padding: 0;
    flex-shrink: 0;
    transition: background 0.1s;
  }
  .batch-clear:hover { background: var(--accent-glow-strong); }

  .batch-sep {
    width: 1px;
    height: 16px;
    background: var(--accent-dim);
    flex-shrink: 0;
  }

  .batch-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }

  .batch-action-label {
    font-family: var(--font-head);
    font-size: var(--fs-min);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-2);
    white-space: nowrap;
    flex-shrink: 0;
  }

  .batch-confirm-msg {
    font-family: var(--font-body);
    font-size: 12px;
    color: var(--text-error);
    flex-shrink: 0;
  }
  .batch-confirm-msg strong { font-weight: 600; }

  .batch-inline-input {
    width: 90px;
    /* Override global input */
    padding: 4px 7px;
    font-size: 12px;
    flex-shrink: 0;
  }
  .batch-inline-input::placeholder { font-size: 11px; }


  .batch-feedback {
    font-size: 11px;
    white-space: nowrap;
    flex-shrink: 0;
  }
  .batch-feedback.error { color: var(--text-error); }
  .reload-error { font-size: 12px; color: var(--text-error); }
  .batch-feedback.ok {
    color: var(--text-success);
    font-family: var(--font-head);
    letter-spacing: 0.08em;
  }

  /* ── Table scroll container ── */
  .table-scroll {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    min-height: 0;
    display: flex;
  }

  /* ── Catalog table ── */
  .catalog-table {
    align-self: flex-start; /* a few rows keep their height instead of filling the pane */
    width: 100%;
    border-collapse: collapse;
    table-layout: fixed;
  }

  /* Sticky header */
  .catalog-table thead {
    position: sticky;
    top: 0;
    z-index: 2;
  }

  .catalog-table thead th {
    background: var(--bg-1);
    border-bottom: 1px solid var(--border);
    padding: 7px 12px;
    text-align: left;
    font-family: var(--font-head);
    font-size: var(--fs-min);
    font-weight: 600;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-3);
    white-space: nowrap;
    user-select: none;
  }

  .catalog-table thead th.sortable {
    cursor: pointer;
    transition: color 0.1s;
  }
  .catalog-table thead th.sortable:hover { color: var(--text-1); }

  .sort-ind {
    font-size: var(--fs-min);
    margin-left: 3px;
    opacity: 0.25;
  }
  .sort-ind.active {
    opacity: 1;
    color: var(--accent-bright);
  }

  /* Column widths (table-layout: fixed so these are authoritative) */
  .col-check   { width: 38px;  text-align: center; }
  .col-sku     { width: 82px;  }
  /*.col-name    { /* takes remaining width }*/
  .col-type    { width: 100px; }
  .col-price   { width: 96px;  }
  .col-source  { width: 122px; }
  .col-actions { width: 64px;  }

  /* Rows */
  .catalog-table tbody tr {
    border-bottom: 1px solid var(--border);
    cursor: pointer;
    transition: background 0.07s;
  }
  .catalog-table tbody tr:last-child { border-bottom: none; }
  .catalog-table tbody tr:hover       { background: var(--bg-2); }
  .catalog-table tbody tr.row-selected { background: var(--accent-glow); }
  .catalog-table tbody tr.row-selected:hover { background: var(--accent-glow-strong); }

  /* Cells */
  .catalog-table td {
    padding: 8px 12px;
    vertical-align: middle;
    font-size: 12px;
    overflow: hidden;
  }

  .catalog-table .col-check { text-align: center; padding: 0; }
  .catalog-table .col-check input[type="checkbox"] { margin: 0; }

  .catalog-table .col-sku {
    font-family: var(--font-body);
    font-variant-numeric: tabular-nums;
    font-size: 11px;
    color: var(--text-2);
    white-space: nowrap;
  }

  /* Name column: max-width: 0 is required for text-overflow to work
     in a table cell when table-layout is fixed. */
  .catalog-table .col-name {
    max-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text-0);
  }

  .catalog-table .col-price {
    font-family: var(--font-body);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    color: var(--text-1);
    text-align: right;
    white-space: nowrap;
  }

  /* Explicit header overrides — thead th.col-* has higher specificity than
     .col-* alone, so these always win and keep all headers visually uniform. */
  .catalog-table thead th.col-sku,
  .catalog-table thead th.col-name,
  .catalog-table thead th.col-price {
    font-family: var(--font-head);
    font-size: var(--fs-min);
    color: var(--text-3);
    text-align: left;
  }

  .catalog-table .col-source { padding-right: 8px; }
  .catalog-table .col-actions { text-align: right; padding-right: 10px; }

  /* Source cell */
  .source-cell {
    display: flex;
    align-items: center;
    gap: 5px;
    overflow: hidden;
  }

  .source-name {
    font-family: var(--font-body);
    font-size: 11px;
    color: var(--text-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  /* Row edit button — slightly smaller padding than the global btn-sm */
  .row-edit-btn { padding: 3px 10px; }

  /* Checkboxes */
  .catalog-table input[type="checkbox"] {
    cursor: pointer;
    accent-color: var(--accent);
    width: 12px;
    height: 12px;
  }

  /* Changed from stock: state, so amber. */
  .modified {
    font-family: var(--font-head);
    font-size: var(--fs-min);
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--amber-bright);
    flex-shrink: 0;
  }
</style>