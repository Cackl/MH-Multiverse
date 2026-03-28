<script lang="ts">
  import { activeTab, serverRunning, apacheRunning, gameRunning, type Tab } from '../lib/store'

  function go(tab: Tab) {
    activeTab.set(tab)
  }
</script>

<nav class="rail">
  <button class="rail-tab" class:active={$activeTab === 'launch'} on:click={() => go('launch')}>
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <path d="M13 2L3 14h9l-1 8 10-12h-9l1-8z"/>
    </svg>
    <span class="rail-label">Launch</span>
  </button>

  <button class="rail-tab" class:active={$activeTab === 'server'} on:click={() => go('server')}>
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <rect x="2" y="3" width="20" height="7" rx="1"/>
      <rect x="2" y="14" width="20" height="7" rx="1"/>
      <circle cx="6" cy="6.5" r="1" fill="currentColor" stroke="none"/>
      <circle cx="6" cy="17.5" r="1" fill="currentColor" stroke="none"/>
    </svg>
    <span class="rail-label">Server</span>
  </button>

  <button class="rail-tab" class:active={$activeTab === 'app'} on:click={() => go('app')}>
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <circle cx="12" cy="12" r="1"/>
      <circle cx="12" cy="5" r="1"/>
      <circle cx="12" cy="19" r="1"/>
    </svg>
    <span class="rail-label">App</span>
  </button>

  <div class="rail-spacer"></div>

  <div class="rail-status">
    <div class="status-dot-wrap" title="MHServerEmu">
      <div class="status-dot" class:on={$serverRunning}></div>
      <span class="status-dot-label">Srv</span>
    </div>
    <div class="status-dot-wrap" title="Apache">
      <div class="status-dot" class:on={$apacheRunning}></div>
      <span class="status-dot-label">Web</span>
    </div>
    <div class="status-dot-wrap" title="Game">
      <div class="status-dot" class:on={$gameRunning}></div>
      <span class="status-dot-label">Game</span>
    </div>
  </div>
</nav>

<style>
  .rail {
    width: var(--rail-w);
    background: var(--bg-0);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 10px 0;
    gap: 2px;
    flex-shrink: 0;
  }

  .rail-tab {
    width: 42px;
    height: 42px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 3px;
    background: none;
    border: none;
    cursor: pointer;
    border-radius: var(--radius-md);
    color: var(--text-2);
    position: relative;
    transition: color 0.15s, background 0.15s;
  }
  .rail-tab svg {
    width: 18px;
    height: 18px;
    flex-shrink: 0;
  }
  .rail-tab .rail-label {
    font-family: var(--font-head);
    font-size: 8px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    line-height: 1;
  }

  .rail-tab:hover {
    color: var(--text-1);
    background: var(--bg-2);
  }

  .rail-tab.active {
    color: var(--accent-bright);
    background: var(--accent-glow);
  }
  .rail-tab.active::before {
    content: '';
    position: absolute;
    left: -7px;
    top: 50%;
    transform: translateY(-50%);
    width: 3px;
    height: 20px;
    background: var(--accent);
    border-radius: 0 2px 2px 0;
  }

  .rail-spacer { flex: 1; }

  .rail-status {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 8px 0;
  }

  .status-dot-wrap {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
  }

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-3);
    transition: background 0.3s, box-shadow 0.3s;
  }
  .status-dot.on {
    background: var(--green-bright);
    box-shadow: 0 0 8px rgba(46, 204, 113, 0.4);
  }

  .status-dot-label {
    font-family: var(--font-head);
    font-size: 7px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-3);
  }
</style>