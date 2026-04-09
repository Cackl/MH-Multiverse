<script lang="ts">
  import { onMount } from 'svelte'
  import { invoke } from '@tauri-apps/api/core'
  import { listen } from '@tauri-apps/api/event'
  import { fly } from 'svelte/transition'
  import PlayerCard from './PlayerCard.svelte'
  import { type PlayerSession, userLevelLabel, isBanned, isWhitelisted } from '../lib/playerMeta'

  export let open = false
  export let playerCount = 0

  type PlayerEventPayload = {
    kind: string
    session_id?: string | null
    username?: string | null
    count: number
  }

  let players: PlayerSession[] = []
  let searchQuery = ''
  let selectedPlayer: PlayerSession | null = null

  $: searchLower = searchQuery.toLowerCase()
  $: filteredPlayers = players.filter(p => {
    if (!searchLower) return true
    return (
      p.username.toLowerCase().includes(searchLower) ||
      (p.email ?? '').toLowerCase().includes(searchLower) ||
      p.session_id.toLowerCase().includes(searchLower)
    )
  })

  $: if (selectedPlayer) {
    const updated = players.find(p => p.session_id === selectedPlayer!.session_id)
    if (updated) selectedPlayer = updated
    else selectedPlayer = null
  }

  async function loadPlayers() {
    players = await invoke<PlayerSession[]>('get_players')
    playerCount = players.length
  }

  function togglePlayer(player: PlayerSession) {
    selectedPlayer = selectedPlayer?.session_id === player.session_id ? null : player
  }

  onMount(() => {
    loadPlayers()
    const unlistenPromise = listen<PlayerEventPayload>('player-event', async () => {
      await loadPlayers()
    })
    return () => { unlistenPromise.then(u => u()) }
  })
</script>

