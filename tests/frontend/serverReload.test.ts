import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { reloadCatalog, reloadLiveTuning } from '../../src/lib/serverReload'

type Handler = (e: { payload: { msg: string }[] }) => void
let handler: Handler | null
let unlisten: ReturnType<typeof vi.fn>

/** Feeds log lines to the watcher, as the server-log event would. */
const log = (...msgs: string[]) => handler!({ payload: msgs.map(msg => ({ msg })) })
const flush = () => new Promise(r => setTimeout(r, 0))

beforeEach(() => {
  handler = null
  unlisten = vi.fn()
  vi.mocked(listen).mockImplementation(async (_event, h) => { handler = h as Handler; return unlisten })
  vi.mocked(invoke).mockResolvedValue(undefined)
})
afterEach(() => vi.useRealTimers())

describe('reloadLiveTuning', () => {
  it('sends the command and resolves with nothing when the load works', async () => {
    const p = reloadLiveTuning()
    await flush()
    expect(invoke).toHaveBeenCalledWith('send_command', { cmd: '!server reloadlivetuning' })
    log('Loaded 120 Live Tuning settings')
    expect(await p).toBe('')
    expect(unlisten).toHaveBeenCalledOnce()
  })

  it('counts rejected entries across log batches', async () => {
    const p = reloadLiveTuning()
    await flush()
    log('Verify failed: Failed to parse LiveTuningDataUserX.json')
    log('Verify failed: bad entry', 'Loaded 3 Live Tuning settings')
    expect(await p).toBe('2 entries rejected. See the Server log.')
  })

  it('says 1 entry, not 1 entries', async () => {
    const p = reloadLiveTuning()
    await flush()
    log('Verify failed: x', 'Loaded 3 Live Tuning settings')
    expect(await p).toBe('1 entry rejected. See the Server log.')
  })

  it('reports no reply after 5 seconds and stops listening', async () => {
    vi.useFakeTimers()
    let settled = false
    const p = reloadLiveTuning().finally(() => (settled = true))
    await vi.advanceTimersByTimeAsync(4999)
    log('Some other line')
    expect(settled).toBe(false)
    await vi.advanceTimersByTimeAsync(1)
    expect(await p).toBe('No reply from the server. See the Server log.')
    expect(unlisten).toHaveBeenCalledOnce()
  })

  it('reports a command that was not sent', async () => {
    vi.mocked(invoke).mockRejectedValue('Server not running')
    expect(await reloadLiveTuning()).toBe('Reload not sent: Server not running')
    expect(unlisten).toHaveBeenCalledOnce()
  })
})

describe('reloadCatalog', () => {
  it('sends its own command and reports skipped files', async () => {
    const p = reloadCatalog()
    await flush()
    expect(invoke).toHaveBeenCalledWith('send_command', { cmd: '!server reloadcatalog' })
    log('Failed to deserialize CatalogX.json as CatalogEntry', 'Loaded 900 store catalog entries')
    expect(await p).toBe('1 catalog file skipped. See the Server log.')
  })

  it('ignores Live Tuning lines', async () => {
    const p = reloadCatalog()
    await flush()
    log('Loaded 120 Live Tuning settings', 'Loaded 900 store catalog entries')
    expect(await p).toBe('')
  })
})
