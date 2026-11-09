<script lang="ts">
  import { onDestroy } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { createPrototypeSearch, dropdownStyle, type PrototypeMatch } from '../lib/prototypes'

  export let value: string
  export let serverExe: string
  export let placeholder = 'Prototype path (type to search)'
  export let id: string | undefined = undefined
  export let disabled = false
  export let multiline = false
  export let compact = false
  export let blueprintHint: string | undefined = undefined
  export let onInput: ((value: string) => void) | undefined = undefined
  export let onSelect: ((match: PrototypeMatch) => void) | undefined = undefined
  export let onEnter: (() => void) | undefined = undefined

  const search = createPrototypeSearch(query =>
    invoke<PrototypeMatch[]>('search_prototypes', { serverExe, query, blueprintHint })
  )
  const { state: searchState } = search

  let inputEl: HTMLInputElement | HTMLTextAreaElement
  let wrapEl: HTMLDivElement
  let dropdownEl: HTMLDivElement
  let dropdownPosition = ''
  let skipFocusSearch = false

  $: if (disabled) search.close()
  $: if ($searchState.open && inputEl) {
    dropdownPosition = dropdownStyle(inputEl.getBoundingClientRect(), window.innerWidth, window.innerHeight)
  }

  onDestroy(search.close)

  function handleInput(e: Event) {
    const text = (e.currentTarget as HTMLInputElement | HTMLTextAreaElement).value
    value = multiline ? text.replace(/[\r\n]/g, '') : text
    onInput?.(value)
    search.input(value)
  }

  function handleFocus() {
    if (!skipFocusSearch && !disabled) search.input(value)
  }

  function pick(match: PrototypeMatch) {
    value = match.path
    search.close()
    onSelect?.(match)
    skipFocusSearch = true
    inputEl.focus() // keyboard picks leave focus on an option that no longer exists
    skipFocusSearch = false
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && $searchState.open) {
      e.stopPropagation() // the modal closes on Escape otherwise
      search.close()
    } else if (e.key === 'Enter' && e.target === inputEl) {
      if (multiline) e.preventDefault()
      search.close()
      onEnter?.()
    }
  }

  function handleFocusOut(e: FocusEvent) {
    if (!wrapEl.contains(e.relatedTarget as Node | null)) search.close()
  }

  // Fixed position doesn't follow the modal body when it scrolls.
  function handleScroll(e: Event) {
    if ($searchState.open && !dropdownEl?.contains(e.target as Node)) search.close()
  }
</script>

<svelte:window on:scroll|capture={handleScroll} on:resize={search.close} />

<div class="picker" role="presentation" bind:this={wrapEl} on:keydown={handleKeydown} on:focusout={handleFocusOut}>
  {#if multiline}
    <textarea
      {id}
      class="picker-input"
      rows="2"
      {value}
      {placeholder}
      {disabled}
      autocomplete="off"
      spellcheck="false"
      bind:this={inputEl}
      on:input={handleInput}
      on:focus={handleFocus}
    ></textarea>
  {:else}
    <input
      {id}
      class="picker-input"
      class:compact
      type="text"
      {value}
      {placeholder}
      {disabled}
      autocomplete="off"
      spellcheck="false"
      bind:this={inputEl}
      on:input={handleInput}
      on:focus={handleFocus}
    />
  {/if}

  {#if $searchState.error}
    <div class="picker-hint">{$searchState.error}</div>
  {/if}

  {#if $searchState.open}
    <div class="picker-dropdown" role="presentation" style={dropdownPosition} bind:this={dropdownEl} on:mousedown|preventDefault>
      {#each $searchState.results as match (match.path)}
        <button class="picker-option" type="button" on:click={() => pick(match)}>
          <span class="picker-opt-name">{match.display_name || match.leaf}</span>
          <span class="picker-opt-path">{match.path}</span>
          <span class="picker-opt-bp">{match.blueprint}</span>
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .picker {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }

  .picker-input {
    width: 100%;
    background: var(--bg-0);
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    color: var(--text-0);
    font-family: var(--font-mono);
    font-size: 12px;
    padding: 6px 10px;
    outline: none;
    transition: border-color 0.12s;
  }
  .picker-input:focus { border-color: var(--accent-dim); }
  .picker-input:disabled { opacity: 0.45; cursor: not-allowed; }
  .picker-input.compact { padding: 5px 8px; }
  textarea.picker-input { font-size: 11px; line-height: 1.5; resize: none; }

  .picker-hint {
    font-size: var(--fs-min);
    color: var(--text-error);
  }

  .picker-dropdown {
    position: fixed;
    overflow-y: auto;
    background: var(--bg-2);
    border: 1px solid var(--border-lit);
    border-radius: var(--radius-sm);
    z-index: calc(var(--z-modal) + 10);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
  }

  .picker-option {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    border-bottom: 1px solid var(--border);
    padding: 6px 10px;
    cursor: pointer;
    transition: background 0.08s;
  }
  .picker-option:last-child { border-bottom: none; }
  .picker-option:hover,
  .picker-option:focus-visible { background: var(--bg-3); }

  .picker-opt-name,
  .picker-opt-path {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .picker-opt-name {
    font-family: var(--font-head);
    font-size: 11px;
    font-weight: 600;
    color: var(--text-0);
  }

  .picker-opt-path {
    font-family: var(--font-mono);
    font-size: var(--fs-min);
    color: var(--text-2);
  }

  .picker-opt-bp {
    font-family: var(--font-head);
    font-size: var(--fs-min);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--accent);
  }
</style>
