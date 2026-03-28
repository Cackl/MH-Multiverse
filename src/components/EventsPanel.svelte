<script lang="ts">
  // EventsPanel.svelte — Live Tuning event toggle management
  // Feature 2 in LIVETUNEPLANNING.md — not yet implemented
</script>

<div class="events-panel">
  <div class="placeholder">
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
      <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z"/>
    </svg>
    <span class="placeholder-label">Events</span>
    <span class="placeholder-sub">Coming soon</span>
  </div>
</div>

<style>
  .events-panel {
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