<script lang="ts">
  // A dropdown whose option list follows the theme. A native select's list is drawn by
  // Windows, whose hover colour CSS can't reach.
  // Callers size the trigger with CSS variables, e.g. <Dropdown --dd-width="100%" />:
  // --dd-width, --dd-max-width, --dd-min-width, --dd-pad, --dd-color, --dd-bg.
  import { onMount, tick } from 'svelte'

  export let value: string
  export let options: { value: string; label: string }[]
  export let label: string
  export let placeholder = 'Select'
  /** The title bar's quieter look: a fainter border and less padding. */
  export let subtle = false
  export let disabled = false
  export let id: string | undefined = undefined
  export let onChange: (value: string) => void

  let open = false
  let root: HTMLElement
  let menu: HTMLElement
  let menuPos = ''
  $: current = options.find(o => o.value === value)?.label

  // The menu is position: fixed so a scrolling panel or a modal body can't clip it.
  // It opens upward when there's more room above.
  async function toggle() {
    open = !open
    if (!open) return
    await tick()
    const r = root.getBoundingClientRect()
    const below = innerHeight - r.bottom - 12
    const above = r.top - 50 // clear of the 38px title bar
    const up = menu.offsetHeight > below && above > below
    menuPos = `right:${innerWidth - r.right}px;min-width:${r.width}px;max-height:${up ? above : below}px;`
      + (up ? `bottom:${innerHeight - r.top + 4}px` : `top:${r.bottom + 4}px`)
  }
  function pick(v: string) {
    open = false
    if (v !== value) onChange(v)
  }
  function closeOutside(e: MouseEvent) {
    if (open && !root.contains(e.target as Node)) open = false
  }
  // A fixed menu would drift from its trigger, so scrolling anything but the menu closes it.
  onMount(() => {
    const close = (e: Event) => { if (open && e.target !== menu) open = false }
    addEventListener('scroll', close, true)
    addEventListener('resize', close)
    return () => { removeEventListener('scroll', close, true); removeEventListener('resize', close) }
  })
</script>

<svelte:window on:click={closeOutside} on:keydown={(e) => e.key === 'Escape' && (open = false)} />

<div class="dd" bind:this={root}>
  <button {id} class="dd-btn" class:subtle class:placeholder={!current} aria-haspopup="listbox" aria-expanded={open}
    aria-label={label} {disabled} on:click={toggle}>
    <!-- Every label sits in one grid cell so the trigger is as wide as the longest, like a native select. -->
    <span class="dd-label">
      {#each options as o}<span class="dd-sizer" aria-hidden="true">{o.label}</span>{/each}
      <span class="dd-sizer" aria-hidden="true">{placeholder}</span>
      <span>{current ?? placeholder}</span>
    </span>
    <svg viewBox="0 0 10 10" width="10" height="10" fill="none" stroke="currentColor" stroke-width="1.5">
      <polyline points="2,3.5 5,6.5 8,3.5" />
    </svg>
  </button>
  {#if open}
    <div class="dd-menu" role="listbox" aria-label={label} bind:this={menu} style={menuPos}>
      {#each options as o (o.value)}
        <button class="dd-option" class:selected={o.value === value} role="option" aria-selected={o.value === value}
          on:click={() => pick(o.value)}>{o.label}</button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .dd {
    position: relative;
    display: inline-block;
    width: var(--dd-width, auto);
    max-width: var(--dd-max-width, none);
  }
  .dd-btn {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    width: 100%;
    min-width: var(--dd-min-width, 120px);
    background: var(--dd-bg, var(--bg-0));
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    color: var(--dd-color, var(--text-0));
    font-family: var(--font-body);
    font-size: 12px;
    padding: var(--dd-pad, 6px 10px);
    cursor: pointer;
    transition: border-color 0.12s;
  }
  .dd-btn.subtle { border-color: var(--border); color: var(--text-1); padding: 3px 8px; min-width: 140px; }
  .dd-btn.placeholder { color: var(--text-2); }
  .dd-btn:hover:not(:disabled), .dd-btn[aria-expanded="true"] { border-color: var(--border-lit); }
  .dd-btn.subtle:hover, .dd-btn.subtle[aria-expanded="true"] { border-color: var(--border-mid); }
  .dd-btn:focus-visible { outline: none; border-color: var(--accent-dim); }
  .dd-btn:disabled { opacity: 0.5; cursor: not-allowed; }
  .dd-btn svg { color: var(--text-3); flex-shrink: 0; }
  .dd-label { display: grid; text-align: left; white-space: nowrap; min-width: 0; }
  .dd-label > * { grid-area: 1 / 1; overflow: hidden; text-overflow: ellipsis; }
  .dd-sizer { visibility: hidden; }
  .dd-menu {
    position: fixed;
    z-index: calc(var(--z-modal) + 1);
    overflow-y: auto;
    padding: 4px;
    background: var(--bg-2);
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    display: flex;
    flex-direction: column;
  }
  .dd-option {
    flex-shrink: 0;
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
