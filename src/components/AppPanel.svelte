<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog'
  import { appConfig, setGameExe, setServerExe, activeTheme, setTheme, setLaunchOptions, type LaunchOptions } from '../lib/store'
  import PanelSidebar from './PanelSidebar.svelte'

  type Section = 'client' | 'theme' | 'about'
  let activeSection: Section = 'client'

  const navItems: { id: Section; label: string }[] = [
    { id: 'client', label: 'Marvel Heroes Omega' },
    { id: 'theme',  label: 'Theme' },
    { id: 'about',  label: 'About' },
  ]

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
    { id: '',              label: 'Teal',            accent: '#3ea7c7' },
    { id: 'blue',          label: 'Blue',            accent: '#3ea7c7' },
    { id: 'mh-itembase',   label: 'Item Base',       accent: '#10c3ff' },
    { id: 'itembase-v2',   label: 'Item Base v2',    accent: '#00c8ff' },
    { id: 'light',         label: 'Light',           accent: '#f4f7fa' },
    { id: 'phoenix',       label: 'Phoenix',         accent: '#d96a1d' }
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
                <span class="opt-desc">Passes credentials as launch arguments</span>
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
            <div class="theme-hint">Selection is saved and restored on next launch. Additional themes coming soon.</div>
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
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" style="width:20px;height:20px;">
                  <polygon points="12,2 22,8.5 22,15.5 12,22 2,15.5 2,8.5"/>
                  <line x1="12" y1="2" x2="12" y2="22" opacity="0.3"/>
                  <line x1="2" y1="8.5" x2="22" y2="8.5" opacity="0.3"/>
                </svg>
              </div>
              <div class="about-text">
                <h3>MH Manifold</h3>
                <p>v0.1.0 -- Tauri 2 + Svelte 5 + Rust</p>
              </div>
            </div>
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
    padding: 14px 20px 12px;
    border-bottom: 1px solid var(--border);
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
    max-height: 180px;
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
</style>