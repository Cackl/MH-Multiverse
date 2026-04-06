<script lang="ts">
  import { activeTab, activeDataTab, serverRunning, apacheRunning, gameRunning, type Tab, type DataTab } from '../lib/store'

  function go(tab: Tab) {
    activeTab.set(tab)
  }

  // Switches to the Data rail entry, restoring whichever sub-tab was last active.
  function goData(sub: DataTab) {
    activeTab.set('data')
    activeDataTab.set(sub)
  }
</script>

<nav class="rail">

  <div class="rail-group">
    <span class="rail-group-label">MHO</span>
  </div>

  <button class="rail-tab" class:active={$activeTab === 'launch'} on:click={() => go('launch')}>
    <svg viewBox="0 0 25 25" fill="none" stroke="currentColor" stroke-width="2">
      <polygon points="5,3 19,12 5,21" fill="none" stroke="currentColor"/>
    </svg>
    <span class="rail-label">Launch</span>
  </button>

  <div class="rail-group">
    <span class="rail-group-label">Local</span>
  </div>

  <button class="rail-tab" class:active={$activeTab === 'server'} on:click={() => go('server')}>
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <rect x="2" y="3" width="20" height="7" rx="1"/>
      <rect x="2" y="14" width="20" height="7" rx="1"/>
      <circle cx="6" cy="6.5" r="1" fill="currentColor" stroke="none"/>
      <circle cx="6" cy="17.5" r="1" fill="currentColor" stroke="none"/>
    </svg>
    <span class="rail-label">Server</span>
  </button>

  <button class="rail-tab" class:active={$activeTab === 'config'} on:click={() => go('config')}>
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
      <polyline points="14 2 14 8 20 8"/>
      <line x1="8" y1="13" x2="16" y2="13"/>
      <line x1="8" y1="17" x2="13" y2="17"/>
    </svg>
    <span class="rail-label">Config</span>
  </button>

  <button class="rail-tab" class:active={$activeTab === 'data'} on:click={() => goData($activeDataTab)}>
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <line x1="4"  y1="6"  x2="20" y2="6"/>
      <line x1="4"  y1="12" x2="14" y2="12"/>
      <line x1="4"  y1="18" x2="17" y2="18"/>
      <circle cx="18" cy="6"  r="2" fill="currentColor" stroke="none"/>
      <circle cx="17" cy="12" r="2" fill="currentColor" stroke="none"/>
      <circle cx="20" cy="18" r="2" fill="currentColor" stroke="none"/>
    </svg>
    <span class="rail-label">Data</span>
  </button>

  <button class="rail-tab" class:active={$activeTab === 'ops'} on:click={() => go('ops')}>
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
      <polyline points="7 10 12 15 17 10"/>
      <line x1="12" y1="15" x2="12" y2="3"/>
    </svg>
    <span class="rail-label">Update</span>
  </button>

  <div class="rail-group">
    <span class="rail-group-label">App</span>
  </div>

  <button class="rail-tab" class:active={$activeTab === 'settings'} on:click={() => go('settings')}>
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <path d="M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z"/>
      <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>
    </svg>
    <span class="rail-label">Settings</span>
  </button>

  <div class="rail-spacer"></div>

  <div class="rail-status">
    <div class="status-dot-wrap" title="MHServerEmu">
      <div class="status-dot" class:on={$serverRunning}></div>
      <span class="status-dot-label">Server</span>
    </div>
    <div class="status-dot-wrap" title="Apache">
      <div class="status-dot" class:on={$apacheRunning}></div>
      <span class="status-dot-label">Apache</span>
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

  /* ── Group divider ── */
  .rail-group {
    width: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 8px 8px 4px;
    gap: 3px;
  }

  .rail-group-label {
    font-family: var(--font-head);
    font-size: 8px;
    font-weight: 600;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--text-3);
    line-height: 1;
  }

  .rail-group::after {
    content: '';
    display: block;
    width: 32px;
    height: 1px;
    background: var(--border-mid);
  }

  /* ── Nav tabs ── */
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

  /* ── Status indicators ── */
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