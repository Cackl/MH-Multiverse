<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { appConfig, upsertLinked, deleteLinked, activateLinked, askConfirm, type LinkedInstall } from '../../lib/store'
  import { pickExe } from '../../lib/browseExe'
  import { versionMatch } from '../../lib/versionCaps'
  import { serverFolder, withExes } from '../../lib/linkedVersions'
  import { tip } from '../../lib/tooltip'
  import IntegrationCard, { openCard } from './integrations/IntegrationCard.svelte'
  import Dropdown from '../Dropdown.svelte'

  const VERSIONS = ['1.48', '1.52', '1.53']

  let error = ''

  // Detected versions by exe path. null is not detected.
  let detected: Record<string, string | null> = {}
  async function detect(command: string, key: string, path: string): Promise<string | null> {
    if (!path) return null
    if (!(path in detected)) {
      const v = await invoke<string | null>(command, { [key]: path }).catch(() => null)
      detected = { ...detected, [path]: v }
    }
    return detected[path]
  }
  const client = (l: LinkedInstall) => detect('detect_client_version', 'gameExe', l.game_exe)
  const server = (l: LinkedInstall) => detect('detect_server_version', 'serverExe', l.server_exe)
  $: for (const l of $appConfig.linked) { client(l); server(l) }

  $: info = (l: LinkedInstall) => {
    const c = detected[l.game_exe] ?? null
    const s = detected[l.server_exe] ?? null
    const known = (!l.game_exe || l.game_exe in detected) && (!l.server_exe || l.server_exe in detected)
    return { client: c, server: s, match: versionMatch(c, s), found: c ?? s, known }
  }

  async function save(l: LinkedInstall) {
    error = ''
    try {
      await upsertLinked(l)
    } catch (e) {
      error = String(e)
    }
  }

  async function browse(l: LinkedInstall, field: 'game_exe' | 'server_exe') {
    const path = await pickExe()
    if (!path) return
    const next = { ...l, [field]: path }
    await save(withExes(l, next.game_exe, next.server_exe, await client(next), await server(next)))
  }

  async function add() {
    const l: LinkedInstall = {
      id: crypto.randomUUID(), name: '', version: '', game_exe: '', server_exe: '', custom: false, no_steam: false,
    }
    openCard(`linked-${l.id}`)
    await save(l)
  }

  async function use(id: string) {
    error = ''
    try {
      await activateLinked(id)
    } catch (e) {
      error = String(e)
    }
  }

  async function remove(l: LinkedInstall) {
    if (!await askConfirm({
      title: `Remove ${l.name || 'this version'}?`,
      message: 'Its client and server files stay where they are.',
      okLabel: 'Remove', cancelLabel: 'Keep it',
    })) return
    error = ''
    try {
      await deleteLinked(l.id)
    } catch (e) {
      error = String(e)
    }
  }
</script>

<div class="lv-head">
  <span class="field-label">Linked Versions</span>
  <button class="btn btn-sm btn-outline" on:click={add}>Add version</button>
</div>

{#if error}
  <div class="lv-error">{error}</div>
{/if}

{#each $appConfig.linked as l (l.id)}
  {@const i = info(l)}
  {@const active = l.id === $appConfig.active_linked_id}
  <IntegrationCard id={`linked-${l.id}`} title={l.name || 'Unnamed'} subtitle={serverFolder(l.server_exe) || 'No server'}>
    <svelte:fragment slot="actions">
      {#if l.custom}<span class="tag">Experimental</span>{/if}
      <span class="lv-version" class:warn={i.match === 'mismatch'}>
        {i.match === 'mismatch' ? 'Mismatch' : l.version}
      </span>
      {#if active}
        <span class="tag tag-active">Active</span>
      {:else}
        <button class="btn btn-sm btn-outline" on:click={() => use(l.id)}>Use</button>
      {/if}
    </svelte:fragment>

    <div class="lv-edit">
      <span class="field-label">Name</span>
      <input type="text" class="lv-name-input" value={l.name}
        on:change={(e) => save({ ...l, name: e.currentTarget.value.trim() })} />
      <span></span>

      <span class="field-label">Client</span>
      <div class="field-value path-value" use:tip={l.game_exe}>{l.game_exe || 'Not set'}</div>
      <button class="btn btn-sm btn-outline" on:click={() => browse(l, 'game_exe')}>Browse</button>

      <span class="field-label">Server</span>
      <div class="field-value path-value" use:tip={l.server_exe}>{l.server_exe || 'Not set'}</div>
      <button class="btn btn-sm btn-outline" on:click={() => browse(l, 'server_exe')}>Browse</button>

      <span class="field-label">Version</span>
      {#if i.match === 'mismatch'}
        <span class="lv-value warn">Client {i.client}, server {i.server}</span>
      {:else if i.found || !i.known}
        <span class="lv-value">{i.found ?? l.version}</span>
      {:else}
        <div class="lv-select">
          <Dropdown label="Version" value={l.version} options={VERSIONS.map(v => ({ value: v, label: v }))}
            onChange={(version) => save({ ...l, version })} />
        </div>
      {/if}
      <span></span>

      <span class="field-label">Custom</span>
      <label class="check-row">
        <input type="checkbox" checked={l.custom} aria-label="Custom"
          on:change={(e) => save({ ...l, custom: e.currentTarget.checked })} />
      </label>
      {#if active}
        <span></span>
      {:else}
        <button class="btn btn-sm btn-outline" on:click={() => remove(l)}>Remove</button>
      {/if}
    </div>
  </IntegrationCard>
{/each}

<style>
  .lv-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .lv-head .field-label { margin-bottom: 0; }

  .tag-active { color: var(--accent); border-color: var(--accent-dim); }
  .lv-version, .lv-value {
    font-size: 12px;
    color: var(--text-1);
    font-variant-numeric: tabular-nums;
  }
  .lv-version { min-width: 56px; text-align: right; }
  .warn { color: var(--amber); }

  .lv-edit {
    display: grid;
    grid-template-columns: 72px minmax(0, 1fr) auto;
    gap: 8px 10px;
    align-items: center;
  }
  .lv-edit .field-label { margin-bottom: 0; }
  .lv-name-input { font-family: var(--font-body); }
  .lv-select { justify-self: start; }
  .lv-error {
    font-size: 12px;
    color: var(--text-error);
  }
</style>
