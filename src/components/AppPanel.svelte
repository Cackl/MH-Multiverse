<script lang="ts">
  import { onDestroy } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { openUrl } from '@tauri-apps/plugin-opener'
  import {
    appConfig, type SettingsSection, activeTheme, setTheme, setTextSize, setLaunchOptions, type LaunchOptions,
    appUpdate, appUpdateError, appUpdateCheckedAt, checkAppUpdate, showUpdateBadge,
    updateRemindLater, dismissedUpdate, serverRunning, settingsSection, activeCaps,
  } from '../lib/store'
  import { parseReleaseNotes } from '../lib/releaseNotes'
  import { tip } from '../lib/tooltip'
  import PanelSidebar from './PanelSidebar.svelte'
  import LinkedVersions from './LinkedVersions.svelte'
  import OverlayIntegration from './integrations/OverlayIntegration.svelte'
  import ProfileImport from './integrations/ProfileImport.svelte'

  const REPO_URL = 'https://github.com/Cackl/MH-Multiverse'
  const STAGE_LABELS: Record<string, string> = {
    downloading: 'Downloading',
    verifying: 'Verifying signature',
    installing: 'Installing, restarting shortly',
  }

  let checking = false
  let installing = false
  let installError = ''
  let progress: { stage: string; pct: number } | null = null
  let unlistenProgress: UnlistenFn | null = null

  $: breakingVersions = ($appUpdate?.notes ?? []).filter(n => n.breaking).map(n => n.version)

  async function checkNow() {
    checking = true
    await checkAppUpdate()
    checking = false
  }

  async function installUpdate() {
    if (!$appUpdate || installing) return
    installing = true
    installError = ''
    progress = { stage: 'downloading', pct: 0 }
    unlistenProgress ??= await listen<{ stage: string; pct: number }>('app-update-progress', e => { progress = e.payload })
    try {
      // On success the app exits and relaunches, so nothing after this runs.
      await invoke('install_app_update', { version: $appUpdate.latest })
    } catch (e) {
      installError = String(e)
      progress = null
    }
    installing = false
  }

  function formatDate(iso: string | null): string {
    return iso ? new Date(iso).toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' }) : ''
  }

  onDestroy(() => unlistenProgress?.())

  type Section = SettingsSection
  // Backed by a store so other pages can deep-link to a section.
  let activeSection: Section = $settingsSection
  settingsSection.set('client')

  const navItems: { id: Section; label: string }[] = [
    { id: 'client', label: 'Marvel Heroes Omega' },
    { id: 'display', label: 'Display' },
    { id: 'integrations', label: 'Integrations' },
    { id: 'about',  label: 'About' },
  ]

  const version = __APP_VERSION__

  async function updateOpt<K extends keyof LaunchOptions>(key: K, value: LaunchOptions[K]) {
    const updated = { ...$appConfig.launch_options, [key]: value }
    await setLaunchOptions(updated)
  }

  type BoolKey = { [K in keyof LaunchOptions]: LaunchOptions[K] extends boolean ? K : never }[keyof LaunchOptions]
  // Ordered by how often / how much they matter. `invert`: toggle shows the opposite of the stored flag.
  const launchToggles: { key: BoolKey; name: string; desc: string; invert?: boolean }[] = [
    { key: 'auto_login', name: 'Auto Login',
      desc: 'Passes your password as a launch argument. Note that the client writes it in plain text to its Launch.log' },
    { key: 'no_steam', invert: true, name: 'Steam Mode',
      desc: 'Enable to launch via Steam, if your copy of the game is from there. Disable to use Robocopy' },
    { key: 'force_32bit', name: 'Force 32-bit Client', desc: 'Runs the Win32 executable instead of Win64, if both are installed' },
    { key: 'skip_startup_movies', name: 'Skip Startup Movies', desc: 'Disables logo movies on launch' },
    { key: 'skip_motion_comics', name: 'Skip Motion Comics', desc: 'Disables in-game motion comic cutscenes' },
    { key: 'no_sound', name: 'No Sound', desc: 'Disables all game audio on launch' },
    { key: 'custom_resolution', name: 'Custom Resolution', desc: 'Forces a resolution not available in-game options' },
    { key: 'enable_client_log', name: 'Enable Client Log',
      desc: 'Opens a verbose log window alongside the game. Logs can contain your launch command, including your password' },
    { key: 'patched_client', name: 'Patched Client',
      desc: 'Enable if using Crypto137\'s port 8080 client patch. Changes the SiteConfig URL path for local servers' },
  ]

  const textSizes = [0.9, 1, 1.1, 1.25]

  const themes = [
    { id: '',              label: 'Blue' },
    { id: 'mh-itembase',   label: 'Item Base' },
    { id: 'light',         label: 'Light' },
    { id: 'phoenix',       label: 'Phoenix' },
    { id: 'tahiti',        label: 'Tahiti' },
    { id: 'grey',          label: 'Grey' },
    { id: 'blue-grey',     label: 'Slate' },
    { id: 'high-contrast', label: 'High Contrast' },
  ]
