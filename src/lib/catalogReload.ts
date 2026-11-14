import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { catalogReloadResult } from './storeSimple'

/**
 * Sends `!server reloadcatalog` and resolves with an error message, or '' when it worked.
 * send_command only proves the text reached the server console, so the result comes from
 * the log lines that follow it.
 */
export async function reloadCatalog(): Promise<string> {
  const lines: string[] = []
  let done: (msg: string) => void = () => {}
  const result = new Promise<string>(resolve => (done = resolve))
  const unlisten = await listen<{ msg: string }[]>('server-log', e => {
    lines.push(...e.payload.map(l => l.msg))
    const r = catalogReloadResult(lines)
    if (r) done(r.skipped > 0 ? `${r.skipped} catalog ${r.skipped === 1 ? 'file' : 'files'} skipped. See the Server log.` : '')
  })
  const timer = setTimeout(() => done('No reply from the server. See the Server log.'), 5000)
  invoke('send_command', { cmd: '!server reloadcatalog' }).catch(e => done(`Reload not sent: ${e}`))
  try {
    return await result
  } finally {
    unlisten()
    clearTimeout(timer)
  }
}
