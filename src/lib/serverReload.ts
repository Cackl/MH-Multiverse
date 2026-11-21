import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { catalogReloadResult } from './storeSimple'
import { reloadResult } from './tuningMeta'

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`

/**
 * Sends a reload command and resolves with an error message, or '' when it worked.
 * send_command only proves the text reached the server console, so the result comes from
 * the log lines that follow it. `check` returns null until the lines show a result.
 */
async function reloadAndWatch(cmd: string, check: (lines: string[]) => string | null): Promise<string> {
  const lines: string[] = []
  let done: (msg: string) => void = () => {}
  const result = new Promise<string>(resolve => (done = resolve))
  const unlisten = await listen<{ msg: string }[]>('server-log', e => {
    lines.push(...e.payload.map(l => l.msg))
    const msg = check(lines)
    if (msg !== null) done(msg)
  })
  const timer = setTimeout(() => done('No reply from the server. See the Server log.'), 5000)
  invoke('send_command', { cmd }).catch(e => done(`Reload not sent: ${e}`))
  try {
    return await result
  } finally {
    unlisten()
    clearTimeout(timer)
  }
}

export function reloadCatalog(): Promise<string> {
  return reloadAndWatch('!server reloadcatalog', lines => {
    const r = catalogReloadResult(lines)
    if (!r) return null
    return r.skipped > 0 ? `${plural(r.skipped, 'catalog file', 'catalog files')} skipped. See the Server log.` : ''
  })
}

export function reloadLiveTuning(): Promise<string> {
  return reloadAndWatch('!server reloadlivetuning', lines => {
    const r = reloadResult(lines)
    if (!r) return null
    return r.rejected > 0 ? `${plural(r.rejected, 'entry', 'entries')} rejected. See the Server log.` : ''
  })
}
