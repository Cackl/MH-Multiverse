<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { openPath } from '@tauri-apps/plugin-opener'
  import { appConfig, serverRunning } from '../lib/store'
  import { tip } from '../lib/tooltip'
  import PanelSidebar from './PanelSidebar.svelte'
  import { patchPack, STOCK_PACKS, type PatchFileInfo } from '../lib/patchesMeta'

  type View = 'all' | 'on' | 'off'
  const VIEWS: { id: View; label: string }[] = [
    { id: 'all', label: 'All' },
    { id: 'on',  label: 'On' },
    { id: 'off', label: 'Off' },
  ]
  // Your own files come first, the stock ones after.
  const GROUPS = ['Yours', 'Stock'] as const

  let files: PatchFileInfo[] = []
  let loaded = false
  let loadError = ''
  let toggleError = ''
  let folderError = ''
  let toggling: string | null = null
  let view: View = 'all'
  /** Set by a toggle; cleared once the server has (re)started, which is when patches load. */
  let pendingRestart = false
  let sawStopped = false

  $: serverExe = $appConfig.server_exe
  $: if (serverExe) load(serverExe)

  $: if (!$serverRunning) sawStopped = true
  $: if ($serverRunning && sawStopped && pendingRestart) pendingRestart = false

  async function load(exe: string) {
    loadError = ''
    try {
      files = await invoke<PatchFileInfo[]>('scan_patch_files', { serverExe: exe })
      loaded = true
    } catch (e) {
      loadError = String(e)
    }
  }

  // A file can sit in both Patches and Patches/Off after an update, so the key includes the folder.
  const keyOf = (f: PatchFileInfo) => `${f.enabled ? 'on' : 'off'}/${f.file_name}`

  $: rows = files
    .filter(f => view === 'all' || f.enabled === (view === 'on'))
    .map(f => ({ file: f, key: keyOf(f), pack: patchPack(f.file_name), stock: f.file_name in STOCK_PACKS }))
    .sort((a, b) => a.pack.name.localeCompare(b.pack.name))

  async function toggle(f: PatchFileInfo) {
    const key = keyOf(f)
    toggling = key
    toggleError = ''
    try {
      const enabled = await invoke<boolean>('set_patch_file_enabled', {
        serverExe,
        fileName: f.file_name,
        currentlyEnabled: f.enabled,
      })
      files = files.map(x => keyOf(x) === key ? { ...x, enabled } : x)
      pendingRestart = true
      sawStopped = !$serverRunning
    } catch (e) {
      toggleError = String(e)
    } finally {
      toggling = null
    }
  }

  async function openFolder() {
    folderError = ''
    try {
      await openPath(await invoke<string>('get_patches_dir', { serverExe }))
    } catch (e) {
      folderError = String(e)
    }
  }
</script>

<div class="patches">
  <div class="panel-bg"></div>
  <div class="grid-overlay"></div>
  <div class="patches-layout">
    <PanelSidebar width="var(--sidebar-narrow)">
      <svelte:fragment slot="header">
        <div class="section-title">Show</div>
        <button
          class="btn-icon"
          style="margin-left:auto;"
          on:click={openFolder}
          disabled={!serverExe}
          aria-label="Open Patches folder"
          use:tip={'Open Patches folder'}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
          </svg>
        </button>
      </svelte:fragment>

      <div class="patches-side">
        {#each VIEWS as v}
          <button class="side-item" class:active={view === v.id} on:click={() => view = v.id}><span class="side-name">{v.label}</span></button>
        {/each}
      </div>
    </PanelSidebar>

    <section class="data-main">
      <header class="page-head">
        <h3 class="page-title">Patches</h3>
        <div class="page-acts">
          <span class="restart-note">Patches load when the server starts</span>
        </div>
      </header>

      <div class="patches-body">
        {#if !serverExe}
          <p class="patches-msg">Set the server location first.</p>
        {:else if loadError}
          <p class="patches-msg error">{loadError}</p>
        {:else if loaded && files.length === 0}
          <p class="patches-msg">This server has no patch files.</p>
        {:else if loaded && rows.length === 0}
          <p class="patches-msg">{view === 'on' ? 'No patches are on.' : 'No patches are off.'}</p>
        {:else}
          {#each GROUPS as group}
          {@const groupRows = rows.filter(r => r.stock === (group === 'Stock'))}
          {#if groupRows.length}
          <div class="subsection-title patches-group">{group}</div>
          {#each groupRows as row (row.key)}
            <div class="pack-row" class:off={!row.file.enabled}>
              <span class="pack-name">
                <span class="pack-title" use:tip={row.file.file_name}>{row.pack.name}</span>
              </span>
              <span class="pack-desc">{row.pack.description}</span>
              <button
                class="toggle-switch"
                class:on={row.file.enabled}
                role="switch"
                aria-checked={row.file.enabled}
                aria-label={row.pack.name}
                disabled={toggling === row.key}
                on:click={() => toggle(row.file)}
              ></button>
            </div>
          {/each}
          {/if}
          {/each}
        {/if}
      </div>

      <footer class="panel-footer">
        <div class="patches-status">
          {#if toggleError}<span class="error">{toggleError}</span>
          {:else if folderError}<span class="error">{folderError}</span>
          {:else if pendingRestart}<span>{$serverRunning ? 'Takes effect after a server restart.' : 'Applies next time the server starts.'}</span>{/if}
        </div>
      </footer>
    </section>
  </div>
</div>

<style>
  .patches {
    display: flex;
    flex: 1;
    position: relative;
    overflow: hidden;
    min-height: 0;
  }
  .patches-layout {
    position: relative;
    z-index: 1;
    flex: 1;
    display: flex;
    min-height: 0;
  }
  .patches-side { padding: 6px; }
  .patches-body {
    flex: 1;
    overflow-y: auto;
    padding: 4px 20px 22px;
  }
  .patches-msg { font-size: 12px; color: var(--text-2); }
  .error { font-size: 12px; color: var(--text-error); }
  .patches-status {
    flex: 1;
    min-width: 0;
    min-height: 26px;
    display: flex;
    align-items: center;
    font-size: 12px;
    color: var(--text-1);
  }
  .patches-group { margin: 14px 0 0; }
  .restart-note { font-size: 12px; color: var(--text-2); white-space: nowrap; }

  /* Name, what it does, switch. */
  .pack-row {
    display: grid;
    grid-template-columns: minmax(0, 0.7fr) minmax(0, 1.6fr) 36px;
    column-gap: 16px;
    align-items: center;
    padding: 10px 0;
    border-bottom: 1px solid var(--border);
  }
  .pack-name {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
  }
  .pack-title {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-head);
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.02em;
    color: var(--text-1);
  }
  .pack-desc { font-size: 12px; color: var(--text-2); }
  .pack-row.off .pack-title,
  .pack-row.off .pack-desc { opacity: var(--dimmed-opacity); }
</style>