{#if open}
  <!-- <div class="players-blade" transition:fly={{ x: 400, duration: 180 }}> -->
  <div class="players-blade">

    <div class="blade-header">
      <div class="blade-title-row">
        <span class="blade-title">Players</span>
        <span class="blade-count">{playerCount}</span>
        <div class="blade-spacer"></div>
        <button class="close-btn" on:click={() => (open = false)} title="Close">
          <svg viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5">
            <line x1="2" y1="2" x2="12" y2="12"/>
            <line x1="12" y1="2" x2="2" y2="12"/>
          </svg>
        </button>
      </div>
      <input
        class="blade-search"
        type="text"
        placeholder="Search name, email, session…"
        bind:value={searchQuery}
        spellcheck="false"
      />
    </div>

    <div class="blade-list">
      {#if filteredPlayers.length === 0}
        <div class="blade-empty">
          {searchQuery ? 'No players match.' : 'No players online.'}
        </div>
      {:else}
        {#each filteredPlayers as player (player.session_id)}
          <button
            class="player-row"
            class:selected={selectedPlayer?.session_id === player.session_id}
            class:banned={isBanned(player.flags)}
            class:whitelisted={isWhitelisted(player.flags)}
            on:click={() => togglePlayer(player)}
          >
            <div class="player-row-top">
              <span class="player-name">{player.username}</span>
              {#if player.user_level && player.user_level > 0}
                <span class="level-badge level-{player.user_level}">{userLevelLabel(player.user_level)}</span>
              {/if}
              {#if isBanned(player.flags)}
                <span class="banned-badge">BANNED</span>
              {/if}
              {#if isWhitelisted(player.flags)}
                <span class="whitelisted-badge">WHITELISTED</span>
              {/if}
            </div>
            <span class="session-mono">{player.session_id}</span>
          </button>
        {/each}
      {/if}
    </div>

    {#if selectedPlayer !== null}
      <div class="blade-detail">
        <PlayerCard player={selectedPlayer} compact={true} />
      </div>
    {/if}

  </div>
{/if}

<style>
  .players-blade {
    position: absolute;
    top: 0;
    right: 0;
    bottom: 0;
    width: 400px;
    background: var(--bg-1);
    border-left: 1px solid var(--border-lit);
    display: flex;
    flex-direction: column;
    z-index: 10;
    box-shadow: -6px 0 28px rgba(0, 0, 0, 0.35);
  }

  /* -- Header -- */
  .blade-header {
    flex-shrink: 0;
    background: var(--bg-0);
    border-bottom: 1px solid var(--border);
  }

  .blade-title-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px 8px;
  }

  .blade-title {
    font-family: var(--font-head);
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--text-1);
  }

  .blade-count {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--green-bright);
    background: var(--green-dim);
    border: 1px solid rgba(39, 174, 96, 0.25);
    border-radius: 3px;
    padding: 1px 5px;
    line-height: 1.4;
  }

  .blade-spacer { flex: 1; }

  .close-btn {
    background: none;
    border: none;
    cursor: pointer;
    color: var(--text-3);
    padding: 2px;
    border-radius: var(--radius-sm);
    display: flex;
    align-items: center;
    justify-content: center;
    transition: color 0.1s;
  }
  .close-btn:hover { color: var(--text-1); }
  .close-btn svg { width: 12px; height: 12px; }

  .blade-search {
    display: block;
    width: 100%;
    box-sizing: border-box;
    padding: 7px 12px 9px;
    background: none;
    border: none;
    border-top: 1px solid var(--border);
    color: var(--text-0);
    font-family: var(--font-body);
    font-size: 12px;
    outline: none;
  }
  .blade-search::placeholder { color: var(--text-3); }

  /* -- Player list -- */
  .blade-list {
    flex: 1;
    min-height: 120px;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 6px;
  }

  .blade-list::-webkit-scrollbar { width: 4px; }
  .blade-list::-webkit-scrollbar-thumb { background: var(--border-lit); }

  .blade-empty {
    padding: 20px 16px;
    font-size: 12px;
    color: var(--text-3);
    text-align: center;
  }

  .player-row {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    text-align: left;
    padding: 7px 12px;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    background: transparent;
    cursor: pointer;
    transition: all 0.12s;
    margin-bottom: 2px;
  }
  .player-row:last-child { margin-bottom: 0; }
  .player-row:hover { background: var(--bg-3); border-color: var(--border-mid); }
  .player-row.selected { background: var(--accent-glow); border-color: var(--accent-dim); }
  .player-row.selected:hover { background: var(--accent-glow); border-color: var(--accent-dim); }
  .player-row.banned { opacity: 0.55; }

  .player-row-top {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }

  .player-name {
    font-size: 13px;
    color: var(--text-0);
    font-weight: 500;
    font-family: var(--font-body)
  }

  .session-mono {
    font-family: var(--font-body);
    font-size: 10px;
    color: var(--text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .level-badge,
  .banned-badge {
    font-family: var(--font-head);
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    padding: 1px 5px;
    border-radius: 2px;
    border: 1px solid transparent;
    flex-shrink: 0;
  }
  .whitelisted-badge {
    font-family: var(--font-head);
    font-size: 8px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    padding: 1px 5px;
    border-radius: 2px;
    border: 1px solid transparent;
    flex-shrink: 0;
  }

  .level-1 {
    color: var(--accent);
    border-color: var(--accent-dim);
    background: var(--accent-glow);
  }

  .level-2 {
    color: var(--accent-bright);
    border-color: var(--accent-dim);
    background: var(--accent-glow);
  }

  .banned-badge {
    color: var(--text-error);
    border-color: rgba(200, 50, 50, 0.4);
    background: rgba(200, 50, 50, 0.1);
  }
  .whitelisted-badge {
    color: var(--text-1);
    border-color: rgba(219, 219, 219, 0.4);
    background: rgba(182, 182, 182, 0.1);
  }

  /* -- Detail section -- */
  .blade-detail {
    flex-shrink: 0;
    max-height: 55%;
    overflow-y: auto;
    border-top: 1px solid var(--border-lit);
  }

  .blade-detail::-webkit-scrollbar { width: 4px; }
  .blade-detail::-webkit-scrollbar-thumb { background: var(--border-lit); }
</style>