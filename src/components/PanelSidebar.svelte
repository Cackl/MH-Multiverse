<script lang="ts">
  export let width: string = 'var(--sidebar-wide)'
</script>

<aside class="panel-sidebar" style="width: {width}; max-width: 360px;">
  <div class="panel-sidebar-head">
    <slot name="header" />
  </div>
  <div class="panel-sidebar-body">
    <slot />
  </div>
</aside>

<style>
  .panel-sidebar {
    flex-shrink: 0;
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    background: var(--sidebar-bg);
    overflow: hidden;
  }

  .panel-sidebar-head {
    display: flex;
    align-items: center;
    padding: 14px 16px 12px;
    border-bottom: 1px solid var(--border);
    min-height: 53px;
    flex-shrink: 0;
    gap: 8px;
  }

  .panel-sidebar-body {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
  }
</style>