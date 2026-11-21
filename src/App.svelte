<script lang="ts">
  import { onMount } from 'svelte'
  import Titlebar from './components/TitleBar.svelte'
  import Rail from './components/Rail.svelte'
  import LaunchPanel from './components/launch/LaunchPanel.svelte'
  import ServerPanel from './components/server/ServerPanel.svelte'
  import ConfigPanel from './components/ConfigPanel.svelte'
  import OpsPanel from './components/OpsPanel.svelte'
  import AppPanel from './components/settings/AppPanel.svelte'
  import { listen } from '@tauri-apps/api/event'
  import { activeTab, appConfig, loadConfig, startGameRunningPoll, checkAppUpdate, handleCloseRequest } from './lib/store'
  import { initServerEventBridge } from './lib/serverEvents'
  import DataPanel from './components/data/DataPanel.svelte'
  import ConfirmDialog from './components/ConfirmDialog.svelte'

  onMount(async () => {
    listen<number>('close-requested', e => handleCloseRequest(e.payload))
    startGameRunningPoll()
    checkAppUpdate()
    await Promise.all([
      loadConfig(),
      initServerEventBridge(),
    ])
  })
</script>

<div class="chrome">
  <Titlebar />
  <div class="main-area">
    <Rail />
    <div class="content">
      <!-- A new server_exe remounts the tab, so no panel keeps the old version's data. -->
      {#key $appConfig.server_exe}
      {#if $activeTab === 'launch'}
        <LaunchPanel />
      {:else if $activeTab === 'server'}
        <ServerPanel />
      {:else if $activeTab === 'config'}
        <ConfigPanel />
      {:else if $activeTab === 'data'}
        <DataPanel />
      {:else if $activeTab === 'ops'}
        <OpsPanel />
      {:else if $activeTab === 'settings'}
        <AppPanel />
      {/if}
      {/key}
    </div>
  </div>
</div>

<ConfirmDialog />

<style>
  .chrome {
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
    background: var(--bg-0);
  }

  .main-area {
    flex: 1;
    display: flex;
    overflow: hidden;
  }

  .content {
    flex: 1;
    overflow: hidden;
    display: flex;
    min-width: 0;
    background: var(--bg-1);
  }
</style>