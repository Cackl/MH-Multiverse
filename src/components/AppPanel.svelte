<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog'
  import { onDestroy } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { listen, type UnlistenFn } from '@tauri-apps/api/event'
  import { openUrl } from '@tauri-apps/plugin-opener'
  import {
    appConfig, setGameExe, setServerExe, activeTheme, setTheme, setLaunchOptions, type LaunchOptions,
    appUpdate, appUpdateError, appUpdateCheckedAt, checkAppUpdate, showUpdateBadge,
    updateRemindLater, dismissedUpdate, serverRunning,
  } from '../lib/store'
  import { parseReleaseNotes } from '../lib/releaseNotes'
  import PanelSidebar from './PanelSidebar.svelte'

  const REPO_URL = 'https://github.com/Cackl/MH-Multiverse'
  const STAGE_LABELS: Record<string, string> = {
    downloading: 'Downloading',
    verifying: 'Verifying signature',
    installing: 'Installing — restarting shortly',
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

  type Section = 'client' | 'theme' | 'about'
  let activeSection: Section = 'client'

  const navItems: { id: Section; label: string }[] = [
    { id: 'client', label: 'Marvel Heroes Omega' },
    { id: 'theme',  label: 'Theme' },
    { id: 'about',  label: 'About' },
  ]

  const version = __APP_VERSION__

  async function browseGameExe() {
    const selected = await open({
      filters: [{ name: 'Executable', extensions: ['exe'] }],
      multiple: false,
    })
    if (selected && typeof selected === 'string') {
      await setGameExe(selected)
    }
  }

  async function browseServerExe() {
    const selected = await open({
      filters: [{ name: 'Executable', extensions: ['exe'] }],
      multiple: false,
    })
    if (selected && typeof selected === 'string') {
      await setServerExe(selected)
    }
  }

  async function updateOpt<K extends keyof LaunchOptions>(key: K, value: LaunchOptions[K]) {
    const updated = { ...$appConfig.launch_options, [key]: value }
    await setLaunchOptions(updated)
  }

  const themes = [
    { id: '',              label: 'Blue',            accent: '#3ea7c7' },
    { id: 'mh-itembase',   label: 'Item Base',       accent: '#10c3ff' },
    { id: 'light',         label: 'Light',           accent: '#f4f7fa' },
    { id: 'phoenix',       label: 'Phoenix',         accent: '#d96a1d' },
    { id: 'tahiti',        label: 'Tahiti',          accent: '#b31618' },
    { id: 'grey',          label: 'Grey',            accent: '#8f8f8f' },
    { id: 'blue-grey',     label: 'HC Blue',         accent: '#4a5a7a' }
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
            {#if item.id === 'about' && $showUpdateBadge}<span class="nav-badge" title="Update available"></span>{/if}
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
          <div class="detail-body">

            <div class="path-group">
              <span class="field-label">Server Executable</span>
              <div class="path-row">
                <div class="field-value path-value">
                  {$appConfig.server_exe || 'Not set'}
                </div>
                <button class="btn btn-sm btn-outline" on:click={browseServerExe}>Browse</button>
              </div>
            </div>
            <div class="path-group">
              <span class="field-label">Game Executable</span>
              <div class="path-row">
                <div class="field-value path-value">
                  {$appConfig.game_exe || 'Not set'}
                </div>
                <button class="btn btn-sm btn-outline" on:click={browseGameExe}>Browse</button>
              </div>
            </div>

            <div class="section-divider"><span>Launch Options</span></div>

            <div class="opt-row">
              <div class="opt-label">
                <span class="opt-name">Auto Login</span>
                <span class="opt-desc">Passes credentials as launch arguments. Does not work if launching through Steam</span>
              </div>
              <div
                class="toggle-switch"
                class:on={$appConfig.launch_options.auto_login}
                role="switch"
                aria-checked={$appConfig.launch_options.auto_login}
                tabindex="0"
                on:click={() => updateOpt('auto_login', !$appConfig.launch_options.auto_login)}
                on:keydown={(e) => e.key === 'Enter' && updateOpt('auto_login', !$appConfig.launch_options.auto_login)}
              ></div>
            </div>

            <div class="opt-row">
              <div class="opt-label">
                <span class="opt-name">Patched Client</span>
                <span class="opt-desc">Enable if using Crypto137's port 8080 client patch. Changes the SiteConfig URL path for local servers</span>
              </div>
              <div
                class="toggle-switch"
                class:on={$appConfig.launch_options.patched_client}
                role="switch"
                aria-checked={$appConfig.launch_options.patched_client}
                tabindex="0"
                on:click={() => updateOpt('patched_client', !$appConfig.launch_options.patched_client)}
                on:keydown={(e) => e.key === 'Enter' && updateOpt('patched_client', !$appConfig.launch_options.patched_client)}
              ></div>
            </div>

            <div class="opt-row">
              <div class="opt-label">
                <span class="opt-name">Custom Resolution</span>
                <span class="opt-desc">Forces a resolution not available in-game options</span>
              </div>
              <div
                class="toggle-switch"
                class:on={$appConfig.launch_options.custom_resolution}
                role="switch"
                aria-checked={$appConfig.launch_options.custom_resolution}
                tabindex="0"
                on:click={() => updateOpt('custom_resolution', !$appConfig.launch_options.custom_resolution)}
                on:keydown={(e) => e.key === 'Enter' && updateOpt('custom_resolution', !$appConfig.launch_options.custom_resolution)}
              ></div>
            </div>
            {#if $appConfig.launch_options.custom_resolution}
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

            <div class="opt-row">
              <div class="opt-label">
                <span class="opt-name">Skip Startup Movies</span>
                <span class="opt-desc">Disables logo movies on launch</span>
              </div>
              <div
                class="toggle-switch"
                class:on={$appConfig.launch_options.skip_startup_movies}
                role="switch"
                aria-checked={$appConfig.launch_options.skip_startup_movies}
                tabindex="0"
                on:click={() => updateOpt('skip_startup_movies', !$appConfig.launch_options.skip_startup_movies)}
                on:keydown={(e) => e.key === 'Enter' && updateOpt('skip_startup_movies', !$appConfig.launch_options.skip_startup_movies)}
              ></div>
            </div>

            <div class="opt-row">
              <div class="opt-label">
                <span class="opt-name">Skip Motion Comics</span>
                <span class="opt-desc">Disables in-game motion comic cutscenes</span>
              </div>
              <div
                class="toggle-switch"
                class:on={$appConfig.launch_options.skip_motion_comics}
                role="switch"
                aria-checked={$appConfig.launch_options.skip_motion_comics}
                tabindex="0"
                on:click={() => updateOpt('skip_motion_comics', !$appConfig.launch_options.skip_motion_comics)}
                on:keydown={(e) => e.key === 'Enter' && updateOpt('skip_motion_comics', !$appConfig.launch_options.skip_motion_comics)}
              ></div>
            </div>

            <div class="opt-row">
              <div class="opt-label">
                <span class="opt-name">No Sound</span>
                <span class="opt-desc">Disables all game audio on launch</span>
              </div>
              <div
                class="toggle-switch"
                class:on={$appConfig.launch_options.no_sound}
                role="switch"
                aria-checked={$appConfig.launch_options.no_sound}
                tabindex="0"
                on:click={() => updateOpt('no_sound', !$appConfig.launch_options.no_sound)}
                on:keydown={(e) => e.key === 'Enter' && updateOpt('no_sound', !$appConfig.launch_options.no_sound)}
              ></div>
            </div>

            <div class="opt-row">
              <div class="opt-label">
                <span class="opt-name">Enable Client Log</span>
                <span class="opt-desc">Opens a verbose log window alongside the game</span>
              </div>
              <div
                class="toggle-switch"
                class:on={$appConfig.launch_options.enable_client_log}
                role="switch"
                aria-checked={$appConfig.launch_options.enable_client_log}
                tabindex="0"
                on:click={() => updateOpt('enable_client_log', !$appConfig.launch_options.enable_client_log)}
                on:keydown={(e) => e.key === 'Enter' && updateOpt('enable_client_log', !$appConfig.launch_options.enable_client_log)}
              ></div>
            </div>

            <div class="opt-row">
              <div class="opt-label">
                <span class="opt-name">Robocopy</span>
                <span class="opt-desc">Launches in standalone mode, required for private servers</span>
              </div>
              <div
                class="toggle-switch"
                class:on={$appConfig.launch_options.robocopy}
                role="switch"
                aria-checked={$appConfig.launch_options.robocopy}
                tabindex="0"
                on:click={() => updateOpt('robocopy', !$appConfig.launch_options.robocopy)}
                on:keydown={(e) => e.key === 'Enter' && updateOpt('robocopy', !$appConfig.launch_options.robocopy)}
              ></div>
            </div>

            <div class="opt-row">
              <div class="opt-label">
                <span class="opt-name">No Steam</span>
                <span class="opt-desc">Disables Steam integration on launch</span>
              </div>
              <div
                class="toggle-switch"
                class:on={$appConfig.launch_options.no_steam}
                role="switch"
                aria-checked={$appConfig.launch_options.no_steam}
                tabindex="0"
                on:click={() => updateOpt('no_steam', !$appConfig.launch_options.no_steam)}
                on:keydown={(e) => e.key === 'Enter' && updateOpt('no_steam', !$appConfig.launch_options.no_steam)}
              ></div>
            </div>

          </div>
        </div>

      {:else if activeSection === 'theme'}
        <div class="detail-section">
          <div class="detail-head">
            <div class="section-title">Theme</div>
          </div>
          <div class="detail-body">
            <div class="theme-list">
              {#each themes as theme}
                <div
                  class="theme-row"
                  class:active={$activeTheme === theme.id}
                  on:click={() => setTheme(theme.id)}
                  role="button"
                  tabindex="0"
                  on:keydown={(e) => e.key === 'Enter' && setTheme(theme.id)}
                >
                  <span class="theme-dot" style="background: {theme.accent}"></span>
                  <span class="theme-label">{theme.label}</span>
                  {#if $activeTheme === theme.id}
                    <span class="theme-active-dot"></span>
                  {/if}
                </div>
              {/each}
            </div>
            <div class="theme-hint"></div>
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
                    title={$serverRunning ? 'Stop the server before updating' : ''}
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
                    <summary>v{note.version}{note.breaking ? ' — breaking changes' : ''} <span class="update-sub">{formatDate(note.published_at)}</span></summary>
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
  .section-divider {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 4px 0;
  }
  .section-divider span {
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--text-3);
    white-space: nowrap;
  }
  .section-divider::after {
    content: '';
    flex: 1;
    height: 1px;
    background: var(--border-mid);
  }

  /* -- Launch option rows -- */
  .opt-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }

  .opt-label {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .opt-name {
    font-family: var(--font-head);
    font-size: 12px;
    font-weight: 600;
    color: var(--text-1);
  }

  .opt-desc {
    font-size: 11px;
    color: var(--text-3);
  }

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
  .path-group {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .path-row {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .path-value {
    flex: 1;
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* -- Theme -- */
  .theme-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 600px;
    overflow-y: auto;
    border: 1px solid var(--border-mid);
    border-radius: var(--radius-sm);
    padding: 4px;
  }

  .theme-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: all 0.12s;
  }
  .theme-row:hover {
    background: var(--bg-3);
    border-color: var(--border-mid);
  }
  .theme-row.active {
    background: var(--accent-glow);
    border-color: var(--accent-dim);
  }

  .theme-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .theme-label {
    font-family: var(--font-head);
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.06em;
    color: var(--text-1);
    flex: 1;
  }
  .theme-row.active .theme-label {
    color: var(--accent-bright);
  }

  .theme-active-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    flex-shrink: 0;
  }

  .theme-hint {
    font-size: 11px;
    color: var(--text-3);
    font-style: italic;
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
    font-size: 10px;
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