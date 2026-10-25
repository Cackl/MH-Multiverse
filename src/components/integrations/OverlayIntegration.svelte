<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { open } from '@tauri-apps/plugin-dialog'
  import { invoke } from '@tauri-apps/api/core'
  import { overlayInject, overlayLog, overlayStatus, overlayUpdate, type OverlayStatus, type OverlayUpdate } from '../../lib/store'
  import { openUrl } from '@tauri-apps/plugin-opener'
  import { tip } from '../../lib/tooltip'
  import IntegrationCard from './IntegrationCard.svelte'

  let overlay: OverlayStatus | null = null
  let flags: Record<string, boolean> = {}
  let error = ''

  $: if (overlay) overlayStatus.set(overlay)
  $: state = overlay?.state ?? 'not_linked'
  $: canTrust = state === 'untrusted' || state === 'changed'
  $: trustLabel = state === 'trusted' ? 'Untrust' : state === 'changed' ? 'Re-trust' : 'Trust'

  const HOW_TRUST_WORKS =
    'Multiverse never runs the overlay programs. Trust records a hash of the overlay files, which are ' +
    're-checked right before the overlay is loaded into the game. If anything changed, it is not loaded. ' +
    'The hash only detects changes: trusting the overlay is your decision.'
  const STATE_HELP: Record<OverlayStatus['state'], string> = {
    not_linked: 'Choose the overlay folder first.',
    missing: 'The overlay files were not found in the linked folder.',
    untrusted: 'Not trusted yet.',
    trusted: 'Trusted: the files match what you trusted. Press to remove trust.',
    changed: 'The files changed since you trusted them (for example after an update). Press to trust the new files.',
  }
  $: trustHelp = `${STATE_HELP[state]}\n\n${HOW_TRUST_WORKS}`

  const FLAGS: { key: string; name: string; desc: string }[] = [
    { key: 'terminals', name: 'Terminals Tracker', desc: 'Shows terminal tracker when opening the Waypoint' },
    { key: 'dps', name: 'DPS Meter', desc: 'Shows the damage meter' },
    { key: 'detector_log', name: 'Debug Logging', desc: 'Writes overlay log files, separate to Hidden Items' },
  ]

  async function call(cmd: string, args?: Record<string, unknown>) {
    error = ''
    try {
      overlay = await invoke<OverlayStatus>(cmd, args)
    } catch (e) {
      error = String(e)
    }
  }

  async function loadFlags() {
    try { flags = await invoke('overlay_get_flags') } catch { flags = {} }
  }

  async function setFlag(key: string, on: boolean) {
    error = ''
    try { flags = await invoke('overlay_set_flag', { key, on }) } catch (e) { error = String(e) }
  }

  async function browse() {
    const selected = await open({ directory: true, multiple: false })
    if (selected && typeof selected === 'string') {
      await call('overlay_set_dir', { dir: selected })
      await loadFlags()
    }
  }

  function onTrustButton() {
    call(state === 'trusted' ? 'overlay_untrust' : 'overlay_trust')
  }

  let checking = false
  let updateError = ''
  async function checkUpdate() {
    checking = true
    updateError = ''
    try { overlayUpdate.set(await invoke<OverlayUpdate>('check_overlay_update')) } catch (e) { updateError = String(e) }
    checking = false
  }

  onMount(() => { call('overlay_status'); loadFlags() })

  // ── Loot log ──
  let showLog = true
  try { showLog = localStorage.getItem('overlay-log-open') !== '0' } catch { /* storage unavailable */ }
  function toggleLog() {
    showLog = !showLog
    try { localStorage.setItem('overlay-log-open', showLog ? '1' : '0') } catch { /* storage unavailable */ }
  }

  $: hiddenCount = $overlayLog.filter(l => l.event === 'loot_hidden').length

  // Keep the newest row in view unless the user has scrolled up to read.
  let logEl: HTMLDivElement | undefined
  let stickToBottom = true
  function onLogScroll() {
    if (logEl) stickToBottom = logEl.scrollHeight - logEl.scrollTop - logEl.clientHeight < 24
  }
  // Scroll via a local ref: assigning logEl.scrollTop in a reactive statement would
  // count as a change to logEl and re-trigger it forever.
  async function scrollLogToEnd() {
    await tick()
    const el = logEl
    if (el) el.scrollTop = el.scrollHeight
  }
  $: if ($overlayLog && logEl && stickToBottom) scrollLogToEnd()
