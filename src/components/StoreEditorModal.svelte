<script lang="ts">
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import {
    typeNames,
    modifiersForType,
    buildModifiers,
    defaultLocalizedEntry,
    inferOfferType,
    ITEM_CATEGORIES,
    type ItemCategory,
    type OfferType,
    type CatalogEntry,
    type CatalogEntryWithMeta,
    type GuidItem,
  } from '../lib/catalogMeta'

  type EditableStoreState = {
    title: string
    description: string
    price: number
    releaseDate: string
    typeName: string
    typeOrder: number
    infoUrl: string
    contentUrl: string
    guidItems: GuidItem[]
    additionalGuidItems: GuidItem[]
    modifiers: string[]
  }

  // ── Props ─────────────────────────────────────────────────────────────────

  /** Existing entry to edit. Null when creating a new entry. */
  export let entry: CatalogEntryWithMeta | null = null
  export let serverExe: string
  /** Base `Catalog*.json` filenames present in MTXStore — for the target-file dropdown. */
  export let catalogFiles: string[]
  /** Output directory for generated bundle HTML. Empty string uses the Rust default. */
  export let htmlOutputDir: string = ''
  /**
   * All currently loaded catalog entries. Used by the item picker to identify
   * items already present in any entry across the catalog.
   */
  export let allEntries: CatalogEntryWithMeta[] = []
  export let onClose: () => void
  export let onSaved: (entry: CatalogEntryWithMeta) => void = () => {}
  export let onDeleted: (skuId: number) => void = () => {}

  function deepClone<T>(value: T): T {
    return JSON.parse(JSON.stringify(value))
  }

  function cloneGuidItems(items: GuidItem[] = []): GuidItem[] {
    return items.map(item => ({ ...item }))
  }

  function pickLocalizedEntry(source: CatalogEntry | CatalogEntryWithMeta | null | undefined) {
    return (
      source?.LocalizedEntries.find(e => e.LanguageId === 'en_us') ??
      source?.LocalizedEntries[0] ??
      defaultLocalizedEntry()
    )
  }

  function pickLocalizedUrl(
    items: { LanguageId: string; Url: string }[] | undefined
  ): string {
    return (
      items?.find(item => item.LanguageId === 'en_us')?.Url ??
      items?.[0]?.Url ??
      ''
    )
  }

  function normalizeEntryForEdit(
    source: CatalogEntry | CatalogEntryWithMeta | null | undefined
  ): EditableStoreState {
    const loc = pickLocalizedEntry(source)
    return {
      title: loc.Title ?? '',
      description: loc.Description ?? '',
      price: loc.ItemPrice ?? 1,
      releaseDate: loc.ReleaseDate ?? '',
      typeName: source?.Type.Name ?? 'Boost',
      typeOrder: source?.Type.Order ?? 5,
      infoUrl: pickLocalizedUrl(source?.InfoUrls),
      contentUrl: pickLocalizedUrl(source?.ContentData),
      guidItems: cloneGuidItems(source?.GuidItems ?? []),
      additionalGuidItems: cloneGuidItems(source?.AdditionalGuidItems ?? []),
      modifiers: (source?.TypeModifiers ?? []).map(m => m.Name).sort(),
    }
  }

  function serializeEditableState(state: EditableStoreState): string {
    return JSON.stringify(state)
  }

  // ── Mode ──────────────────────────────────────────────────────────────────

  $: isNew = entry === null

  // ── Snapshot for initialization / dirty detection ─────────────────────────

  const original: CatalogEntryWithMeta | null = entry ? deepClone(entry) : null
  const initialState = normalizeEntryForEdit(original)
  let initialStateSerialized = serializeEditableState(initialState)

  // ── Offer type ────────────────────────────────────────────────────────────

  let offerType: OfferType = entry ? inferOfferType(entry) : 'single'

  // When switching back to single, restore the last non-Bundle type name.
  let prevSingleType: string =
    initialState.typeName !== 'Bundle' ? initialState.typeName : 'Boost'

  function setOfferType(ot: OfferType) {
    offerType = ot
    if (ot === 'single') {
      formTypeName = prevSingleType
    } else {
      if (formTypeName !== 'Bundle') prevSingleType = formTypeName
      formTypeName = 'Bundle'
      if (ot === 'bundle') {
        // Switching from bogo → bundle discards any bonus items.
        formAdditionalGuidItems = []
        additionalGuidNames = []
        addedAdditionalPaths.clear()
        addedAdditionalPaths = addedAdditionalPaths
      }
    }
  }

  // ── Working SKU ───────────────────────────────────────────────────────────

  let workingSku: number = entry?.SkuId ?? 0

  // ── Form fields ───────────────────────────────────────────────────────────

  let formTitle = initialState.title
  let formDesc  = initialState.description
  let formPrice = initialState.price

  let formTypeName = initialState.typeName
  let formOrder    = initialState.typeOrder

  let formInfoUrl    = initialState.infoUrl
  let formContentUrl = initialState.contentUrl

  // ── Target file (create mode) ─────────────────────────────────────────────

  let targetFile = catalogFiles[0] ?? ''

  // ── Modifier state ────────────────────────────────────────────────────────

  let selectedModifiers = new Set<string>(initialState.modifiers)
  $: availableModifiers = modifiersForType(formTypeName)
  $: unsupportedSelectedModifiers = [...selectedModifiers]
    .filter(mod => !availableModifiers.includes(mod))
    .sort()

  function toggleModifier(mod: string) {
    if (selectedModifiers.has(mod)) selectedModifiers.delete(mod)
    else selectedModifiers.add(mod)
    selectedModifiers = selectedModifiers
  }

  // ── GuidItems: main (purchase) items ──────────────────────────────────────

  let formGuidItems: GuidItem[] = cloneGuidItems(initialState.guidItems)
  /** Parallel display name array — kept in sync with formGuidItems. */
  let guidNames: string[] = formGuidItems.map(g => g.ItemPrototypeRuntimeIdForClient)

  /** Prototype paths for items added via the picker in this session. */
  let addedMainPaths = new Set<string>()

  // ── GuidItems: bonus (BOGO get) items ─────────────────────────────────────

  let formAdditionalGuidItems: GuidItem[] = cloneGuidItems(initialState.additionalGuidItems)
  let additionalGuidNames: string[] = formAdditionalGuidItems.map(g => g.ItemPrototypeRuntimeIdForClient)
  let addedAdditionalPaths = new Set<string>()

  // ── Display name resolution ───────────────────────────────────────────────

  async function resolveGuidNames() {
    guidNames = await Promise.all(
      formGuidItems.map(g =>
        invoke<string>('resolve_display_name', {
          serverExe,
          prototypeRuntimeId: g.ItemPrototypeRuntimeIdForClient,
        }).catch(() => g.ItemPrototypeRuntimeIdForClient)
      )
    )
  }

  async function resolveAdditionalGuidNames() {
    additionalGuidNames = await Promise.all(
      formAdditionalGuidItems.map(g =>
        invoke<string>('resolve_display_name', {
          serverExe,
          prototypeRuntimeId: g.ItemPrototypeRuntimeIdForClient,
        }).catch(() => g.ItemPrototypeRuntimeIdForClient)
      )
    )
  }

  // ── Item operations ───────────────────────────────────────────────────────

  function removeGuidItem(i: number) {
    formGuidItems = formGuidItems.filter((_, idx) => idx !== i)
    guidNames     = guidNames.filter((_, idx) => idx !== i)
  }

  function removeAdditionalGuidItem(i: number) {
    formAdditionalGuidItems = formAdditionalGuidItems.filter((_, idx) => idx !== i)
    additionalGuidNames     = additionalGuidNames.filter((_, idx) => idx !== i)
  }

  function updateItemQty(i: number, val: string) {
    const q = parseInt(val, 10)
    formGuidItems = formGuidItems.map((item, idx) =>
      idx === i ? { ...item, Quantity: isNaN(q) || q < 1 ? 1 : q } : item
    )
  }

  function updateAdditionalItemQty(i: number, val: string) {
    const q = parseInt(val, 10)
    formAdditionalGuidItems = formAdditionalGuidItems.map((item, idx) =>
      idx === i ? { ...item, Quantity: isNaN(q) || q < 1 ? 1 : q } : item
    )
  }

  // ── Item picker state ─────────────────────────────────────────────────────

  /** Which list the picker's Add button writes to. Only meaningful for BOGO. */
  let bogoTarget: 'main' | 'additional' = 'main'

  let selectedCategory: ItemCategory | null = null
  let pickerSearch = ''
  type PrototypePickerResult = {
    path: string
    blueprint: string
    display_name: string
    leaf: string
  }

  let pickerResults: PrototypePickerResult[] = []
  let pickerLoading = false
  let pickerError   = ''

  let pickerSelectedPath = ''
  let pickerSelectedId   = ''
  let pickerIdLoading    = false
  let pickerIdError      = ''
  let pickerQty          = 1

  let hideAddedItems = false
  let hideProtoPaths = true

  let pickerDebounce: ReturnType<typeof setTimeout> | null = null

  // Set of runtime ID strings from all loaded catalog entries, used to flag
  // already-catalogued items when the user selects them in the picker.
  $: existingEntryIds = new Set<string>(
    allEntries.flatMap(e => [
      ...e.GuidItems.map(g => g.ItemPrototypeRuntimeIdForClient),
      ...e.AdditionalGuidItems.map(g => g.ItemPrototypeRuntimeIdForClient),
    ])
  )

  // Is the currently selected picker item already in any catalog entry?
  $: pickerSelectedIsExisting =
    pickerSelectedId !== '' && existingEntryIds.has(pickerSelectedId)

  // All prototype paths added to either list this session.
  $: allAddedPaths = new Set([...addedMainPaths, ...addedAdditionalPaths])

  $: filteredPickerResults = hideAddedItems
    ? pickerResults.filter(r => !allAddedPaths.has(r.path))
    : pickerResults

  function hasFriendlyDisplayName(result: PrototypePickerResult): boolean {
    return !!result.display_name && result.display_name !== result.path
  }

  function formatPickerResultLabel(result: PrototypePickerResult): string {
    return hasFriendlyDisplayName(result)
      ? `${result.display_name} (${result.path})`
      : result.path
  }

  function formatPickerResultCompactLabel(result: PrototypePickerResult): string {
    return hasFriendlyDisplayName(result)
      ? result.display_name
      : (result.leaf || result.path.split('/').pop() || result.path)
  }

  function selectedPickerLabel(): string {
    const selected = pickerResults.find(r => r.path === pickerSelectedPath)
    if (!selected) return pickerSelectedPath.split('/').pop() ?? pickerSelectedPath
    return hideProtoPaths
      ? formatPickerResultCompactLabel(selected)
      : formatPickerResultLabel(selected)
  }

  async function loadCategory(cat: ItemCategory) {
    selectedCategory = cat
    pickerSelectedPath = ''
    pickerSelectedId   = ''
    pickerIdError      = ''
    await runPickerSearch(pickerSearch)
  }

  async function runPickerSearch(query: string) {
    if (!selectedCategory) return
    pickerLoading = true
    pickerError   = ''
    pickerResults = []
    try {
      pickerResults = await invoke<PrototypePickerResult[]>(
        'search_prototypes',
        {
          serverExe,
          query,
          categoryPath: selectedCategory.Path,
          isInventoryType: selectedCategory.IsInventoryType,
          // Legacy arg kept for backend compatibility with older builds.
          blueprintHint: selectedCategory.Path,
        }
      )
    } catch (e) {
      pickerError = String(e)
    } finally {
      pickerLoading = false
    }
  }

  function onPickerSearchInput() {
    if (pickerDebounce) clearTimeout(pickerDebounce)
    pickerDebounce = setTimeout(() => runPickerSearch(pickerSearch), 250)
  }

  async function selectPickerItem(path: string) {
    if (pickerSelectedPath === path) {
      pickerSelectedPath = ''
      pickerSelectedId   = ''
      return
    }
    pickerSelectedPath = path
    pickerSelectedId   = ''
    pickerIdError      = ''
    pickerIdLoading    = true
    try {
      pickerSelectedId = await invoke<string>('lookup_prototype_id', {
        serverExe,
        prototypePath: path,
      })
    } catch (e) {
      pickerIdError = String(e)
    } finally {
      pickerIdLoading = false
    }
  }

  function addPickerItem() {
    if (!pickerSelectedId || !pickerSelectedPath) return

    const newItem: GuidItem = {
      PrototypeGuid: 0,
      ItemPrototypeRuntimeIdForClient: pickerSelectedId,
      Quantity: Math.max(1, pickerQty),
    }
    const insertIdx = offerType === 'bogo' && bogoTarget === 'additional'
      ? formAdditionalGuidItems.length
      : formGuidItems.length

    const capturedPath = pickerSelectedPath
    const capturedId   = pickerSelectedId

    if (offerType === 'bogo' && bogoTarget === 'additional') {
      formAdditionalGuidItems = [...formAdditionalGuidItems, newItem]
      additionalGuidNames     = [...additionalGuidNames, capturedPath]
      addedAdditionalPaths.add(capturedPath)
      addedAdditionalPaths = addedAdditionalPaths
    } else {
      formGuidItems = [...formGuidItems, newItem]
      guidNames     = [...guidNames, capturedPath]
      addedMainPaths.add(capturedPath)
      addedMainPaths = addedMainPaths
    }

    // Resolve display name asynchronously and back-fill the name slot.
    invoke<string>('resolve_display_name', {
      serverExe,
      prototypeRuntimeId: capturedId,
    }).then(name => {
      if (offerType === 'bogo' && bogoTarget === 'additional') {
        additionalGuidNames = additionalGuidNames.map((n, i) =>
          i === insertIdx ? name : n
        )
      } else {
        guidNames = guidNames.map((n, i) => i === insertIdx ? name : n)
      }
    }).catch(() => {})

    pickerSelectedPath = ''
    pickerSelectedId   = ''
    pickerQty          = 1
  }

  // ── Entry assembly ────────────────────────────────────────────────────────

  function buildEntry(): CatalogEntry {
    const order    = formOrder
    const isBundle = offerType === 'bundle' || offerType === 'bogo'
    return {
      SkuId: workingSku,
      GuidItems: cloneGuidItems(formGuidItems),
      AdditionalGuidItems: offerType === 'bogo' ? cloneGuidItems(formAdditionalGuidItems) : [],
      LocalizedEntries: [{
        LanguageId:   'en_us',
        Description:  formDesc,
        Title:        formTitle,
        ReleaseDate:  initialState.releaseDate,
        ItemPrice:    formPrice,
      }],
      InfoUrls: isBundle && formInfoUrl
        ? [{ LanguageId: 'en_us', Url: formInfoUrl, ImageData: '' }]
        : (entry?.InfoUrls ?? []),
      ContentData: isBundle && formContentUrl
        ? [{ LanguageId: 'en_us', Url: formContentUrl, ImageData: '' }]
        : (entry?.ContentData ?? []),
      Type: { Name: formTypeName, Order: order },
      TypeModifiers: buildModifiers([...selectedModifiers], order),
    }
  }

  // ── Dirty detection ───────────────────────────────────────────────────────

  $: currentStateSerialized = serializeEditableState({
    title: formTitle,
    description: formDesc,
    price: formPrice,
    releaseDate: initialState.releaseDate,
    typeName: formTypeName,
    typeOrder: formOrder,
    infoUrl: formInfoUrl,
    contentUrl: formContentUrl,
    guidItems: formGuidItems,
    additionalGuidItems: formAdditionalGuidItems,
    modifiers: [...selectedModifiers].sort(),
  })
  $: dirty = isNew || currentStateSerialized !== initialStateSerialized

  // ── Save ──────────────────────────────────────────────────────────────────

  let saving      = false
  let saveError   = ''
  let saveSuccess = false

  async function save() {
    if (!formTitle.trim()) { saveError = 'Title is required.'; return }
    saveError   = ''
    saveSuccess = false
    saving      = true
    try {
      const built  = buildEntry()
      const target = isNew ? targetFile : entry!.source_file
      await invoke('save_catalog_entry', { serverExe, entry: built, targetFile: target })
      const meta: CatalogEntryWithMeta = { ...built, source_file: target, from_modified: true }
      initialStateSerialized = currentStateSerialized
      saveSuccess = true
      setTimeout(() => (saveSuccess = false), 3000)
      onSaved(meta)
      if (isNew) onClose()
    } catch (e) {
      saveError = String(e)
    } finally {
      saving = false
    }
  }

  // ── Delete ────────────────────────────────────────────────────────────────

  let deleteConfirming = false
  let deleting         = false
  let deleteError      = ''

  async function confirmDelete() {
    if (!entry) return
    deleting     = true
    deleteError  = ''
    try {
      await invoke('delete_catalog_entry', {
        serverExe,
        skuId:        entry.SkuId,
        sourceFile:   entry.source_file,
        fromModified: entry.from_modified,
      })
      onDeleted(entry.SkuId)
      onClose()
    } catch (e) {
      deleteError      = String(e)
      deleteConfirming = false
    } finally {
      deleting = false
    }
  }

  // ── Generate HTML ─────────────────────────────────────────────────────────

  let generating    = false
  let generatePath  = ''
  let generateError = ''

  async function generateHtml() {
    generating    = true
    generatePath  = ''
    generateError = ''
    try {
      generatePath = await invoke<string>('generate_bundle_html', {
        serverExe,
        entry:     buildEntry(),
        outputDir: htmlOutputDir,
      })
    } catch (e) {
      generateError = String(e)
    } finally {
      generating = false
    }
  }

  // ── Close guard ───────────────────────────────────────────────────────────

  let closeBlocked = false

  function tryClose() {
    if (dirty && !isNew) closeBlocked = true
    else onClose()
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      if (deleteConfirming) { deleteConfirming = false; return }
      tryClose()
    }
  }

  // ── Mount ─────────────────────────────────────────────────────────────────

  onMount(async () => {
    if (isNew) {
      try { workingSku = await invoke<number>('get_next_sku_id', { serverExe }) } catch {}
    }
    if (formGuidItems.length)           resolveGuidNames()
    if (formAdditionalGuidItems.length) resolveAdditionalGuidNames()
  })
