import { writable } from 'svelte/store'

// One `search_prototypes` result. Mirrors `PrototypeMatch` in calligraphy.rs.
export interface PrototypeMatch {
  path: string
  blueprint: string
  display_name: string
  leaf: string
  class: string
  /** Where `display_name` came from. */
  name_source: 'custom' | 'game' | 'bundled' | 'path'
  /** Decimal string, to keep full u64 precision. */
  prototype_id: string
  /** Decimal string, to keep full u64 precision. */
  prototype_guid: string
}

export type PrototypeSearchState = {
  results: PrototypeMatch[]
  searching: boolean
  error: string
  open: boolean
}

const MIN_QUERY_LENGTH = 2
const DEBOUNCE_MS = 200
const CLOSED: PrototypeSearchState = { results: [], searching: false, error: '', open: false }

export function createPrototypeSearch(search: (query: string) => Promise<PrototypeMatch[]>) {
  const state = writable(CLOSED)
  let timer: ReturnType<typeof setTimeout> | undefined
  let latest = 0 // a response only counts if no newer input or close happened since its request

  function cancel() {
    clearTimeout(timer)
    latest++
  }

  function close() {
    cancel()
    state.set(CLOSED)
  }

  async function run(query: string) {
    const id = ++latest
    state.update(s => ({ ...s, searching: true, error: '' }))
    try {
      const results = await search(query)
      if (id === latest) state.set({ ...CLOSED, results, open: results.length > 0 })
    } catch (e) {
      if (id === latest) state.set({ ...CLOSED, error: String(e) })
    }
  }

  // Keeps the current results showing while the next search waits.
  function input(query: string) {
    if (query.trim().length < MIN_QUERY_LENGTH) return close()
    cancel()
    timer = setTimeout(() => run(query), DEBOUNCE_MS)
  }

  return { state, input, close }
}

type Rect = { top: number; bottom: number; left: number; width: number }

const DROPDOWN_MIN_WIDTH = 460
const DROPDOWN_MAX_HEIGHT = 280
const WINDOW_MARGIN = 8
const INPUT_GAP = 3

// Fixed position, so the modals' overflow can't clip it. Opens on whichever side has room.
export function dropdownStyle(input: Rect, windowWidth: number, windowHeight: number): string {
  const width = Math.min(Math.max(input.width, DROPDOWN_MIN_WIDTH), windowWidth - 2 * WINDOW_MARGIN)
  const left = Math.max(WINDOW_MARGIN, Math.min(input.left, windowWidth - width - WINDOW_MARGIN))
  const below = windowHeight - input.bottom - INPUT_GAP - WINDOW_MARGIN
  const above = input.top - INPUT_GAP - WINDOW_MARGIN
  const openBelow = below >= DROPDOWN_MAX_HEIGHT || below >= above
  const edge = openBelow ? `top:${input.bottom + INPUT_GAP}px` : `bottom:${windowHeight - input.top + INPUT_GAP}px`
  const maxHeight = Math.min(DROPDOWN_MAX_HEIGHT, openBelow ? below : above)
  return `${edge};left:${left}px;width:${width}px;max-height:${maxHeight}px;`
}
