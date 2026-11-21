import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { get } from 'svelte/store'
import { createPrototypeSearch, dropdownStyle, type PrototypeMatch } from '../../src/lib/prototypes'

const match = (path: string) => ({ path }) as PrototypeMatch

describe('createPrototypeSearch', () => {
  let pending: { query: string; resolve: (r: PrototypeMatch[]) => void; reject: (e: unknown) => void }[]
  let search: ReturnType<typeof createPrototypeSearch>
  const queries = () => pending.map(p => p.query)
  const state = () => get(search.state)

  beforeEach(() => {
    vi.useFakeTimers()
    pending = []
    search = createPrototypeSearch(query => new Promise((resolve, reject) => {
      pending.push({ query, resolve, reject })
    }))
  })

  afterEach(() => vi.useRealTimers())

  it.each(['', 'a', ' a '])('does not search for %j', query => {
    search.input(query)
    vi.advanceTimersByTime(1000)
    expect(queries()).toEqual([])
  })

  it('waits out the debounce and only sends the last input', () => {
    search.input('do')
    vi.advanceTimersByTime(199)
    search.input('doo')
    vi.advanceTimersByTime(199)
    expect(queries()).toEqual([])
    vi.advanceTimersByTime(1)
    expect(queries()).toEqual(['doo'])
  })

  it('opens with results', async () => {
    search.input('doom')
    vi.advanceTimersByTime(200)
    pending[0].resolve([match('a')])
    await vi.advanceTimersByTimeAsync(0)
    expect(state()).toEqual({ results: [match('a')], error: '', open: true })
  })

  it('stays closed with no results', async () => {
    search.input('doom')
    vi.advanceTimersByTime(200)
    pending[0].resolve([])
    await vi.advanceTimersByTimeAsync(0)
    expect(state().open).toBe(false)
  })

  it('shows an error', async () => {
    search.input('doom')
    vi.advanceTimersByTime(200)
    pending[0].reject('no sip')
    await vi.advanceTimersByTimeAsync(0)
    expect(state()).toEqual({ results: [], error: 'no sip', open: false })
  })

  it('ignores a slow response once a newer search has started', async () => {
    search.input('do')
    vi.advanceTimersByTime(200)
    search.input('doom')
    vi.advanceTimersByTime(200)
    pending[1].resolve([match('new')])
    pending[0].resolve([match('old')])
    await vi.advanceTimersByTimeAsync(0)
    expect(state().results).toEqual([match('new')])
  })

  it('ignores a slow failure once a newer search has started', async () => {
    search.input('do')
    vi.advanceTimersByTime(200)
    search.input('doom')
    vi.advanceTimersByTime(200)
    pending[1].resolve([match('new')])
    pending[0].reject('old failure')
    await vi.advanceTimersByTimeAsync(0)
    expect(state()).toEqual({ results: [match('new')], error: '', open: true })
  })

  it('keeps showing results while the next search waits', async () => {
    search.input('do')
    vi.advanceTimersByTime(200)
    pending[0].resolve([match('a')])
    await vi.advanceTimersByTimeAsync(0)
    search.input('doo')
    expect(state().open).toBe(true)
  })

  it('close cancels a waiting search', () => {
    search.input('doom')
    search.close()
    vi.advanceTimersByTime(1000)
    expect(queries()).toEqual([])
  })

  it('close ignores a response already on its way', async () => {
    search.input('doom')
    vi.advanceTimersByTime(200)
    search.close()
    pending[0].resolve([match('a')])
    await vi.advanceTimersByTimeAsync(0)
    expect(state()).toEqual({ results: [], error: '', open: false })
  })

  it('a too-short input closes the dropdown', async () => {
    search.input('do')
    vi.advanceTimersByTime(200)
    pending[0].resolve([match('a')])
    await vi.advanceTimersByTimeAsync(0)
    search.input('d')
    expect(state().open).toBe(false)
  })
})

describe('dropdownStyle', () => {
  const input = (top: number, left = 100, width = 200) => ({ top, bottom: top + 30, left, width })
  const cases: [string, ReturnType<typeof input>, number, number, string][] = [
    ['opens below with room',            input(100),          1000, 800, 'top:133px;left:100px;width:460px;max-height:280px;'],
    ['opens above near the bottom',      input(700),          1000, 800, 'bottom:103px;left:100px;width:460px;max-height:280px;'],
    ['limits height to the room below',  input(350),          1000, 700, 'top:383px;left:100px;width:460px;max-height:280px;'],
    ['shrinks to fit a short window',    input(200),          1000, 500, 'top:233px;left:100px;width:460px;max-height:259px;'],
    ['keeps a wide input width',         input(100, 50, 600), 1000, 800, 'top:133px;left:50px;width:600px;max-height:280px;'],
    ['moves left to stay on screen',     input(100, 800),     1000, 800, 'top:133px;left:532px;width:460px;max-height:280px;'],
    ['narrows in a narrow window',       input(100, 20),      300,  800, 'top:133px;left:8px;width:284px;max-height:280px;'],
  ]
  it.each(cases)('%s', (_name, rect, w, h, want) => {
    expect(dropdownStyle(rect, w, h)).toBe(want)
  })
})
