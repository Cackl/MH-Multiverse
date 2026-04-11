<script lang="ts">
  import { tick } from 'svelte'
  import { upsertServer, type Server } from '../lib/store'

  export let server: Server | null = null
  export let onClose: () => void

  let name = server?.name ?? ''
  let host = server?.host ?? ''
  let email = server?.email ?? ''
  let password = ''
  let saving = false
  let error = ''
  let isLocal  = server?.is_local  ?? true
  let useHttps = server?.use_https ?? false

  const isEdit = server !== null

  async function save() {
    name = name.trim()
    host = host.trim()
    email = email.trim()
    password = password.trim()

    if (!name) { error = 'Name is required.'; return }
    if (!isLocal && !host) { error = 'Host is required for non-local servers.'; return }
    if (!isEdit && !password) { error = 'Password is required for a new server.'; return }

    saving = true
    error = ''

    try {
      const entry: Server = {
        id: server?.id ?? crypto.randomUUID(),
        name,
        host: isLocal ? '' : host,
        email,
        is_local: isLocal,
        use_https: useHttps,
      }
      await upsertServer(entry, password)
      onClose()
    } catch (e) {
      error = String(e)
    } finally {
      saving = false
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') onClose()
  }

  // -- Tooltip --

  let tooltip = ''
  let tooltipX = 0
  let tooltipY = 0
  let tooltipVisible = false
  let tooltipEl: HTMLDivElement | null = null

  const TOOLTIP_OFFSET = 12
  const VIEWPORT_PAD = 10

  async function showTooltip(e: MouseEvent, text: string) {
    tooltip = text
    tooltipVisible = true
    await tick()
    if (!tooltipEl) return
    const rect = tooltipEl.getBoundingClientRect()
    const vw = window.innerWidth
    const vh = window.innerHeight
    let x = e.clientX + TOOLTIP_OFFSET
    let y = e.clientY - 8
    if (x + rect.width + VIEWPORT_PAD > vw) x = e.clientX - rect.width - TOOLTIP_OFFSET
    x = Math.max(VIEWPORT_PAD, Math.min(x, vw - rect.width - VIEWPORT_PAD))
    if (y < VIEWPORT_PAD) y = e.clientY + TOOLTIP_OFFSET
    y = Math.max(VIEWPORT_PAD, Math.min(y, vh - rect.height - VIEWPORT_PAD))
    tooltipX = x
    tooltipY = y
  }

  function hideTooltip() {
    tooltipVisible = false
  }
</script>

<svelte:window on:keydown={onKeydown} />

{#if tooltipVisible}
  <div bind:this={tooltipEl} class="tooltip" style="left:{tooltipX}px; top:{tooltipY}px">
    {tooltip}
  </div>
{/if}

<div class="modal-backdrop" role="dialog" aria-modal="true">
  <div class="modal">

    <div class="modal-header">
      <div class="modal-title">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <rect x="2" y="3" width="20" height="18" rx="2"/>
          <line x1="2" y1="9" x2="22" y2="9"/>
          <circle cx="7" cy="6" r="1" fill="currentColor" stroke="none"/>
          <circle cx="11" cy="6" r="1" fill="currentColor" stroke="none"/>
        </svg>
        <span>{isEdit ? 'Edit Server' : 'Add Server'}</span>
      </div>
      <button class="close-btn" aria-label="Close" on:click={onClose}>
        <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5">
          <line x1="3.5" y1="3.5" x2="10.5" y2="10.5"/>
          <line x1="10.5" y1="3.5" x2="3.5" y2="10.5"/>
        </svg>
      </button>
    </div>

    <form on:submit|preventDefault={save}>
      <div class="modal-body">

        <div class="form-group">
          <label class="field-label" for="modal-name">Name</label>
          <input id="modal-name" type="text" bind:value={name} placeholder="Local Server">
        </div>

        <label class="check-row">
          <input type="checkbox" bind:checked={isLocal}>
          <span class="check-label">Local</span>
          <button
            class="info-btn"
            type="button"
            on:mouseenter={(e) => showTooltip(e, 'Server runs on this machine - port and dashboard path are read from MHServerEmu Config.')}
            on:mouseleave={hideTooltip}
            tabindex="-1"
          >?</button>
        </label>

        {#if !isLocal}
          <div class="form-group">
            <div class="label-row">
              <label class="field-label" for="modal-host">Host / IP</label>
              <button
                class="info-btn"
                type="button"
                on:mouseenter={(e) => showTooltip(e, 'Hostname or IP address only - no http:// prefix or path suffix required. Include a port as hostname:port if not on standard 80/443.')}
                on:mouseleave={hideTooltip}
                tabindex="-1"
              >?</button>
            </div>
            <input id="modal-host" type="text" bind:value={host} placeholder="e.g. 192.168.xxx, mhphoenix.net, &lt;hostname&gt;:&lt;port&gt;">
          </div>
          <label class="check-row">
            <input type="checkbox" bind:checked={useHttps}>
            <span class="check-label">Use HTTPS</span>
            <button
              class="info-btn"
              type="button"
              on:mouseenter={(e) => showTooltip(e, 'Use HTTPS for the dashboard URL and SiteConfig requests. Requires SSL to be configured on the remote server.')}
              on:mouseleave={hideTooltip}
              tabindex="-1"
            >?</button>
          </label>
        {/if}

        <div class="form-group">
          <label class="field-label" for="modal-email">Email</label>
          <input id="modal-email" type="text" bind:value={email} placeholder="player1@local.host">
        </div>

        <div class="form-group">
          <label class="field-label" for="modal-password">
            Password
            {#if isEdit}<span class="hint">(leave blank to keep existing)</span>{/if}
          </label>
          <input id="modal-password" type="password" bind:value={password} placeholder={isEdit ? '............' : 'Required'}>
        </div>

        {#if error}
          <div class="error">{error}</div>
        {/if}
      </div>

      <div class="modal-footer">
        <button type="button" class="btn btn-outline" on:click={onClose} disabled={saving}>Cancel</button>
        <button type="submit" class="btn btn-accent" disabled={saving}>
          {saving ? 'Saving...' : isEdit ? 'Save Changes' : 'Add Server'}
        </button>
      </div>
    </form>

  </div>
</div>

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.65);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    backdrop-filter: blur(2px);
  }

  .modal {
    background: var(--bg-2);
    border: 1px solid var(--border-lit);
    border-radius: var(--radius-md);
    width: 440px;
    display: flex;
    flex-direction: column;
    box-shadow: 0 24px 80px rgba(0, 0, 0, 0.5), 0 0 0 1px rgba(255, 255, 255, 0.03) inset;
  }

  .modal-header {
    padding: 14px 16px;
    border-bottom: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .modal-title {
    display: flex;
    align-items: center;
    gap: 10px;
    font-family: var(--font-head);
    font-size: 14px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-0);
  }
  .modal-title svg {
    width: 18px;
    height: 18px;
    color: var(--accent);
  }

  .close-btn {
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: transparent;
    border: 1px solid transparent;
    color: var(--text-2);
    cursor: pointer;
    border-radius: var(--radius-sm);
    transition: all 0.12s;
  }
  .close-btn:hover {
    color: var(--text-0);
    background: var(--bg-3);
    border-color: var(--border-mid);
  }
  .close-btn svg { width: 14px; height: 14px; }

  .modal-body {
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .form-group {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .label-row {
    display: flex;
    align-items: center;
    gap: 5px;
  }
  .label-row .field-label {
    margin-bottom: 0;
  }

  .hint {
    font-family: var(--font-body);
    font-size: 10px;
    color: var(--text-3);
    text-transform: none;
    letter-spacing: 0;
    margin-left: 6px;
  }

  .error {
    font-size: 12px;
    color: #e74c3c;
    padding: 8px 10px;
    border: 1px solid rgba(192, 57, 43, 0.4);
    background: var(--red-dim);
    border-radius: var(--radius-sm);
  }

  .modal-footer {
    padding: 12px 16px;
    border-top: 1px solid var(--border);
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  /* -- Checkbox rows -- */
  .check-row {
    display: flex;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    user-select: none;
  }
  .check-row input[type="checkbox"] {
    width: 12px;
    height: 12px;
    cursor: pointer;
    accent-color: var(--accent);
    flex-shrink: 0;
  }
  .check-label {
    font-family: var(--font-head);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--text-1);
  }

  /* -- Info button -- */
  .info-btn {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    border: 1px solid var(--border-lit);
    background: var(--bg-3);
    color: var(--text-3);
    font-size: 9px;
    cursor: help;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    transition: all 0.12s;
    padding: 0;
  }
  .info-btn:hover { border-color: var(--accent-dim); color: var(--accent-bright); }

  /* -- Tooltip -- */
  .tooltip {
    position: fixed;
    z-index: calc(var(--z-modal) + 1);
    background: var(--bg-3);
    border: 1px solid var(--border-lit);
    border-radius: var(--radius-sm);
    color: var(--text-1);
    font-size: 12px;
    font-family: var(--font-body);
    padding: 6px 10px;
    max-width: 280px;
    line-height: 1.5;
    pointer-events: none;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
  }
</style>