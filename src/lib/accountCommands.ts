import { get } from 'svelte/store'
import { invoke } from '@tauri-apps/api/core'
import { appConfig, startServer, serverStartError, serverReady, serverLog } from './store'

// Account changes go through the server's own console commands, so the server validates and
// stores them exactly as it would for an in-game admin. Account.db is only ever read here
// (read-only): to skip the server entirely when the account is already right, and to confirm.

export type AccountInput = { email: string; playerName: string; password: string }

export type SyncInput = AccountInput & {
  makeAdmin?: boolean
  // Set the password on the server when the account exists with a different one.
  // Off, that case is reported as `mismatch` so the user decides.
  overwritePassword?: boolean
}

export type AccountResult =
  | { ok: true; created: boolean; startedServer: boolean; warning?: string }
  | { ok: false; error: string; mismatch?: boolean; timedOut?: boolean }

export type Timing = {
  sleep?: (ms: number) => Promise<void>
  pollMs?: number
  // Covers a cold start: the server loads all game data before it reads commands.
  startTimeoutMs?: number
  // The server replies as soon as it runs the command, after its database write.
  replyTimeoutMs?: number
  // Progress text for the UI while the (possibly slow) steps run.
  onStatus?: (message: string) => void
}

// Console arguments are split on whitespace and a newline would run a second command,
// so none of these may appear in anything sent.
const UNSAFE = /[\s\x00-\x1f\x7f]/

function checkArg(label: string, value: string): string {
  if (!value) return `${label} is required.`
  if (UNSAFE.test(value)) return `${label} can't contain spaces.`
  return ''
}

// The server's AccountManager.ValidatePassword: 3 to 64 characters.
export function validatePassword(password: string): string {
  return checkArg('Password', password)
    || (password.length < 3 || password.length > 64 ? 'Password must be 3 to 64 characters.' : '')
}

// Mirrors AccountManager.ValidateEmail: at most 320 characters, an "@", and a final "." after it
// followed by at least two characters. The server lowercases before checking.
function validateEmail(email: string): string {
  const unsafe = checkArg('Email', email)
  if (unsafe) return unsafe
  const at = email.indexOf('@')
  const dot = email.lastIndexOf('.')
  if (email.length > 320 || at < 0 || dot < at || email.length - dot - 1 < 2) {
    return 'Enter a valid email address, like name@example.com.'
  }
  return ''
}

// PlayerNameValidator allows 1 to 16 letters or digits. The blocked-name list is checked
// separately (blocked_player_names), since it lives in the server's Data folder.
function validatePlayerName(name: string): string {
  if (!name) return 'Player name is required.'
  return /^[a-zA-Z0-9]{1,16}$/.test(name) ? '' : 'Player name must be 1 to 16 letters or numbers.'
}

export function validateNewAccount({ email, playerName, password }: AccountInput): string {
  return validateEmail(email) || validatePlayerName(playerName) || validatePassword(password)
}

const realSleep = (ms: number) => new Promise<void>(r => setTimeout(r, ms))

type Account = { email: string; player_name: string }

async function serverIsRunning(): Promise<boolean> {
  return invoke<boolean>('server_is_running').catch(() => false)
}

// Starts the server if needed and waits for its "ready for commands" log line.
async function ensureReady(t: Required<Timing>): Promise<{ error: string } | { started: boolean }> {
  let started = false
  if (!(await serverIsRunning())) {
    t.onStatus('Starting the server...')
    await startServer()
    const startError = get(serverStartError)
    if (startError) return { error: startError }
    started = true
  }
  if (!get(serverReady)) t.onStatus('Waiting for the server to finish starting...')
  for (let waited = 0; !get(serverReady); waited += t.pollMs) {
    // ponytail: with no ready line (console logging turned off) this waits the full timeout, then
    // sends anyway; stdin buffers the command and the database check decides.
    if (waited >= t.startTimeoutMs) break
    if (waited > 0 && !(await serverIsRunning())) return { error: 'The server stopped while starting.' }
    await t.sleep(t.pollMs)
  }
  return { started }
}

