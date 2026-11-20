import { open } from '@tauri-apps/plugin-dialog'
import { setGameExe, setServerExe } from './store'

export async function pickExe(): Promise<string | null> {
  const selected = await open({
    filters: [{ name: 'Executable', extensions: ['exe'] }],
    multiple: false,
  })
  return typeof selected === 'string' ? selected : null
}

/** These return the save error, or '' when saved or cancelled. */
export async function browseGameExe(): Promise<string> {
  const path = await pickExe()
  return path ? setGameExe(path) : ''
}

export async function browseServerExe(): Promise<string> {
  const path = await pickExe()
  return path ? setServerExe(path) : ''
}
