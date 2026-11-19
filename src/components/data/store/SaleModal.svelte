<script lang="ts">
  import { STORE_SECTIONS, saleLabel } from '../../../lib/storeSimple'

  /** How many items a sale with these settings would reprice. */
  export let countFor: (percent: number, sections: string[]) => number
  export let onStart: (percent: number, sections: string[]) => void
  export let onClose: () => void

  let percent = 25
  // Bundles start off: their pages print the original price, so a sale makes the page wrong.
  let sections: string[] = STORE_SECTIONS.map(s => s.type).filter(t => t !== 'Bundle')

  $: count = countFor(percent, sections)
  const toggle = (type: string) => sections = sections.includes(type) ? sections.filter(t => t !== type) : [...sections, type]
</script>

<svelte:window on:keydown={(e) => e.key === 'Escape' && onClose()} />

<div class="modal-backdrop" role="presentation" on:click={onClose}>
  <div class="modal" role="dialog" aria-modal="true" aria-labelledby="sale-title" tabindex="-1" on:click|stopPropagation on:keydown|stopPropagation>
    <div class="modal-header">
      <div class="modal-title">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M20.6 13.4 13.4 20.6a2 2 0 0 1-2.8 0L3 13V3h10l7.6 7.6a2 2 0 0 1 0 2.8z"/>
          <line x1="7" y1="7" x2="7.01" y2="7"/>
        </svg>
        <span class="modal-display-name" id="sale-title">Start a sale</span>
      </div>
      <button class="close-btn" aria-label="Close" on:click={onClose}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="18" y1="6" x2="6" y2="18"/>
          <line x1="6" y1="6" x2="18" y2="18"/>
        </svg>
      </button>
    </div>

    <div class="modal-body">
      <div class="field-group field-group-center">
        <span class="field-label">Percent off</span>
        <div class="field-control field-inline">
          <span class="range-track">
            <input type="range" aria-label="Percent off" min="5" max="100" step="5" bind:value={percent} style="--fill:{(percent - 5) / 95 * 100}%" />
          </span>
          <span class="sale-pct">{percent}%</span>
        </div>
      </div>

      <div class="field-group">
        <span class="field-label">On</span>
        <div class="field-control">
          <div class="sale-sections" role="group" aria-label="Sections on sale">
            {#each STORE_SECTIONS as s}
              <label class="check-row">
                <input type="checkbox" checked={sections.includes(s.type)} on:change={() => toggle(s.type)} />
                <span class="check-label">{s.label}</span>
              </label>
            {/each}
          </div>
          {#if sections.includes('Bundle')}
            <span class="field-hint">Bundle pages still show their original price. Players pay the sale price.</span>
          {/if}
        </div>
      </div>

      <div class="field-group">
        <span class="field-label">What happens</span>
        <div class="field-control">
          <span class="field-hint">
            {count === 0 ? 'Nothing to reprice.' : `${saleLabel(percent, sections)}: ${count} ${count === 1 ? 'item' : 'items'}${percent === 100 ? ' become free (0 G).' : ', rounded to 5 G.'}`}
            Hidden items stay as they are. End sale puts every price back.
          </span>
        </div>
      </div>
    </div>

    <div class="modal-footer">
      <button class="btn btn-sm btn-outline" on:click={onClose}>Cancel</button>
      <button class="btn btn-sm btn-accent" disabled={count === 0} on:click={() => onStart(percent, sections)}>Start sale</button>
    </div>
  </div>
</div>

<style>
  .modal {
    width: min(520px, 92vw);
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
  /* A row with a control, not a text box: centre the label on it. */
  .field-group-center { align-items: center; }
  .field-group-center .field-label { padding-top: 0; margin-bottom: 0; }
  .field-hint { font-size: 12px; line-height: 1.45; color: var(--text-2); }
  /* No reference line: there is no stock percent. */
  .range-track::before { display: none; }
  .sale-pct {
    width: 40px;
    font-size: 12px;
    font-variant-numeric: tabular-nums;
    text-align: right;
    color: var(--text-0);
  }
  .sale-sections {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px 12px;
  }
</style>
