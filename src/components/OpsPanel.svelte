<script lang="ts">
  // OpsPanel.svelte — Nightly updater and backup management
  // Features 4 & 5 in LIVETUNEPLANNING.md — not yet implemented
</script>

<div class="ops-panel">
  <div class="placeholder">
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
      <polyline points="23 6 13.5 15.5 8.5 10.5 1 18"/>
      <polyline points="17 6 23 6 23 12"/>
    </svg>
    <span class="placeholder-label">Ops</span>
    <span class="placeholder-sub">Coming soon</span>
  </div>
</div>

<style>
  .ops-panel {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .placeholder {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    color: var(--text-3);
  }

  .placeholder svg {
    width: 36px;
    height: 36px;
    opacity: 0.4;
  }

  .placeholder-label {
    font-family: var(--font-head);
    font-size: 13px;
    font-weight: 600;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-2);
  }

  .placeholder-sub {
    font-size: 11px;
    color: var(--text-3);
  }
</style>