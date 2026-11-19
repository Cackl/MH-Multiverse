<script lang="ts">
  // A dropdown whose option list follows the theme. A native select's list is drawn by
  // Windows, whose hover colour CSS can't reach.
  export let value: string
  export let options: { value: string; label: string }[]
  export let label: string
  export let placeholder = 'Select'
  /** The title bar's quieter look: a fainter border and less padding. */
  export let subtle = false
  export let onChange: (value: string) => void

  let open = false
  let root: HTMLElement
  $: current = options.find(o => o.value === value)?.label

  function pick(v: string) {
    open = false
    if (v !== value) onChange(v)
  }
  function closeOutside(e: MouseEvent) {
    if (open && !root.contains(e.target as Node)) open = false
  }
</script>

<svelte:window on:click={closeOutside} on:keydown={(e) => e.key === 'Escape' && (open = false)} />

<div class="dd" bind:this={root}>
  <button class="dd-btn" class:subtle class:placeholder={!current} aria-haspopup="listbox" aria-expanded={open}
    aria-label={label} on:click={() => open = !open}>
    {current ?? placeholder}
    <svg viewBox="0 0 10 10" width="10" height="10" fill="none" stroke="currentColor" stroke-width="1.5">
      <polyline points="2,3.5 5,6.5 8,3.5" />
    </svg>
  </button>
  {#if open}
    <div class="dd-menu" role="listbox" aria-label={label}>
      {#each options as o (o.value)}
        <button class="dd-option" class:selected={o.value === value} role="option" aria-selected={o.value === value}
          on:click={() => pick(o.value)}>{o.label}</button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .dd { position: relative; display: inline-block; }
  .dd-btn {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    min-width: 120px;
    background: var(--bg-0);
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    color: var(--text-0);
    font-family: var(--font-body);
    font-size: 12px;
    padding: 6px 10px;
    cursor: pointer;
    transition: border-color 0.12s;
  }
  .dd-btn.subtle { border-color: var(--border); color: var(--text-1); padding: 3px 8px; min-width: 140px; }
  .dd-btn.placeholder { color: var(--text-2); }
  .dd-btn:hover, .dd-btn[aria-expanded="true"] { border-color: var(--border-lit); }
  .dd-btn.subtle:hover, .dd-btn.subtle[aria-expanded="true"] { border-color: var(--border-mid); }
  .dd-btn:focus-visible { outline: none; border-color: var(--accent-dim); }
  .dd-btn svg { color: var(--text-3); flex-shrink: 0; }
  .dd-menu {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    min-width: 100%;
    z-index: 10;
    padding: 4px;
    background: var(--bg-2);
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    display: flex;
    flex-direction: column;
  }
  .dd-option {
    text-align: left;
    white-space: nowrap;
    background: none;
    border: none;
    border-radius: 2px;
    color: var(--text-1);
    font-family: var(--font-body);
    font-size: 12px;
    padding: 5px 8px;
    cursor: pointer;
  }
  .dd-option:hover, .dd-option:focus-visible { outline: none; background: var(--bg-3); color: var(--text-0); }
  .dd-option.selected { color: var(--accent); }
</style>
