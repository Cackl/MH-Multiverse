import { open } from '@tauri-apps/plugin-dialog'
import { setGameExe, setServerExe } from './store'

async function pickExe(): Promise<string | null> {
  const selected = await open({
    filters: [{ name: 'Executable', extensions: ['exe'] }],
    multiple: false,
  })
  return typeof selected === 'string' ? selected : null
}

export async function browseGameExe() {
  const path = await pickExe()
  if (path) await setGameExe(path)
}

export async function browseServerExe() {
  const path = await pickExe()
  if (path) await setServerExe(path)
}