</script>

<div class="app-panel">
  <div class="panel-bg"></div>
  <div class="grid-overlay"></div>

  <div class="app-layout">

    <PanelSidebar width="var(--sidebar-narrow)">
      <svelte:fragment slot="header">
        <div class="section-title">Settings</div>
      </svelte:fragment>
      <nav class="settings-nav">
        {#each navItems as item}
          <div
            class="nav-item"
            class:selected={activeSection === item.id}
            on:click={() => activeSection = item.id}
            role="button"
            tabindex="0"
            on:keydown={(e) => e.key === 'Enter' && (activeSection = item.id)}
          >
            {item.label}
            {#if item.id === 'about' && $showUpdateBadge}<span class="nav-badge" use:tip={'Update available'}></span>{/if}
          </div>
        {/each}
      </nav>
    </PanelSidebar>

    <!-- Detail pane -->
    <div class="settings-detail">

      {#if activeSection === 'client'}
        <div class="detail-section">
          <div class="detail-head">
            <div class="section-title">Marvel Heroes Omega</div>
          </div>
          <div class="detail-body content-col">

            <LinkedVersions />

            <div class="section-divider"><span>Launch Options</span></div>

            {#each launchToggles.filter(t => t.key !== 'no_steam' || $activeCaps.steamMode) as t (t.key)}
              {@const on = t.invert ? !$appConfig.launch_options[t.key] : $appConfig.launch_options[t.key]}
              <div class="opt-row">
                <div class="opt-label">
                  <span class="opt-name">{t.name}</span>
                  <span class="opt-desc">{t.desc}</span>
                </div>
                <div
                  class="toggle-switch"
                  class:on
                  role="switch"
                  aria-checked={on}
                  tabindex="0"
                  on:click={() => updateOpt(t.key, !$appConfig.launch_options[t.key])}
                  on:keydown={(e) => e.key === 'Enter' && updateOpt(t.key, !$appConfig.launch_options[t.key])}
                ></div>
              </div>
              {#if t.key === 'custom_resolution' && on}
                <div class="resolution-row">
                  <div class="resolution-field">
                    <span class="field-label">Width</span>
                    <input
                      type="number"
                      value={$appConfig.launch_options.resolution_width || ''}
                      placeholder="e.g. 2560"
                      on:change={(e) => updateOpt('resolution_width', parseInt((e.target as HTMLInputElement).value) || 0)}
                    />
                  </div>
                  <span class="resolution-sep">×</span>
                  <div class="resolution-field">
                    <span class="field-label">Height</span>
                    <input
                      type="number"
                      value={$appConfig.launch_options.resolution_height || ''}
                      placeholder="e.g. 1440"
                      on:change={(e) => updateOpt('resolution_height', parseInt((e.target as HTMLInputElement).value) || 0)}
                    />
                  </div>
                </div>
              {/if}
            {/each}

            <div class="path-group">
              <span class="field-label">Additional Arguments</span>
              <input
                type="text"
                value={$appConfig.launch_options.extra_args}
                placeholder="e.g. -nosplash -nomovies"
                on:change={(e) => updateOpt('extra_args', (e.target as HTMLInputElement).value.trim())}
              />
              <span class="opt-desc">Added to the end of the launch command, separated by spaces</span>
            </div>

          </div>
        </div>

      {:else if activeSection === 'display'}
        <div class="detail-section">
          <div class="detail-head">
            <div class="section-title">Display</div>
          </div>
          <div class="detail-body content-col">
            <div class="subsection-title">Text size</div>
            <div class="size-row">
              <div class="seg" role="radiogroup" aria-label="Text size">
                {#each textSizes as s}
                  <button
                    role="radio"
                    aria-checked={$appConfig.text_size === s}
                    class:on={$appConfig.text_size === s}
                    on:click={() => setTextSize(s)}
                  >{Math.round(s * 100)}%</button>
                {/each}
              </div>
              <span class="opt-desc">Scales text and layout together</span>
            </div>

            <div class="subsection-title">Theme</div>
            <div class="theme-grid">
              {#each themes as theme}
                {@const active = $activeTheme === theme.id}
                <button class="theme-card" class:active aria-pressed={active} on:click={() => setTheme(theme.id)}>
                  <!-- Each preview resets to the default palette, then its own theme on top -->
                  <div class="theme-preview" data-theme-preview data-theme={theme.id || undefined} aria-hidden="true">
                    <span class="pv-side"><span class="pv-item"></span><span class="pv-item"></span><span class="pv-item"></span></span>
                    <span class="pv-main">
                      <span class="pv-line pv-title"></span>
                      <span class="pv-line"></span>
                      <span class="pv-line pv-short"></span>
                      <span class="pv-btn"></span>
                    </span>
                  </div>
                  <span class="theme-name">
                    {theme.label}
                    {#if active}<span class="theme-active">Active</span>{/if}
                  </span>
                </button>
              {/each}
            </div>
          </div>
        </div>

      {:else if activeSection === 'integrations'}
        <div class="detail-section">
          <div class="detail-head">
            <div class="section-title">Integrations</div>
          </div>
          <div class="detail-body">
            <OverlayIntegration />
            <ProfileImport />
          </div>
        </div>

      {:else if activeSection === 'about'}
        <div class="detail-section">
          <div class="detail-head">
            <div class="section-title">About</div>
          </div>
          <div class="detail-body">
            <div class="about-row">
              <div class="about-logo">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="color: var(--accent)">
                  <polygon points="22,12 17,20.66 7,20.66 2,12 7,3.34 17,3.34" />
                  <polygon points="17,15 12,18 7,15 7,9 12,6 17,9" opacity="0.45" stroke-width="1.5" />
                </svg>
              </div>
              <div class="about-text">
                <h3>MH Multiverse</h3>
                <p>{version} -- Tauri 2 + Svelte 5 + Rust</p>
              </div>
              <button class="btn btn-outline btn-sm github-link" on:click={() => openUrl(REPO_URL)}>View on GitHub</button>
            </div>

            <div class="section-divider"><span>Updates</span></div>

            {#if $appUpdate && !$appUpdate.up_to_date}
              <div class="update-card">
                <div class="update-head">
                  <div>
                    <div class="update-title">Update available: v{$appUpdate.current} → v{$appUpdate.latest}</div>
                    <div class="update-sub">Released {formatDate($appUpdate.notes[0]?.published_at ?? null)}</div>
                  </div>
                  <button class="btn btn-outline btn-sm" on:click={() => openUrl($appUpdate.html_url)}>Release page</button>
                </div>

                {#if breakingVersions.length}
                  <div class="breaking-box">
                    <strong>Breaking changes</strong> in v{breakingVersions.join(', v')}. Read the notes below before updating.
                  </div>
                {/if}

                {#if progress}
                  <div class="progress-wrap">
                    <div class="progress-bar"><div class="progress-fill" style="width: {progress.pct}%"></div></div>
                    <div class="progress-meta">
                      <span class="progress-stage">{STAGE_LABELS[progress.stage] ?? progress.stage}</span>
                      <span class="progress-pct">{Math.round(progress.pct)}%</span>
                    </div>
                  </div>
                {/if}
                {#if installError}<div class="update-error">{installError}</div>{/if}

                <div class="update-actions">
                  <button
                    class="btn btn-accent btn-sm"
                    disabled={installing || $serverRunning}
                    use:tip={$serverRunning ? 'Stop the server before updating' : ''}
                    on:click={installUpdate}
                  >{installing ? 'Updating…' : 'Update now'}</button>
                  {#if $showUpdateBadge}
                    <button class="btn btn-outline btn-sm" disabled={installing} on:click={() => updateRemindLater.set(true)}>Remind me later</button>
                    <button class="btn btn-outline btn-sm" disabled={installing} on:click={() => $appUpdate && dismissedUpdate.set($appUpdate.latest)}>Dismiss this version</button>
                  {/if}
                </div>
              </div>

              <div class="release-notes">
                {#each $appUpdate.notes as note, i (note.version)}
                  <details open={i === 0}>
                    <summary>v{note.version}{note.breaking ? ' (breaking changes)' : ''} <span class="update-sub">{formatDate(note.published_at)}</span></summary>
                    <div class="notes-body">
                      {#each parseReleaseNotes(note.body) as block}
                        {#if block.kind === 'hr'}
                          <hr />
                        {:else}
                          <div class="md-{block.kind}" class:md-h1={block.kind === 'h' && block.level <= 1} class:md-h2={block.kind === 'h' && block.level === 2} style={block.kind === 'li' ? `margin-left: ${block.depth * 14}px` : ''}>
                            {#each block.spans as s}{#if s.code}<code>{s.text}</code>{:else if s.bold}<strong>{s.text}</strong>{:else}{s.text}{/if}{/each}
                          </div>
                        {/if}
                      {/each}
                    </div>
                  </details>
                {/each}
              </div>
            {:else}
              <div class="update-card">
                <div class="update-head">
                  <div>
                    {#if $appUpdate}
                      <div class="update-title up-to-date">✓ Up to date (v{$appUpdate.current})</div>
                    {:else if $appUpdateError}
                      <div class="update-title">Couldn't check for updates</div>
                      <div class="update-error">{$appUpdateError}</div>
                    {:else}
                      <div class="update-title">Checking for updates…</div>
                    {/if}
                    {#if $appUpdateCheckedAt}<div class="update-sub">Last checked {$appUpdateCheckedAt.toLocaleTimeString()}</div>{/if}
                  </div>
                  <button class="btn btn-outline btn-sm" disabled={checking} on:click={checkNow}>{checking ? 'Checking…' : 'Check again'}</button>
                </div>
              </div>
            {/if}
          </div>
        </div>
      {/if}

    </div>
  </div>
</div>

<style>
  .app-panel {
    flex: 1;
    display: flex;
    flex-direction: column;
    position: relative;
    overflow: hidden;
  }



  .app-layout {
    position: relative;
    z-index: 1;
    flex: 1;
    display: grid;
    grid-template-columns: var(--sidebar-narrow) 1fr;
    overflow: hidden;
  }

  .settings-nav {
    flex: 1;
    overflow-y: auto;
    padding: 6px;
  }

  .nav-item {
    padding: 10px 12px;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: all 0.12s;
    margin-bottom: 2px;
    font-family: var(--font-head);
    font-size: 13px;
    font-weight: 600;
    color: var(--text-1);
  }
  .nav-item:hover {
    background: var(--bg-3);
    border-color: var(--border-mid);
    color: var(--text-0);
  }
  .nav-item.selected {
    background: var(--accent-glow);
    border-color: var(--accent-dim);
    color: var(--accent-bright);
  }

  /* -- Detail pane -- */
  .settings-detail {
    overflow-y: auto;
    background: var(--bg-1);
  }

  .detail-section {
    display: flex;
    flex-direction: column;
  }

  .detail-head {
    display: flex;
    align-items: center;
    padding: 12px 20px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    min-height: 53px;
  }

  .detail-body {
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  /* -- Section divider -- */
  /* -- Launch option rows -- */
  /* -- Resolution inputs -- */
  .resolution-row {
    display: flex;
    align-items: flex-end;
    gap: 10px;
    padding-left: 2px;
  }

  .resolution-field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    flex: 1;
  }

  .resolution-sep {
    font-size: 16px;
    color: var(--text-3);
    padding-bottom: 8px;
    flex-shrink: 0;
  }
  /* -- Display -- */
  .size-row {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-bottom: 22px;
  }
  .seg {
    display: inline-flex;
    gap: 2px;
    padding: 2px;
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    background: var(--bg-2);
  }
  .seg button {
    height: 28px;
    min-width: 56px;
    padding: 0 10px;
    border: 1px solid transparent;
    border-radius: 2px;
    background: transparent;
    color: var(--text-2);
    font-family: var(--font-head);
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.04em;
    font-variant-numeric: tabular-nums;
    cursor: pointer;
  }
  .seg button:hover { color: var(--text-0); }
  .seg button.on { color: var(--accent-bright); border-color: var(--accent-dim); background: var(--accent-glow); }

  .theme-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 12px;
  }
  .theme-card {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg-2);
    cursor: pointer;
    text-align: left;
    transition: border-color 0.12s, background 0.12s;
  }
  .theme-card:hover { border-color: var(--border-lit); background: var(--bg-3); }
  .theme-card.active { border-color: var(--accent); box-shadow: 0 0 0 1px var(--accent); }

  /* A miniature window drawn in the card's own theme */
  .theme-preview {
    display: flex;
    height: 72px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-0);
    overflow: hidden;
  }
  .pv-side {
    display: flex;
    flex-direction: column;
    gap: 5px;
    width: 30%;
    padding: 8px 6px;
    background: var(--bg-1);
    border-right: 1px solid var(--border);
  }
  .pv-item { height: 5px; border-radius: 1px; background: var(--text-3); opacity: 0.6; }
  .pv-item:first-child { background: var(--accent); opacity: 1; }
  .pv-main { flex: 1; display: flex; flex-direction: column; gap: 5px; padding: 8px; }
  .pv-line { height: 4px; width: 80%; border-radius: 1px; background: var(--text-2); }
  .pv-title { width: 55%; height: 5px; background: var(--text-0); }
  .pv-short { width: 45%; }
  .pv-btn {
    margin-top: auto;
    align-self: flex-end;
    width: 34px;
    height: 12px;
    border: 1px solid var(--accent);
    border-radius: 2px;
    background: var(--accent-glow);
  }

  .theme-name {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 2px;
    font-family: var(--font-head);
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.06em;
    color: var(--text-1);
  }
  .theme-card.active .theme-name { color: var(--accent-bright); }
  .theme-active {
    font-size: var(--fs-min);
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--accent);
  }

  /* -- About -- */
  .about-row {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 4px 0;
  }

  .about-logo {
    width: 40px;
    height: 40px;
    background: var(--accent-glow);
    border: 1px solid var(--accent-dim);
    border-radius: var(--radius-md);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--accent-dim);
    flex-shrink: 0;
  }

  .about-text h3 {
    font-family: var(--font-head);
    font-size: 14px;
    font-weight: 700;
    color: var(--text-0);
  }
  .about-text p {
    font-size: 12px;
    color: var(--text-2);
    margin-top: 2px;
  }
  .github-link { margin-left: auto; }

  /* -- Updates -- */
  .nav-item { position: relative; }
  .nav-badge {
    position: absolute;
    top: 50%;
    right: 12px;
    transform: translateY(-50%);
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--amber);
  }

  .update-card {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 14px 16px;
    background: var(--bg-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
  }
  .update-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .update-title {
    font-family: var(--font-head);
    font-size: 13px;
    font-weight: 600;
    color: var(--text-0);
  }
  .update-title.up-to-date { color: var(--text-success); }
  .update-sub {
    font-size: 11px;
    color: var(--text-3);
    margin-top: 2px;
  }
  .update-error {
    font-size: 12px;
    color: var(--text-error);
    word-break: break-word;
  }
  .update-actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .breaking-box {
    font-size: 12px;
    color: var(--text-1);
    padding: 8px 12px;
    border: 1px solid var(--amber);
    border-left-width: 3px;
    border-radius: var(--radius-sm);
  }
  .breaking-box strong { color: var(--amber); }


  .progress-wrap { display: flex; flex-direction: column; gap: 6px; }
  .progress-bar { height: 4px; background: var(--bg-3); border-radius: 2px; overflow: hidden; }
  .progress-fill { height: 100%; background: var(--accent); border-radius: 2px; transition: width 0.2s ease; }
  .progress-meta { display: flex; justify-content: space-between; align-items: center; }
  .progress-stage {
    font-family: var(--font-head);
    font-size: var(--fs-min);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-2);
  }
  .progress-pct { font-family: var(--font-mono); font-size: 11px; color: var(--accent-bright); }

  .release-notes { display: flex; flex-direction: column; gap: 8px; }
  .release-notes details {
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg-2);
  }
  .release-notes summary {
    cursor: pointer;
    padding: 10px 14px;
    font-family: var(--font-head);
    font-size: 12px;
    font-weight: 600;
    color: var(--text-1);
  }
  .notes-body {
    padding: 4px 16px 14px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    line-height: 1.5;
    color: var(--text-1);
  }
  .notes-body .md-h {
    font-family: var(--font-head);
    font-weight: 600;
    color: var(--text-0);
    margin-top: 8px;
  }
  .notes-body .md-h1 { font-size: 15px; }
  .notes-body .md-h2 { font-size: 13px; color: var(--accent-bright); }
  .notes-body .md-li { padding-left: 14px; position: relative; }
  .notes-body .md-li::before { content: '•'; position: absolute; left: 2px; color: var(--text-3); }
  .notes-body code {
    font-family: var(--font-mono);
    font-size: 11px;
    padding: 1px 4px;
    background: var(--bg-3);
    border-radius: 3px;
  }
  .notes-body hr { border: none; border-top: 1px solid var(--border); margin: 6px 0; }
</style>