// The server's replies, from AccountCommands.cs. Each is logged as one line once the command ran.
const REPLIES = {
  create: { ok: 'Created a new account: ', fail: 'Failed to create account: ' },
  password: { ok: 'Successfully changed password for account ', fail: 'Failed to change password: ' },
  userlevel: { ok: 'Successfully set user level for account ', fail: 'Failed to set user level: ' },
}

type Reply = { ok: true } | { ok: false; error: string } | { sendError: string } | null

// Sends one console command and waits for the server's reply. null = no reply in time.
async function send(cmd: string, reply: { ok: string; fail: string }, t: Required<Timing>): Promise<Reply> {
  const log = get(serverLog)
  const from = log.length ? log[log.length - 1].id : -1
  try {
    await invoke('send_command', { cmd })
  } catch (e) {
    return { sendError: String(e) }
  }
  for (let waited = 0; waited < t.replyTimeoutMs; waited += t.pollMs) {
    for (const { id, msg } of get(serverLog)) {
      if (id <= from) continue
      if (msg.includes(reply.ok)) return { ok: true }
      const i = msg.indexOf(reply.fail)
      if (i >= 0) return { ok: false, error: msg.slice(i + reply.fail.length) }
    }
    await t.sleep(t.pollMs)
  }
  return null
}

function timing(t: Timing): Required<Timing> {
  return { sleep: realSleep, pollMs: 100, startTimeoutMs: 90_000, replyTimeoutMs: 5_000, onStatus: () => {}, ...t }
}

/**
 * Makes the server account match a local profile. Nothing is sent when the email and password
 * already match; otherwise the account is created, or its password set when `overwritePassword`.
 * The server is only started when a command has to be sent.
 */
export async function syncAccount(input: SyncInput, t: Timing = {}): Promise<AccountResult> {
  const { email, playerName, password, makeAdmin, overwritePassword } = input
  const invalid = checkArg('Email', email) || validatePassword(password)
  if (invalid) return { ok: false, error: invalid }
  const serverExe = get(appConfig).server_exe
  const matches = () => invoke<boolean>('account_password_matches', { serverExe, email, password }).catch(() => false)

  // Errors while Account.db does not exist yet (the server creates it on first start): treat as missing.
  const exists = await invoke<boolean>('account_exists', { serverExe, email }).catch(() => false)
  if (exists && (await matches())) return { ok: true, created: false, startedServer: false }
  if (exists && !overwritePassword) {
    return { ok: false, mismatch: true, error: 'This account already exists on the server with a different password.' }
  }

  if (!exists) {
    const bad = validateNewAccount(input)
    if (bad) return { ok: false, error: bad }
    const accounts = await invoke<Account[]>('list_accounts_for_import', { serverExe }).catch((): Account[] => [])
    if (accounts.some(a => a.player_name.toLowerCase() === playerName.toLowerCase())) {
      return { ok: false, error: 'That player name is already in use.' }
    }
    const blocked = await invoke<string[]>('blocked_player_names', { serverExe }).catch((): string[] => [])
    if (blocked.includes(playerName.toLowerCase())) {
      return { ok: false, error: "That player name isn't allowed on this server." }
    }
  }

  const timings = timing(t)
  const server = await ensureReady(timings)
  if ('error' in server) return { ok: false, error: server.error }

  timings.onStatus(exists ? 'Changing the password...' : 'Creating the account...')
  const r = exists
    ? await send(`!account password ${email} ${password}`, REPLIES.password, timings)
    : await send(`!account create ${email} ${playerName} ${password}`, REPLIES.create, timings)
  if (r && 'sendError' in r) return { ok: false, error: r.sendError }
  if (r && !r.ok) return { ok: false, error: `The server refused: ${r.error}` }
  // Even after a success reply, the database is what login reads.
  if (!(await matches())) {
    if (!(await serverIsRunning())) return { ok: false, error: 'The server stopped before the change could be confirmed.' }
    return { ok: false, error: 'The server did not confirm the change. Check the Server console for its reply.', timedOut: true }
  }

  let warning: string | undefined
  if (!exists && makeAdmin) {
    const admin = await send(`!account userlevel ${email} 2`, REPLIES.userlevel, timings)
    if (!admin || !('ok' in admin) || !admin.ok) warning = 'Making it an admin account was not confirmed. Check the Server console.'
  }
  return { ok: true, created: !exists, startedServer: server.started, warning }
}