</script>

<IntegrationCard id="overlay" title="MarvelHeroesOmega Overlay" subtitle="Loot filter, terminals tracker and DPS meter. (Wmascent)">

  <div class="path-group">
    <span class="field-label">Overlay Folder</span>
    <div class="path-row">
      <div class="field-value path-value" use:tip={overlay?.dir ?? ''}>{overlay?.dir || 'Not set'}</div>
      <button class="btn btn-sm btn-outline" on:click={browse}>Browse</button>
      <button
        class="btn btn-sm btn-outline trust-btn"
        class:ok={state === 'trusted'}
        class:warn={state === 'changed'}
        disabled={!canTrust && state !== 'trusted'}
        use:tip={trustHelp}
        on:click={onTrustButton}
      >{trustLabel}</button>
    </div>
    {#if overlay?.changes.length}
      <ul class="opt-desc changes">
        {#each overlay.changes as c}<li>{c}</li>{/each}
      </ul>
    {/if}
  </div>

  {#if overlay}
    <div class="opt-row" class:dim={state !== 'trusted'}>
      <div class="opt-label" use:tip={'If the game is restarted outside MH Multiverse, the overlay is not loaded again.'}>
        <span class="opt-name">Inject on Launch</span>
        <span class="opt-desc">Loads the overlay into the game after Launch. Requires a trusted overlay</span>
      </div>
      <div
        class="toggle-switch"
        class:on={overlay.auto_inject}
        role="switch"
        aria-checked={overlay.auto_inject}
        tabindex="0"
        on:click={() => call('overlay_set_auto_inject', { enabled: !overlay?.auto_inject })}
        on:keydown={(e) => e.key === 'Enter' && call('overlay_set_auto_inject', { enabled: !overlay?.auto_inject })}
      ></div>
    </div>

    {#if state !== 'not_linked' && state !== 'missing'}
      <div class="section-divider"><span>Features (apply on next launch)</span></div>
      {#each FLAGS as f}
        <div class="opt-row">
          <div class="opt-label">
            <span class="opt-name">{f.name}</span>
            <span class="opt-desc">{f.desc}</span>
          </div>
          <div
            class="toggle-switch"
            class:on={flags[f.key]}
            role="switch"
            aria-checked={!!flags[f.key]}
            tabindex="0"
            on:click={() => setFlag(f.key, !flags[f.key])}
            on:keydown={(e) => e.key === 'Enter' && setFlag(f.key, !flags[f.key])}
          ></div>
        </div>
      {/each}
    {/if}
  {/if}

  {#if state !== 'not_linked' && state !== 'missing'}
    <div class="opt-desc update-note">
      <button class="btn btn-sm btn-outline" disabled={checking} on:click={checkUpdate}>{checking ? 'Checking...' : 'Check for overlay update'}</button>
      {#if updateError}
        <span class="err">{updateError}</span>
      {:else if $overlayUpdate?.update_available}
        Overlay v{$overlayUpdate.latest} is available (installed v{$overlayUpdate.installed}).
        <button class="btn btn-sm btn-outline" on:click={() => openUrl($overlayUpdate.html_url)}>Release page</button>
      {:else if $overlayUpdate}
        Up to date (v{$overlayUpdate.installed}).
      {/if}
    </div>
  {/if}

  {#if error}<span class="opt-desc err">{error}</span>{/if}
  {#if $overlayInject}
    <span class="opt-desc" class:err={!$overlayInject.ok}>Last launch: {$overlayInject.message}</span>
  {/if}

  <div class="section-divider"><span>Loot Filter</span></div>
  <div class="opt-row">
    <div class="opt-label">
      <span class="opt-name">Hidden Items Log</span>
      <span class="opt-desc">{hiddenCount} hidden this session</span>
    </div>
    <div class="log-actions">
      {#if showLog}
        <button class="btn btn-sm btn-outline" on:click={() => overlayLog.set([])}>Clear</button>
      {/if}
      <div
        class="toggle-switch"
        class:on={showLog}
        role="switch"
        aria-checked={showLog}
        aria-label="Show log"
        tabindex="0"
        use:tip={'Show or hide the log. Items are still collected while it is hidden.'}
        on:click={toggleLog}
        on:keydown={(e) => e.key === 'Enter' && toggleLog()}
      ></div>
    </div>
  </div>
  {#if showLog}
    <div class="log" bind:this={logEl} on:scroll={onLogScroll}>
      <div class="cols head">
        <span>Time</span><span>Category</span><span>Item</span><span>Code</span><span>Rarity</span>
      </div>
      {#each $overlayLog as l (l.id)}
        {#if l.event === 'loot_hidden' && l.fields.length === 6}
          <div class="cols">
            <span class="dim-text">{l.fields[0]}</span>
            <span>{l.fields[1]}</span>
            <span title={l.fields[2]}>{l.fields[2]}</span>
            <span class="dim-text">{l.fields[3]}</span>
            <span class="r r-{l.fields[4].toLowerCase()}" title={l.fields[5]}>{l.fields[4]}</span>
          </div>
          {#if l.fields[5]}<div class="sub-line">{l.fields[5]}</div>{/if}
        {:else if l.event === 'region_entered' && l.fields.length === 3}
          <div class="banner"><span class="dim-text">{l.fields[0]}</span> Terminal <b>{l.fields[1]}</b> {l.fields[2]}</div>
        {:else if l.event === 'mission_state'}
          <div class="status-line ok-text">{l.text}</div>
        {:else if l.event === 'player_death' && l.fields.length === 3}
          <div class="death">
            <div><span class="dim-text">{l.fields[0]}</span> <b>{l.fields[1]}</b> {l.fields[2]}</div>
            {#if l.hits.length}
              <table>
                {#each l.hits as h}
                  <tr>
                    <td class="dim-text">{h.time}</td><td>{h.health}</td>
                    <td title={h.note}>{h.damage}</td><td>{h.kind}</td><td>{h.attacker}</td><td class="dim-text">{h.power}</td>
                  </tr>
                {/each}
              </table>
            {/if}
          </div>
        {:else}
          <div class="status-line">{l.text}</div>
        {/if}
      {:else}
        <div class="status-line">Nothing yet. Items hidden by the loot filter appear here once the game is launched with the overlay injected.</div>
      {/each}
    </div>
  {/if}
</IntegrationCard>

<style>
  .opt-row.dim { opacity: 0.5; }
  .log-actions { display: flex; align-items: center; gap: 10px; }
  .err { color: var(--text-error); }
  .update-note { display: flex; align-items: center; gap: 10px; color: var(--amber); }
  .changes { margin: 4px 0 0; padding-left: 16px; color: var(--amber); }

  /* Trust button carries the state: green when trusted, amber when files changed */
  .trust-btn.ok   { color: var(--green-bright); border-color: rgba(39, 174, 96, 0.5); }
  .trust-btn.warn { color: var(--amber);        border-color: rgba(212, 160, 23, 0.5); }

  .log {
    height: 260px;
    overflow-y: auto;
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    background: var(--bg-0);
  }
  .cols {
    display: grid;
    grid-template-columns: 66px 110px minmax(0, 1fr) 52px 76px;
    gap: 10px;
    padding: 3px 10px;
    font-family: var(--font-mono);
    font-size: 11px;
    line-height: 1.5;
  }
  .cols span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .cols.head {
    position: sticky;
    top: 0;
    background: var(--bg-3);
    font-family: var(--font-head);
    font-size: 9px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-3);
    border-bottom: 1px solid var(--border-mid);
  }
  .dim-text { color: var(--text-3); }
  .status-line { padding: 4px 10px; font-size: 11px; color: var(--text-3); }
  .ok-text { color: var(--green); }
  .sub-line { padding: 0 10px 2px 86px; font-size: 10px; color: var(--text-3); }
  .banner { padding: 4px 10px; font-family: var(--font-mono); font-size: 11px; color: var(--accent); }
  .death { padding: 4px 10px; font-family: var(--font-mono); font-size: 11px; color: var(--text-error); border-left: 2px solid var(--text-error); }
  .death table { margin-top: 2px; border-collapse: collapse; color: var(--text-2); }
  .death td { padding: 0 10px 0 0; white-space: nowrap; }

  .r { font-weight: 600; }
  .r-common   { color: var(--text-2); }
  .r-uncommon { color: var(--green); }
  .r-rare     { color: var(--accent); }
  .r-epic     { color: var(--purple); }
  .r-cosmic   { color: var(--amber); }
  .r-ultimate, .r-omega, .r-unique, .r-runeword { color: var(--amber-bright, var(--amber)); }
</style>
