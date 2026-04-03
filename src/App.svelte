<script lang="ts">
  import { onMount } from 'svelte'
  import Titlebar from './components/TitleBar.svelte'
  import Rail from './components/Rail.svelte'
  import LaunchPanel from './components/LaunchPanel.svelte'
  import ServerPanel from './components/ServerPanel.svelte'
  import TuningPanel from './components/TuningPanel.svelte'
  import ConfigPanel from './components/ConfigPanel.svelte'
  import OpsPanel from './components/OpsPanel.svelte'
  import AppPanel from './components/AppPanel.svelte'
  import StorePanel from './components/StorePanel.svelte'
  import { activeTab, loadConfig } from './lib/store'

  onMount(async () => {
    await loadConfig()
  })
</script>

<div class="chrome">
  <Titlebar />
  <div class="main-area">
    <Rail />
    <div class="content">
      {#if $activeTab === 'launch'}
        <LaunchPanel />
      {:else if $activeTab === 'server'}
        <ServerPanel />
      {:else if $activeTab === 'tuning'}
        <TuningPanel />
      {:else if $activeTab === 'config'}
        <ConfigPanel />
      {:else if $activeTab === 'store'}
        <StorePanel />
      {:else if $activeTab === 'ops'}
        <OpsPanel />
      {:else if $activeTab === 'settings'}
        <AppPanel />
      {/if}
    </div>
  </div>
</div>

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