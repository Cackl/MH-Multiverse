<script context="module" lang="ts">
  // Cards start closed on app launch. This set outlives the component, so a card stays open
  // while you switch tabs, but it is not persisted across launches.
  const openIds = new Set<string>()

  // Lets other pages deep-link here: call before the card mounts.
  export function openCard(id: string) { openIds.add(id) }
</script>

<script lang="ts">
  // Shell for one integration on the Integrations page: a collapsible card with a
  // title and a one-line subtitle.
  // A new integration is a new component wrapped in this card.
  export let id: string
  export let title: string
  export let subtitle = ''

  let open = openIds.has(id)

  function toggle() {
    open = !open
    if (open) openIds.add(id)
    else openIds.delete(id)
  }
</script>

<section class="card">
  <div
    class="card-head"
    role="button"
    tabindex="0"
    aria-expanded={open}
    on:click={toggle}
    on:keydown={(e) => (e.key === 'Enter' || e.key === ' ') && toggle()}
  >
    <svg class="chev" class:open viewBox="0 0 10 10" width="10" height="10" fill="none" stroke="currentColor" stroke-width="1.5">
      <polyline points="3,1.5 7,5 3,8.5" />
    </svg>
    <div class="titles">
      <div class="name">{title}</div>
      {#if subtitle}<div class="sub">{subtitle}</div>{/if}
    </div>
  </div>
  {#if open}
    <div class="card-body"><slot /></div>
  {/if}
</section>

<style>
  .card {
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    background: var(--bg-2);
  }
  .card-head {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    cursor: pointer;
    user-select: none;
  }
  .card-head:hover { background: var(--bg-3); }
  .chev { color: var(--text-3); transition: transform 0.15s; flex-shrink: 0; }
  .chev.open { transform: rotate(90deg); }
  .titles { flex: 1; min-width: 0; }
  .name {
    font-family: var(--font-head);
    font-size: 13px;
    font-weight: 600;
    color: var(--text-1);
  }
  .sub { font-size: 11px; color: var(--text-3); }
  .card-body {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 12px 14px 14px;
    border-top: 1px solid var(--border-mid);
  }
</style>
