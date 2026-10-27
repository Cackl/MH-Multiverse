<script lang="ts">
  import { confirmRequest } from '../lib/store'

  let stayBtn: HTMLButtonElement | undefined

  // Focus the safe choice when a question appears.
  $: if ($confirmRequest && stayBtn) stayBtn.focus()

  function answer(ok: boolean) {
    $confirmRequest?.resolve(ok)
  }

  function onKeydown(e: KeyboardEvent) {
    if ($confirmRequest && e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); answer(false) }
  }
</script>

<svelte:window on:keydown|capture={onKeydown} />

{#if $confirmRequest}
  <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-static-element-interactions -->
  <div class="modal-backdrop" on:click={e => { if (e.target === e.currentTarget) answer(false) }}>
    <div class="confirm-modal" role="alertdialog" aria-modal="true" aria-labelledby="confirm-title" aria-describedby="confirm-message">
      <div class="confirm-header">
        <span id="confirm-title" class="confirm-title">{$confirmRequest.title}</span>
      </div>
      <p id="confirm-message" class="confirm-message">{$confirmRequest.message}</p>
      <div class="confirm-footer">
        <button class="btn btn-sm btn-outline" bind:this={stayBtn} on:click={() => answer(false)}>
          {$confirmRequest.cancelLabel}
        </button>
        <button class="btn btn-sm btn-red" on:click={() => answer(true)}>
          {$confirmRequest.okLabel}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.72);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: var(--z-modal);
    backdrop-filter: blur(3px);
  }

  .confirm-modal {
    width: min(420px, calc(100vw - 32px));
    background: var(--bg-1);
    border: 1px solid var(--border-lit);
    border-radius: var(--radius-md);
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.6);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .confirm-header {
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
  }

  .confirm-title {
    font-family: var(--font-head);
    font-size: 13px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-0);
  }

  .confirm-message {
    margin: 0;
    padding: 18px 16px;
    font-size: 12px;
    line-height: 1.5;
    color: var(--text-1);
  }

  .confirm-footer {
    padding: 12px 16px;
    border-top: 1px solid var(--border);
    background: var(--chrome-footer-bg);
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }
</style>