</script>

<svelte:window on:keydown={onKeydown} />

<div
  class="modal-backdrop"
  role="presentation"
  on:click={e => { if (e.target === e.currentTarget) tryClose() }}
  on:keydown={onKeydown}
>
  <div class="editor-modal" role="dialog" aria-modal="true" tabindex="-1">

    <!-- ── Header ── -->
    <div class="modal-header">
      <div class="modal-title">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M6 2L3 6v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V6l-3-4z"/>
          <line x1="3" y1="6" x2="21" y2="6"/>
          <path d="M16 10a4 4 0 0 1-8 0"/>
        </svg>
        <span class="modal-display-name">
          {isNew ? 'New Catalog Entry' : (formTitle || 'Edit Entry')}
        </span>
        {#if !isNew}
          <span class="meta-badge">SKU {entry?.SkuId}</span>
          <span class="meta-badge dim">{entry?.source_file}</span>
        {/if}
      </div>
      <div class="modal-header-actions">
        <button class="close-btn" aria-label="Close" on:click={tryClose}>
          <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5">
            <line x1="3.5" y1="3.5" x2="10.5" y2="10.5"/>
            <line x1="10.5" y1="3.5" x2="3.5" y2="10.5"/>
          </svg>
        </button>
      </div>
    </div>

    <!-- ── Two-column body ── -->
    <div class="modal-body">

      <!-- ── Left pane: form fields ── -->
      <div class="left-pane">

        <!-- Offer type -->
        <div class="form-section">
          <div class="section-label">Offer Type</div>
          <div class="offer-type-row">
            {#each [['single', 'Single Item'], ['bundle', 'Bundle'], ['bogo', 'BOGO']] as [ot, label] (ot)}
              <label class="offer-chip" class:active={offerType === ot}>
                <input
                  type="radio"
                  name="offer-type"
                  value={ot}
                  checked={offerType === ot}
                  on:change={() => setOfferType(ot as OfferType)}
                >
                {label}
              </label>
            {/each}
          </div>
        </div>

        <!-- Item details -->
        <div class="form-section">
          <div class="section-label">Item Details</div>
          <div class="form-stack">

            <div class="field">
              <label class="field-label" for="f-title">Title</label>
              <input
                id="f-title" class="field-input" type="text"
                bind:value={formTitle}
                placeholder="Display name shown in the store"
                spellcheck="false" autocomplete="off"
              >
            </div>

            <div class="field">
              <label class="field-label" for="f-desc">Description</label>
              <input
                id="f-desc" class="field-input" type="text"
                bind:value={formDesc}
                placeholder="Short description"
                spellcheck="false" autocomplete="off"
              >
            </div>

            <div class="form-row">
              <div class="field">
                <label class="field-label" for="f-type">Type</label>
                {#if offerType === 'single'}
                  <select id="f-type" class="field-select" bind:value={formTypeName}>
                    {#each typeNames().filter(t => t !== 'Bundle') as t}
                      <option value={t}>{t}</option>
                    {/each}
                  </select>
                {:else}
                  <input class="field-input readonly" value="Bundle" readonly tabindex="-1">
                {/if}
              </div>

              <div class="field">
                <label class="field-label" for="f-order">
                  Order
                  <span class="label-hint" title="1 = Top Priority, 2 = Alphabetical, 3->999 = After Alphabetical">?</span>
                </label>
                <input
                  id="f-order" class="field-input" type="number"
                  bind:value={formOrder}
                  min="0" step="1"
                >
              </div>
            </div>

            <div class="form-row">
              <div class="field">
                <label class="field-label" for="f-price">Price (G)</label>
                <input
                  id="f-price" class="field-input" type="number"
                  bind:value={formPrice}
                  min="1" step="1"
                >
              </div>

              <div class="field">
                <label class="field-label">SKU</label>
                <input
                  class="field-input readonly" type="text"
                  value={workingSku}
                  readonly tabindex="-1"
                >
              </div>
            </div>

          </div>
        </div>

        <!-- Type modifiers -->
        <div class="form-section">
          <div class="section-label">Type Modifiers</div>
          {#if availableModifiers.length || unsupportedSelectedModifiers.length}
            <div class="modifier-row">
              {#each availableModifiers as mod}
                <label class="mod-chip">
                  <input
                    type="checkbox"
                    checked={selectedModifiers.has(mod)}
                    on:change={() => toggleModifier(mod)}
                  >
                  <span>{mod}</span>
                </label>
              {/each}

              {#each unsupportedSelectedModifiers as mod}
                <label class="mod-chip unsupported">
                  <input
                    type="checkbox"
                    checked={selectedModifiers.has(mod)}
                    on:change={() => toggleModifier(mod)}
                  >
                  <span>{mod}</span>
                  <span class="mod-chip-tag">Existing</span>
                </label>
              {/each}
            </div>

            {#if unsupportedSelectedModifiers.length}
              <div class="modifier-note">
                Existing modifiers that are not in the preset list for the current type are preserved unless you remove them.
              </div>
            {/if}
          {:else}
            <span class="empty-note">No predefined modifiers for this type.</span>
          {/if}
        </div>

        <!-- Store page URLs (bundle / bogo only) -->
        {#if offerType === 'bundle' || offerType === 'bogo'}
          <div class="form-section">
            <div class="section-label">Store Page URLs</div>
            <div class="form-stack">
              <div class="field">
                <label class="field-label" for="f-info-url">Info Page URL</label>
                <input
                  id="f-info-url" class="field-input" type="text"
                  bind:value={formInfoUrl}
                  placeholder="http://storecdn.marvelheroes.com/cdn/en_us/bundles/..."
                  spellcheck="false" autocomplete="off"
                >
              </div>
              <div class="field">
                <label class="field-label" for="f-thumb-url">Thumbnail URL</label>
                <input
                  id="f-thumb-url" class="field-input" type="text"
                  bind:value={formContentUrl}
                  placeholder="http://storecdn.marvelheroes.com/bundles/MTX_Store_Bundle_..."
                  spellcheck="false" autocomplete="off"
                >
              </div>
            </div>
          </div>
        {/if}

        <!-- Save to (create mode only) -->
        {#if isNew}
          <div class="form-section">
            <div class="section-label">Save To</div>
            {#if catalogFiles.length}
              <select class="field-select" bind:value={targetFile}>
                {#each catalogFiles as f}
                  <option value={f}>{f}</option>
                {/each}
              </select>
            {:else}
              <span class="empty-note">No catalog files found in MTXStore directory.</span>
            {/if}
          </div>
        {/if}

      </div><!-- /left-pane -->

      <!-- ── Right pane: items + picker ── -->
      <div class="right-pane">

        <!-- ── Added items ── -->
        <div class="items-section">

          <!-- BOGO add-target toggle -->
          {#if offerType === 'bogo'}
            <div class="bogo-target-row">
              <span class="bogo-target-label">Adding to:</span>
              <button
                class="target-btn"
                class:active={bogoTarget === 'main'}
                on:click={() => bogoTarget = 'main'}
              >Purchase Items</button>
              <button
                class="target-btn"
                class:active={bogoTarget === 'additional'}
                on:click={() => bogoTarget = 'additional'}
              >Bonus Items</button>
            </div>
          {/if}

          <!-- GuidItems (purchase / main) -->
          <div
            class="items-subsection"
            class:bogo-target-active={offerType === 'bogo' && bogoTarget === 'main'}
          >
            <div class="items-subsection-header">
              <span class="items-subsection-title">
                {offerType === 'bogo' ? 'Purchase Items' : 'Items'}
              </span>
              {#if formGuidItems.length}
                <span class="section-count">{formGuidItems.length}</span>
              {/if}
            </div>
            {#if formGuidItems.length}
              <table class="items-table">
                <thead>
                  <tr>
                    <th class="col-name">Prototype</th>
                    <th class="col-qty">Qty</th>
                    <th class="col-del"></th>
                  </tr>
                </thead>
                <tbody>
                  {#each formGuidItems as item, i}
                    <tr>
                      <td class="col-name" title={item.ItemPrototypeRuntimeIdForClient}>
                        <span class="item-name">{guidNames[i] ?? item.ItemPrototypeRuntimeIdForClient}</span>
                      </td>
                      <td class="col-qty">
                        <input
                          type="number" class="qty-input"
                          value={item.Quantity} min="1" step="1"
                          on:change={e => updateItemQty(i, (e.target as HTMLInputElement).value)}
                        >
                      </td>
                      <td class="col-del">
                        <button class="del-btn" aria-label="Remove item" on:click={() => removeGuidItem(i)}>
                          <svg viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.5">
                            <line x1="2" y1="2" x2="8" y2="8"/>
                            <line x1="8" y1="2" x2="2" y2="8"/>
                          </svg>
                        </button>
                      </td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            {:else}
              <div class="empty-note">No items added yet.</div>
            {/if}
          </div>

          <!-- AdditionalGuidItems (BOGO bonus) -->
          {#if offerType === 'bogo'}
            <div
              class="items-subsection bogo-bonus"
              class:bogo-target-active={bogoTarget === 'additional'}
            >
              <div class="items-subsection-header">
                <span class="items-subsection-title">Bonus Items</span>
                {#if formAdditionalGuidItems.length}
                  <span class="section-count">{formAdditionalGuidItems.length}</span>
                {/if}
              </div>
              {#if formAdditionalGuidItems.length}
                <table class="items-table">
                  <thead>
                    <tr>
                      <th class="col-name">Prototype</th>
                      <th class="col-qty">Qty</th>
                      <th class="col-del"></th>
                    </tr>
                  </thead>
                  <tbody>
                    {#each formAdditionalGuidItems as item, i}
                      <tr>
                        <td class="col-name" title={item.ItemPrototypeRuntimeIdForClient}>
                          <span class="item-name">{additionalGuidNames[i] ?? item.ItemPrototypeRuntimeIdForClient}</span>
                        </td>
                        <td class="col-qty">
                          <input
                            type="number" class="qty-input"
                            value={item.Quantity} min="1" step="1"
                            on:change={e => updateAdditionalItemQty(i, (e.target as HTMLInputElement).value)}
                          >
                        </td>
                        <td class="col-del">
                          <button class="del-btn" aria-label="Remove item" on:click={() => removeAdditionalGuidItem(i)}>
                            <svg viewBox="0 0 10 10" fill="none" stroke="currentColor" stroke-width="1.5">
                              <line x1="2" y1="2" x2="8" y2="8"/>
                              <line x1="8" y1="2" x2="2" y2="8"/>
                            </svg>
                          </button>
                        </td>
                      </tr>
                    {/each}
                  </tbody>
                </table>
              {:else}
                <div class="empty-note">No bonus items added yet.</div>
              {/if}
            </div>
          {/if}

        </div><!-- /items-section -->

        <!-- ── Item picker ── -->
        <div class="picker-section">

          <!-- Category list -->
          <div class="category-list">
            <div class="category-list-header">Categories</div>
            {#each ITEM_CATEGORIES as cat}
              <button
                class="category-item"
                class:active={selectedCategory?.DisplayName === cat.DisplayName}
                on:click={() => loadCategory(cat)}
              >{cat.DisplayName}</button>
            {/each}
          </div>

          <!-- Search + results + add row -->
          <div class="picker-right">

            <div class="picker-toolbar">
              <input
                class="picker-search"
                type="text"
                placeholder={selectedCategory
                  ? `Search in ${selectedCategory.DisplayName}…`
                  : 'Select a category first…'}
                bind:value={pickerSearch}
                on:input={onPickerSearchInput}
                disabled={!selectedCategory}
                spellcheck="false"
                autocomplete="off"
              >
              <label class="picker-check">
                <input type="checkbox" bind:checked={hideAddedItems}>
                <span>Hide added items</span>
              </label>
              <label class="picker-check">
                <input type="checkbox" bind:checked={hideProtoPaths}>
                <span>Hide paths</span>
              </label>
            </div>

            <div class="picker-results">
              {#if !selectedCategory}
                <div class="picker-empty">Select a category on the left to browse prototypes.</div>
              {:else if pickerLoading}
                <div class="picker-empty">Searching…</div>
              {:else if pickerError}
                <div class="picker-empty error">{pickerError}</div>
              {:else if filteredPickerResults.length === 0 && pickerResults.length > 0}
                <div class="picker-empty">All results hidden — uncheck filters to show.</div>
              {:else if filteredPickerResults.length === 0}
                <div class="picker-empty">
                  {selectedCategory ? 'No results — type to search within this category.' : ''}
                </div>
              {:else}
                {#each filteredPickerResults as result}
                  <button
                    class="picker-result"
                    class:selected={pickerSelectedPath === result.path}
                    on:click={() => selectPickerItem(result.path)}
                  >
                    {#if hideProtoPaths}
                      <span class="result-name">{formatPickerResultCompactLabel(result)}</span>
                      {#if hasFriendlyDisplayName(result)}
                        <span class="result-bp">{result.leaf}</span>
                      {/if}
                    {:else}
                      <span class="result-path">{formatPickerResultLabel(result)}</span>
                      <span class="result-bp">{result.blueprint}</span>
                    {/if}
                  </button>
                {/each}
              {/if}
            </div>

            <div class="picker-add-row">
              <div class="picker-selected-info">
                {#if pickerIdLoading}
                  <span class="picker-status dim">Resolving ID…</span>
                {:else if pickerIdError}
                  <span class="picker-status error">{pickerIdError}</span>
                {:else if pickerSelectedPath}
                  <span
                    class="picker-status"
                    class:warn={pickerSelectedIsExisting}
                    title={pickerSelectedIsExisting
                      ? 'This prototype is already present in another catalog entry.'
                      : pickerSelectedId}
                  >
                    {selectedPickerLabel()}
                    {#if pickerSelectedIsExisting}
                      <span class="picker-status-flag">already in catalog</span>
                    {/if}
                  </span>
                {:else}
                  <span class="picker-status dim">Click a result to select</span>
                {/if}
              </div>
              <input
                class="qty-input"
                type="number" min="1" step="1"
                bind:value={pickerQty}
                placeholder="1"
              >
              <button
                class="btn btn-sm btn-accent"
                on:click={addPickerItem}
                disabled={!pickerSelectedId || pickerIdLoading}
              >
                {#if offerType === 'bogo'}
                  {bogoTarget === 'main' ? 'Add Purchase Item' : 'Add Bonus Item'}
                {:else}
                  Add Item
                {/if}
              </button>
            </div>

          </div><!-- /picker-right -->

        </div><!-- /picker-section -->

      </div><!-- /right-pane -->

    </div><!-- /modal-body -->

    <!-- ── Footer ── -->
    <div class="modal-footer">

      {#if closeBlocked}
        <span class="feedback-error">Unsaved changes — save or discard before closing.</span>
        <div class="footer-actions">
          <button class="btn btn-sm btn-red" on:click={onClose}>Discard & Close</button>
          <button
            class="btn btn-sm btn-accent btn-pulse"
            on:click={save}
            disabled={saving}
          >{saving ? 'Saving…' : 'Save & Close'}</button>
        </div>

      {:else}
        <div class="footer-left">
          {#if deleteError}
            <span class="feedback-error">{deleteError}</span>
          {:else if saveError}
            <span class="feedback-error">{saveError}</span>
          {:else if generateError}
            <span class="feedback-error">{generateError}</span>
          {:else if generatePath}
            <span class="feedback-ok" title={generatePath}>HTML written</span>
          {:else if saveSuccess}
            <span class="feedback-ok">Saved</span>
          {:else if dirty && !isNew}
            <span class="dirty-badge">Unsaved changes</span>
          {/if}
        </div>

        <div class="footer-actions">
          {#if !isNew && (offerType === 'bundle' || offerType === 'bogo')}
            <button
              class="btn btn-sm btn-outline"
              on:click={generateHtml}
              disabled={generating}
              title="Generate HTML store page and write store.css"
            >{generating ? 'Generating…' : 'Generate HTML'}</button>
          {/if}

          {#if !isNew}
            {#if deleteConfirming}
              <button class="btn btn-sm btn-outline" on:click={() => deleteConfirming = false}>Cancel</button>
              <button
                class="btn btn-sm btn-red"
                on:click={confirmDelete}
                disabled={deleting}
              >{deleting ? 'Deleting…' : 'Confirm Delete'}</button>
            {:else}
              <button
                class="btn btn-sm btn-outline"
                style="color: var(--text-error);"
                on:click={() => deleteConfirming = true}
              >Delete</button>
            {/if}
          {/if}

          <button class="btn btn-sm btn-outline" on:click={tryClose}>
            {isNew ? 'Cancel' : 'Close'}
          </button>
          <button
            class="btn btn-sm btn-accent"
            class:btn-pulse={dirty}
            on:click={save}
            disabled={saving || (!isNew && !dirty)}
          >{saving ? 'Saving…' : isNew ? 'Create' : 'Save'}</button>
        </div>
      {/if}

    </div><!-- /modal-footer -->

  </div>
</div>

<style>
  /* ── Backdrop & shell ── */
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.72);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: var(--z-modal);
    backdrop-filter: blur(3px);
  }

  .editor-modal {
    background: var(--bg-1);
    border: 1px solid var(--border-lit);
    border-radius: var(--radius-md);
    width: min(1440px, 96vw);
    height: min(820px, 92vh);
    margin-top: 38px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    box-shadow:
      0 32px 100px rgba(0, 0, 0, 0.6),
      0 0 0 1px rgba(255, 255, 255, 0.03) inset;
  }

  /* ── Header ── */
  .modal-header {
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-shrink: 0;
    gap: 12px;
    min-height: 52px;
  }

  .modal-title {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .modal-title svg { width: 16px; height: 16px; color: var(--accent); flex-shrink: 0; }

  .modal-display-name {
    font-family: var(--font-head);
    font-size: 13px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-0);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 400px;
  }

  .meta-badge {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-2);
    background: var(--bg-3);
    border: 1px solid var(--border-mid);
    padding: 2px 7px;
    border-radius: 2px;
    white-space: nowrap;
    flex-shrink: 0;
  }
  .meta-badge.dim { color: var(--text-3); }

  .modal-header-actions { display: flex; align-items: center; gap: 8px; flex-shrink: 0; }

  .close-btn {
    width: 28px; height: 28px;
    display: flex; align-items: center; justify-content: center;
    background: transparent;
    border: 1px solid transparent;
    color: var(--text-2);
    cursor: pointer;
    border-radius: var(--radius-sm);
    transition: all 0.12s;
  }
  .close-btn:hover { color: var(--text-0); background: var(--bg-3); border-color: var(--border-mid); }
  .close-btn svg { width: 14px; height: 14px; }

  /* ── Two-column body ── */
  .modal-body {
    flex: 1;
    display: flex;
    flex-direction: row;
    overflow: hidden;
    min-height: 0;
  }

  /* ── Left pane ── */
  .left-pane {
    flex: 0 0 30%;
    max-width: 30%;
    overflow-y: auto;
    border-right: 1px solid var(--border);
  }

  /* ── Right pane ── */
  .right-pane {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    min-height: 0;
  }

  /* ── Added items section ── */
  .items-section {
    flex-shrink: 0;
    max-height: 280px;
    overflow-y: auto;
    border-bottom: 1px solid var(--border);
  }

  .bogo-target-row {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border-bottom: 1px solid var(--border);
    background: var(--bg-0);
    flex-shrink: 0;
  }

  .bogo-target-label {
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-3);
    flex-shrink: 0;
  }

  .target-btn {
    font-family: var(--font-head);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-2);
    background: var(--bg-2);
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    padding: 3px 10px;
    cursor: pointer;
    transition: all 0.1s;
  }
  .target-btn:hover { border-color: var(--border-lit); color: var(--text-1); }
  .target-btn.active {
    background: var(--accent-glow);
    border-color: var(--accent-dim);
    color: var(--accent-bright);
  }

  .items-subsection {
    padding: 8px 12px;
    border-left: 2px solid transparent;
    transition: border-color 0.1s;
  }
  .items-subsection.bogo-bonus { border-top: 1px solid var(--border); }
  .items-subsection.bogo-target-active { border-left-color: var(--accent-dim); }

  .items-subsection-header {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 6px;
  }

  .items-subsection-title {
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-3);
  }

  /* ── Item picker section ── */
  .picker-section {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: row;
    overflow: hidden;
  }

  .category-list {
    flex: 0 0 160px;
    border-right: 1px solid var(--border);
    overflow-y: auto;
    display: flex;
    flex-direction: column;
  }

  .category-list-header {
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-3);
    padding: 8px 10px 6px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .category-item {
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    border-bottom: 1px solid var(--border);
    padding: 7px 10px;
    font-family: var(--font-body);
    font-size: 12px;
    color: var(--text-1);
    cursor: pointer;
    transition: background 0.08s, color 0.08s;
  }
  .category-item:last-child { border-bottom: none; }
  .category-item:hover { background: var(--bg-2); color: var(--text-0); }
  .category-item.active {
    background: var(--accent-glow);
    color: var(--accent-bright);
  }

  .picker-right {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    overflow: hidden;
  }

  .picker-toolbar {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border-bottom: 1px solid var(--border);
    background: var(--bg-0);
  }

  .picker-search {
    flex: 1;
    background: var(--bg-1);
    border: 1px solid var(--border-mid);
    color: var(--text-0);
    font-family: var(--font-mono);
    font-size: 12px;
    padding: 5px 8px;
    border-radius: var(--radius-sm);
    outline: none;
    transition: border-color 0.12s;
    min-width: 0;
  }
  .picker-search:focus { border-color: var(--accent-dim); }
  .picker-search::placeholder { color: var(--text-3); font-family: var(--font-body); }
  .picker-search:disabled { opacity: 0.4; cursor: not-allowed; }

  .picker-check {
    display: flex;
    align-items: center;
    gap: 5px;
    font-family: var(--font-body);
    font-size: 11px;
    color: var(--text-2);
    cursor: pointer;
    white-space: nowrap;
    user-select: none;
  }
  .picker-check input[type="checkbox"] { accent-color: var(--accent); cursor: pointer; }

  .picker-results {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
  }

  .picker-empty {
    padding: 20px 16px;
    font-family: var(--font-head);
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
    text-align: center;
  }
  .picker-empty.error { color: var(--text-error); text-transform: none; font-family: var(--font-body); font-size: 11px; }

  .picker-result {
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    border-bottom: 1px solid var(--border);
    padding: 5px 10px;
    cursor: pointer;
    transition: background 0.06s;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .picker-result:last-child { border-bottom: none; }
  .picker-result:hover { background: var(--bg-2); }
  .picker-result.selected { background: var(--accent-glow); }

  .result-path {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-0);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .result-name {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-0);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .result-bp {
    font-family: var(--font-head);
    font-size: 9px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--accent-dim);
  }

  .picker-add-row {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border-top: 1px solid var(--border);
    background: var(--bg-0);
  }

  .picker-selected-info {
    flex: 1;
    min-width: 0;
    overflow: hidden;
  }

  .picker-status {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-1);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .picker-status.dim  { color: var(--text-3); font-family: var(--font-body); }
  .picker-status.error { color: var(--text-error); }
  .picker-status.warn { color: var(--amber-bright); }

  .picker-status-flag {
    font-family: var(--font-head);
    font-size: 9px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--amber-bright);
    background: var(--amber-dim);
    border: 1px solid rgba(200,146,10,0.35);
    padding: 1px 5px;
    border-radius: 2px;
    flex-shrink: 0;
  }

  /* ── Form sections (shared between both panes) ── */
  .form-section {
    padding: 12px 14px;
    border-bottom: 1px solid var(--border);
  }
  .form-section:last-child { border-bottom: none; }

  .section-label {
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--text-3);
    margin-bottom: 10px;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .section-count {
    background: var(--bg-3);
    border: 1px solid var(--border-mid);
    color: var(--text-2);
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0;
    text-transform: none;
    padding: 1px 6px;
    border-radius: 2px;
  }

  /* ── Offer type ── */
  .offer-type-row {
    display: flex;
    gap: 6px;
  }

  .offer-chip {
    display: flex;
    align-items: center;
    gap: 5px;
    font-family: var(--font-head);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-2);
    background: var(--bg-2);
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    padding: 4px 10px;
    cursor: pointer;
    transition: all 0.1s;
    user-select: none;
  }
  .offer-chip:hover { border-color: var(--border-lit); color: var(--text-1); }
  .offer-chip.active {
    background: var(--accent-glow);
    border-color: var(--accent-dim);
    color: var(--accent-bright);
  }
  .offer-chip input[type="radio"] { display: none; }

  /* ── Form layout ── */
  .form-stack { display: flex; flex-direction: column; gap: 8px; }
  .form-row   { display: flex; gap: 8px; }
  .form-row .field { flex: 1; min-width: 0; }

  .field { display: flex; flex-direction: column; gap: 4px; }

  .field-label {
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-3);
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .label-hint {
    width: 13px; height: 13px;
    display: inline-flex; align-items: center; justify-content: center;
    background: var(--bg-3);
    border: 1px solid var(--border-mid);
    border-radius: 50%;
    font-size: 9px;
    color: var(--text-3);
    cursor: default;
    flex-shrink: 0;
  }

  .field-input {
    background: var(--bg-0);
    border: 1px solid var(--border-mid);
    color: var(--text-0);
    font-family: var(--font-mono);
    font-size: 12px;
    padding: 6px 8px;
    border-radius: var(--radius-sm);
    outline: none;
    transition: border-color 0.12s;
    width: 100%;
  }
  .field-input:focus { border-color: var(--accent-dim); }
  .field-input.readonly { color: var(--text-3); cursor: default; }
  .field-input::placeholder { color: var(--text-3); font-family: var(--font-body); font-size: 12px; }

  .field-select {
    background: var(--bg-0);
    border: 1px solid var(--border-mid);
    color: var(--text-1);
    font-family: var(--font-body);
    font-size: 12px;
    padding: 6px 8px;
    border-radius: var(--radius-sm);
    outline: none;
    cursor: pointer;
    transition: border-color 0.12s;
    width: 100%;
  }
  .field-select:focus { border-color: var(--accent-dim); }

  /* ── Type modifiers ── */
  .modifier-row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .mod-chip {
    display: flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
    font-family: var(--font-body);
    font-size: 11px;
    color: var(--text-1);
    padding: 3px 8px;
    background: var(--bg-2);
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    transition: border-color 0.1s;
    user-select: none;
  }
  .mod-chip:hover { border-color: var(--border-lit); }
  .mod-chip input[type="checkbox"] { accent-color: var(--accent); cursor: pointer; }

  .mod-chip.unsupported {
    border-style: dashed;
    border-color: rgba(200,146,10,0.35);
    background: rgba(200,146,10,0.08);
    color: var(--amber-bright);
  }

  .mod-chip-tag {
    font-family: var(--font-head);
    font-size: 9px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--amber-bright);
    opacity: 0.9;
  }

  .modifier-note {
    margin-top: 8px;
    font-size: 11px;
    color: var(--text-2);
    line-height: 1.45;
  }

  /* ── Items table (shared by both item lists) ── */
  .items-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    overflow: hidden;
    margin-bottom: 4px;
  }

  .items-table thead th {
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-3);
    padding: 5px 10px;
    border-bottom: 1px solid var(--border);
    text-align: left;
    background: var(--bg-2);
  }

  .items-table tbody tr { border-bottom: 1px solid var(--border); transition: background 0.08s; }
  .items-table tbody tr:last-child { border-bottom: none; }
  .items-table tbody tr:hover { background: var(--bg-2); }
  .items-table td { padding: 5px 10px; vertical-align: middle; }

  .col-name { width: auto; }
  .col-qty  { width: 72px; }
  .col-del  { width: 36px; }

  .item-name {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-1);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    display: block;
    max-width: 500px;
  }

  .qty-input {
    width: 56px;
    background: var(--bg-0);
    border: 1px solid var(--border-mid);
    color: var(--text-0);
    font-family: var(--font-mono);
    font-size: 12px;
    padding: 3px 6px;
    border-radius: var(--radius-sm);
    outline: none;
    transition: border-color 0.12s;
  }
  .qty-input:focus { border-color: var(--accent-dim); }

  .del-btn {
    width: 22px; height: 22px;
    display: flex; align-items: center; justify-content: center;
    background: none; border: 1px solid transparent;
    border-radius: var(--radius-sm); color: var(--text-3);
    cursor: pointer; transition: all 0.1s;
  }
  .del-btn:hover { border-color: rgba(192,57,43,0.4); color: var(--text-error); background: var(--red-dim); }
  .del-btn svg { width: 10px; height: 10px; }

  .empty-note {
    font-family: var(--font-head);
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
  }

  /* ── Footer ── */
  .modal-footer {
    padding: 10px 16px;
    border-top: 1px solid var(--border);
    background: rgba(8, 9, 12, 0.3);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    flex-shrink: 0;
    min-height: 50px;
  }

  .footer-left    { display: flex; align-items: center; gap: 10px; min-width: 0; }
  .footer-actions { display: flex; align-items: center; gap: 8px; flex-shrink: 0; margin-left: auto; }

  .feedback-error { font-size: 11px; color: var(--text-error); }
  .feedback-ok    { font-size: 11px; color: var(--text-success); }

  .dirty-badge {
    font-family: var(--font-head);
    font-size: 9px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-3);
  }
</style>