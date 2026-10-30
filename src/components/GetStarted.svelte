<script lang="ts">
  import {
    appConfig, serverRunning, apacheRunning, startingServer, startingApache,
    startServer, startApache, activeTab, settingsSection, launchOptionsReviewed,
  } from '../lib/store'
  import { browseGameExe, browseServerExe } from '../lib/browseExe'
  import { tip } from '../lib/tooltip'

  export let onAddServer: () => void

  const NEEDS_SERVER_EXE = 'Set the MHServerEmu executable first'

  $: hasServerExe = !!$appConfig.server_exe
  $: launchOptionsDone = $launchOptionsReviewed || !$appConfig.launch_options.auto_login

  function openLaunchOptions() {
    launchOptionsReviewed.set(true)
    settingsSection.set('client')
    activeTab.set('settings')
  }
</script>

<div class="get-started">
  <div class="section-divider"><span>Get Started</span></div>

  <div class="gs-row">
    <span class="mark" class:done={!!$appConfig.game_exe}></span>
    <div class="gs-text">
      <div class="gs-title">Game executable</div>
      <div class="gs-detail" use:tip={$appConfig.game_exe}>{$appConfig.game_exe || 'Not set. Needed to play on any server.'}</div>
    </div>
    <button class="btn btn-sm btn-outline" on:click={browseGameExe}>Browse</button>
  </div>

  <div class="gs-row">
    <span class="mark" class:done={launchOptionsDone}></span>
    <div class="gs-text">
      <div class="gs-title">Launch options</div>
      {#if !launchOptionsDone}
        <div class="gs-detail warn">Auto Login is on. Note the game client writes your password in plain text to its Launch.log</div>
      {/if}
    </div>
    <button class="btn btn-sm btn-outline" on:click={openLaunchOptions}>Set Up</button>
  </div>

  <div class="section-divider"><span>Play Online</span></div>

  <div class="gs-row">
    <span class="mark"></span>
    <div class="gs-text">
      <div class="gs-title">Join a remote server</div>
    </div>
    <button class="btn btn-sm btn-outline" on:click={onAddServer}>Add server</button>
  </div>

  <div class="section-divider"><span>Play Locally</span></div>
  <div class="gs-hint">Host your own MHServerEmu.</div>

  <div class="gs-row">
    <span class="mark" class:done={hasServerExe}></span>
    <div class="gs-text">
      <div class="gs-title">MHServerEmu executable</div>
      <div class="gs-detail" use:tip={$appConfig.server_exe}>{$appConfig.server_exe || 'Not set'}</div>
    </div>
    <button class="btn btn-sm btn-outline" on:click={browseServerExe}>Browse</button>
  </div>

  <div class="gs-row">
    <div class="gs-half">
      <span class="mark" class:done={$serverRunning}></span>
      <div class="gs-text">
        <div class="gs-title">Server</div>
        <div class="gs-detail">{!hasServerExe ? NEEDS_SERVER_EXE : $startingServer ? 'Starting...' : $serverRunning ? 'Running' : 'Stopped'}</div>
      </div>
      <button class="btn btn-sm btn-green" on:click={startServer} disabled={!hasServerExe || $startingServer || $serverRunning}>Start</button>
    </div>
    <div class="gs-sep"></div>
    <div class="gs-half">
      <span class="mark" class:done={$apacheRunning}></span>
      <div class="gs-text">
        <div class="gs-title">Apache</div>
        <div class="gs-detail">{!hasServerExe ? NEEDS_SERVER_EXE : $startingApache ? 'Starting...' : $apacheRunning ? 'Running' : 'Stopped'}</div>
      </div>
      <button class="btn btn-sm btn-green" on:click={startApache} disabled={!hasServerExe || $startingApache || $apacheRunning}>Start</button>
    </div>
  </div>

  <div class="gs-row">
    <span class="mark"></span>
    <div class="gs-text">
      <div class="gs-title">Local account</div>
      <div class="gs-detail">Create an account on your server and a profile to log in with.</div>
    </div>
    <button class="btn btn-sm btn-outline" on:click={onAddServer}>Create local server</button>
  </div>
</div>

<style>
  .get-started {
    grid-column: 2;
    grid-row: 1 / -1;
    overflow-y: auto;
    padding: 20px 24px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .gs-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    background: var(--bg-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
  }

  .mark {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
    border: 1.5px solid var(--border-lit);
    border-radius: 50%;
  }
  .mark.done {
    background: var(--green);
    border-color: var(--green);
  }

  .gs-half {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .gs-half .gs-detail { white-space: normal; }
  .gs-sep {
    align-self: stretch;
    width: 1px;
    background: var(--border);
  }

  .gs-text { flex: 1; min-width: 0; }
  .gs-title {
    font-family: var(--font-head);
    font-size: 13px;
    font-weight: 600;
    color: var(--text-0);
  }
  .gs-detail {
    margin-top: 2px;
    font-size: 12px;
    color: var(--text-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .gs-detail.warn {
    color: var(--amber);
    white-space: normal;
  }
  .gs-hint {
    font-size: 12px;
    color: var(--text-2);
  }
</style>
