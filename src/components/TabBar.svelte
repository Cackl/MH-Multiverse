<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window'

  const appWindow = getCurrentWindow()

  function minimize() { appWindow.minimize() }
  function toggleMaximize() { appWindow.toggleMaximize() }
  function close() { appWindow.close() }

  function onMouseDown(e: MouseEvent) {
    // Only drag from the titlebar background, not from buttons
    if ((e.target as HTMLElement).closest('.titlebar-controls')) return
    if (e.detail === 2) {
      toggleMaximize()
    } else {
      appWindow.startDragging()
    }
  }
</script>

<!-- svelte-ignore a11y-no-static-element-interactions -->
<header class="titlebar" on:mousedown={onMouseDown}>
  <div class="titlebar-brand">
    <div class="titlebar-logo">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="color: var(--accent)">
        <polygon points="12,2 22,8.5 22,15.5 12,22 2,15.5 2,8.5"/>
        <line x1="12" y1="2" x2="12" y2="22" opacity="0.3"/>
        <line x1="2" y1="8.5" x2="22" y2="8.5" opacity="0.3"/>
      </svg>
    </div>
    <div class="titlebar-name">MH <span>Manifold</span></div>
  </div>

  <div class="titlebar-controls">
    <button class="titlebar-btn" on:click={minimize} aria-label="Minimize">
      <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5">
        <line x1="3" y1="7" x2="11" y2="7"/>
      </svg>
    </button>
    <button class="titlebar-btn" on:click={toggleMaximize} aria-label="Maximize">
      <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5">
        <rect x="3" y="3" width="8" height="8" rx="1"/>
      </svg>
    </button>
    <button class="titlebar-btn close" on:click={close} aria-label="Close">
      <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5">
        <line x1="3.5" y1="3.5" x2="10.5" y2="10.5"/>
        <line x1="10.5" y1="3.5" x2="3.5" y2="10.5"/>
      </svg>
    </button>
  </div>
</header>

<style>
  .titlebar {
    height: 38px;
    display: flex;
    align-items: center;
    padding: 0 12px;
    background: var(--bg-0);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    user-select: none;
    position: relative;
    z-index: 10;
  }

  .titlebar-brand {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .titlebar-logo {
    width: 18px;
    height: 18px;
  }
  .titlebar-logo svg {
    width: 100%;
    height: 100%;
  }

  .titlebar-name {
    font-family: var(--font-head);
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-1);
  }
  .titlebar-name span {
    color: var(--accent);
  }

  .titlebar-controls {
    margin-left: auto;
    display: flex;
    gap: 2px;
  }

  .titlebar-btn {
    width: 34px;
    height: 26px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: none;
    color: var(--text-2);
    cursor: pointer;
    border-radius: 3px;
    transition: background 0.12s, color 0.12s;
  }
  .titlebar-btn:hover {
    background: var(--bg-3);
    color: var(--text-0);
  }
  .titlebar-btn.close:hover {
    background: var(--red);
    color: #fff;
  }
  .titlebar-btn svg {
    width: 14px;
    height: 14px;
  }
</style>