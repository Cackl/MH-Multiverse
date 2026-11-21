import type { LinkedInstall } from './store'

/** New exes on an entry: the version follows detection, client first as the backend does. A failed
 *  detection keeps the version the user picked. A name that was empty or only the old version follows it. */
export function withExes(l: LinkedInstall, game_exe: string, server_exe: string,
                         client: string | null, server: string | null): LinkedInstall {
  const version = client ?? server ?? l.version
  const name = l.name === '' || l.name === l.version ? version : l.name
  return { ...l, game_exe, server_exe, version, name }
}

/** The folder a server exe sits in, for a row's second line. */
export function serverFolder(serverExe: string): string {
  return serverExe.split(/[\\/]/).slice(-2, -1)[0] ?? ''
}

/** An empty entry for Add version. It keeps the current Steam setting, since the first one added becomes active. */
export function newLinked(id: string, noSteam: boolean): LinkedInstall {
  return { id, name: '', version: '', game_exe: '', server_exe: '', custom: false, no_steam: noSteam }
}
