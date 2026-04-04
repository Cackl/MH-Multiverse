<script lang="ts">
  import { activeDataTab, type DataTab } from '../lib/store'
  import TuningPanel from './TuningPanel.svelte'
  import StorePanel  from './StorePanel.svelte'

  const TABS: { id: DataTab; label: string }[] = [
    { id: 'tuning',  label: 'Live Tuning' },
    { id: 'store',   label: 'Store'        },
    { id: 'patches', label: 'Patches'      },
  ]
</script>

<div class="data-panel">

  <!-- ── Inner tab bar ── -->
  <div class="data-tabs" role="tablist">
    {#each TABS as tab (tab.id)}
      <button
        class="data-tab"
        class:active={$activeDataTab === tab.id}
        role="tab"
        aria-selected={$activeDataTab === tab.id}
        on:click={() => activeDataTab.set(tab.id)}
      >
        {tab.label}
      </button>
    {/each}
  </div>

  <!-- ── Active panel ── -->
  <div class="data-content">
    {#if $activeDataTab === 'tuning'}
      <TuningPanel />
    {:else if $activeDataTab === 'store'}
      <StorePanel />
    {:else if $activeDataTab === 'patches'}
      <!-- PatchesPanel goes here -->
      <div class="placeholder">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
          <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
          <polyline points="14 2 14 8 20 8"/>
          <line x1="12" y1="11" x2="12" y2="17"/>
          <line x1="9"  y1="14" x2="15" y2="14"/>
        </svg>
        <span class="placeholder-label">Patches</span>
        <span class="placeholder-sub">Data patch editor coming soon.</span>
      </div>
    {/if}
  </div>

</div>

<style>
  .data-panel {
    display: flex;
    flex: 1;
    flex-direction: column;
    overflow: hidden;
    min-height: 0;
  }

  /* ── Tab bar ── */
  .data-tabs {
    display: flex;
    align-items: stretch;
    gap: 0;
    background: var(--bg-0);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    padding: 0 8px;
  }

  .data-tab {
    font-family: var(--font-head);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    padding: 0 14px;
    height: 36px;
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    color: var(--text-3);
    cursor: pointer;
    transition: color 0.14s, border-color 0.14s;
    position: relative;
    white-space: nowrap;
    /* Pull the bottom border flush with the tab bar border */
    margin-bottom: -1px;
  }

  .data-tab:hover {
    color: var(--text-1);
  }

  .data-tab.active {
    color: var(--accent-bright);
    border-bottom-color: var(--accent);
  }

  /* ── Panel area ── */
  .data-content {
    display: flex;
    flex: 1;
    overflow: hidden;
    min-height: 0;
  }

  /* ── Placeholder (Patches not yet implemented) ── */
  .placeholder {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    color: var(--text-3);
  }

  .placeholder svg {
    width: 36px;
    height: 36px;
    opacity: 0.3;
  }

  .placeholder-label {
    font-family: var(--font-head);
    font-size: 12px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-2);
  }

  .placeholder-sub {
    font-size: 11px;
    color: var(--text-3);
  }
</style>