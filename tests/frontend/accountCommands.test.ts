import { describe, it, expect, beforeEach, vi } from 'vitest'
import { get } from 'svelte/store'

const invokeMock = vi.hoisted(() => vi.fn())
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }))

import { appConfig, serverStartError, serverReady, serverRunning, appendLog, clearLog } from '../../src/lib/store'
import { validateNewAccount, validatePassword, syncAccount } from '../../src/lib/accountCommands'

const noWait = { sleep: async () => {}, pollMs: 1, startTimeoutMs: 5, replyTimeoutMs: 5 }
const good = { email: 'a@b.co', playerName: 'Player', password: 'pw1' }

type Fake = {
  // email -> [player name, password]; what Account.db holds.
  accounts?: Record<string, [string, string]>
  running?: boolean
  // Whether the "ready for commands" line has been logged; a successful start logs it.
  ready?: boolean
  startRuns?: boolean
  startError?: string
  sendError?: string
  blocked?: string[]
  // The server never answers commands.
  silent?: boolean
  // The server refuses every command with this reason.
  refuse?: string
}

const FAIL_PREFIX: Record<string, string> = {
  create: 'Failed to create account: ',
  password: 'Failed to change password: ',
  userlevel: 'Failed to set user level: ',
}

// Stands in for the Rust side and the server process. Commands change `accounts` and log the
// server's real reply text (AccountCommands.cs), so the flow is tested against what it prints.
function fakeBackend(f: Fake) {
  const accounts: Record<string, [string, string]> = { ...f.accounts }
  let running = f.running ?? true
  serverRunning.set(running)
  serverReady.set(f.ready ?? running)
  const calls: [string, any][] = []
  const reply = (msg: string) => appendLog({ time: '', level: 'info', msg: `[CommandManager] ${msg}` })
  invokeMock.mockImplementation(async (cmd: string, args?: any) => {
    calls.push([cmd, args])
    switch (cmd) {
      case 'server_is_running': return running
      case 'start_server':
        if (f.startError) throw new Error(f.startError)
        running = f.startRuns ?? true
        serverRunning.set(running)
        if (running) serverReady.set(true)
        return
      case 'list_accounts_for_import':
        return Object.entries(accounts).map(([email, [player_name]]) => ({ email, player_name }))
      case 'blocked_player_names': return f.blocked ?? []
      case 'account_exists': return args.email in accounts
      case 'account_password_matches': return accounts[args.email]?.[1] === args.password
      case 'send_command': {
        if (f.sendError) throw new Error(f.sendError)
        if (f.silent) return
        const [, sub, email, a, b] = args.cmd.split(' ')
        if (f.refuse) return reply(FAIL_PREFIX[sub] + f.refuse)
        if (sub === 'create') { accounts[email] = [a, b]; return reply(`Created a new account: ${email} (${a}).`) }
        if (sub === 'password') { accounts[email][1] = a; return reply(`Successfully changed password for account ${email}.`) }
        if (sub === 'userlevel') return reply(`Successfully set user level for account ${email} to Admin.`)
        return
      }
      default: throw new Error(`unexpected ${cmd}`)
    }
  })
  return {
    calls,
    accounts,
    sent: () => calls.filter(([c]) => c === 'send_command').map(([, a]) => a.cmd as string),
    stopServer: () => { running = false },
  }
}

beforeEach(() => {
  invokeMock.mockReset()
  serverStartError.set('')
  clearLog()
  appConfig.set({ ...get(appConfig), server_exe: 'C:/srv/MHServerEmu.exe' })
})

describe('validateNewAccount', () => {
  const cases: [string, Partial<typeof good>, string][] = [
    ['valid', {}, ''],
    ['empty email', { email: '' }, 'Email is required.'],
    ['empty name', { playerName: '' }, 'Player name is required.'],
    ['empty password', { password: '' }, 'Password is required.'],
    ['space in email', { email: 'a b@c.de' }, "Email can't contain spaces."],
    ['space in name', { playerName: 'Two Words' }, 'Player name must be 1 to 16 letters or numbers.'],
    ['space in password', { password: 'two words' }, "Password can't contain spaces."],
    ['newline in password', { password: 'pw\n!server shutdown' }, "Password can't contain spaces."],
    ['tab in name', { playerName: 'a\tb' }, 'Player name must be 1 to 16 letters or numbers.'],
    ['control character in email', { email: 'a\u0007@b.co' }, "Email can't contain spaces."],
    // Same rules as the server's AccountManager.ValidateEmail.
    ['email with no @', { email: 'me' }, 'Enter a valid email address, like name@example.com.'],
    ['email with no dot', { email: 'a@b' }, 'Enter a valid email address, like name@example.com.'],
    ['email with the dot before the @', { email: 'a.b@c' }, 'Enter a valid email address, like name@example.com.'],
    ['email with a one letter domain ending', { email: 'a@b.c' }, 'Enter a valid email address, like name@example.com.'],
    ['two letter domain ending', { email: 'a@b.co' }, ''],
    ['email over 320 characters', { email: `${'a'.repeat(320)}@b.co` }, 'Enter a valid email address, like name@example.com.'],
    ['email of exactly 320 characters', { email: `${'a'.repeat(314)}@b.co` }, ''],
    // Same rules as PlayerNameValidator.
    ['symbol in name', { playerName: 'Play_er' }, 'Player name must be 1 to 16 letters or numbers.'],
    ['name of 16 characters', { playerName: 'a'.repeat(16) }, ''],
    ['name of 17 characters', { playerName: 'a'.repeat(17) }, 'Player name must be 1 to 16 letters or numbers.'],
    // Same rules as AccountManager.ValidatePassword.
    ['password of 2 characters', { password: 'ab' }, 'Password must be 3 to 64 characters.'],
    ['password of 3 characters', { password: 'abc' }, ''],
    ['password of 64 characters', { password: 'a'.repeat(64) }, ''],
    ['password of 65 characters', { password: 'a'.repeat(65) }, 'Password must be 3 to 64 characters.'],
  ]
  it.each(cases)('%s', (_label, patch, expected) => {
    expect(validateNewAccount({ ...good, ...patch })).toBe(expected)
  })

  it('checks the password on its own for a password change', () => {
    expect(validatePassword('okay')).toBe('')
    expect(validatePassword('no way')).toBe("Password can't contain spaces.")
    expect(validatePassword('ab')).toBe('Password must be 3 to 64 characters.')
  })
})

describe('syncAccount', () => {
  it('never contacts the server for invalid input', async () => {
    const b = fakeBackend({})
    expect((await syncAccount({ ...good, password: 'x\ny' }, noWait)).ok).toBe(false)
    expect(b.calls).toEqual([])
  })

  it('saves straight away when the account already has this password, without starting the server', async () => {
    const b = fakeBackend({ running: false, accounts: { 'a@b.co': ['Player', 'pw1'] } })
    expect(await syncAccount(good, noWait)).toEqual({ ok: true, created: false, startedServer: false })
    expect(b.calls.map(([c]) => c)).not.toContain('start_server')
    expect(b.sent()).toEqual([])
  })

  it('reports a different password without changing it', async () => {
    const b = fakeBackend({ accounts: { 'a@b.co': ['Player', 'old'] } })
    expect(await syncAccount(good, noWait)).toMatchObject({ ok: false, mismatch: true })
    expect(b.sent()).toEqual([])
  })

  it('sets the password when asked to overwrite', async () => {
    const b = fakeBackend({ accounts: { 'a@b.co': ['Player', 'old'] } })
    expect(await syncAccount({ ...good, overwritePassword: true }, noWait)).toEqual({ ok: true, created: false, startedServer: false })
    expect(b.sent()).toEqual(['!account password a@b.co pw1'])
    expect(b.accounts['a@b.co'][1]).toBe('pw1')
  })

  it('creates a missing account', async () => {
    const b = fakeBackend({})
    expect(await syncAccount(good, noWait)).toEqual({ ok: true, created: true, startedServer: false })
    expect(b.sent()).toEqual(['!account create a@b.co Player pw1'])
  })

  it('makes a new account an admin when asked', async () => {
    const b = fakeBackend({})
    expect(await syncAccount({ ...good, makeAdmin: true }, noWait)).toEqual({ ok: true, created: true, startedServer: false })
    expect(b.sent()).toEqual(['!account create a@b.co Player pw1', '!account userlevel a@b.co 2'])
  })

  it('needs a player name to create', async () => {
    const b = fakeBackend({})
    expect(await syncAccount({ ...good, playerName: '' }, noWait)).toEqual({ ok: false, error: 'Player name is required.' })
    expect(b.sent()).toEqual([])
  })

  it('refuses a player name in use, ignoring case, without sending anything', async () => {
    const b = fakeBackend({ accounts: { 'x@y.zz': ['PLAYER', 'pw'] } })
    expect(await syncAccount(good, noWait)).toEqual({ ok: false, error: 'That player name is already in use.' })
    expect(b.sent()).toEqual([])
  })

  it('refuses a name on the server blacklist, ignoring case, without sending anything', async () => {
    const b = fakeBackend({ blocked: ['player'] })
    expect(await syncAccount({ ...good, playerName: 'PLAYER' }, noWait)).toEqual({
      ok: false,
      error: "That player name isn't allowed on this server.",
    })
    expect(b.sent()).toEqual([])
  })

  it('starts a stopped server first and says so', async () => {
    const b = fakeBackend({ running: false })
    expect(await syncAccount(good, noWait)).toEqual({ ok: true, created: true, startedServer: true })
    const order = b.calls.map(([c]) => c)
    expect(order.indexOf('start_server')).toBeLessThan(order.indexOf('send_command'))
  })

  it('reports progress, mentioning the server only when it has to be started', async () => {
    const seen: string[] = []
    fakeBackend({ running: false })
    await syncAccount(good, { ...noWait, onStatus: m => seen.push(m) })
    expect(seen).toEqual(['Starting the server...', 'Creating the account...'])

    seen.length = 0
    clearLog()
    fakeBackend({})
    await syncAccount(good, { ...noWait, onStatus: m => seen.push(m) })
    expect(seen).toEqual(['Creating the account...'])
  })

  it('waits for the ready line before sending', async () => {
    const b = fakeBackend({ ready: false })
    let ticks = 0
    const sleep = async () => { if (++ticks === 3) serverReady.set(true) }
    const seen: string[] = []
    const r = await syncAccount(good, { ...noWait, startTimeoutMs: 100, sleep, onStatus: m => seen.push(m) })
    expect(r).toMatchObject({ ok: true })
    expect(seen[0]).toBe('Waiting for the server to finish starting...')
    expect(ticks).toBe(3)
    expect(b.sent()).toHaveLength(1)
  })

  it('still sends when no ready line ever comes, and lets the database decide', async () => {
    const b = fakeBackend({ ready: false })
    expect(await syncAccount(good, noWait)).toMatchObject({ ok: true, created: true })
    expect(b.sent()).toHaveLength(1)
  })

  it('reports a server that fails to start without sending the command', async () => {
    const b = fakeBackend({ running: false, startError: 'Executable not found' })
    expect(await syncAccount(good, noWait)).toEqual({ ok: false, error: 'Error: Executable not found' })
    expect(b.sent()).toEqual([])
  })

  it('reports a server that stops while starting', async () => {
    const b = fakeBackend({ running: false, startRuns: false })
    expect(await syncAccount(good, noWait)).toEqual({ ok: false, error: 'The server stopped while starting.' })
    expect(b.sent()).toEqual([])
  })

  it("shows the server's own reason when it refuses", async () => {
    fakeBackend({ refuse: 'Email a@b.co is already used by another account.' })
    expect(await syncAccount(good, noWait)).toEqual({
      ok: false,
      error: 'The server refused: Email a@b.co is already used by another account.',
    })
  })

  it('ignores replies logged before the command was sent', async () => {
    fakeBackend({ silent: true })
    appendLog({ time: '', level: 'info', msg: 'Created a new account: a@b.co (Player).' })
    expect(await syncAccount(good, noWait)).toMatchObject({ ok: false, timedOut: true })
  })

  it('times out when the server never answers', async () => {
    fakeBackend({ silent: true })
    expect(await syncAccount(good, noWait)).toMatchObject({ ok: false, timedOut: true })
  })

  it('reports the server stopping before the change is confirmed', async () => {
    const b = fakeBackend({ silent: true })
    expect(await syncAccount(good, { ...noWait, sleep: async () => b.stopServer() })).toEqual({
      ok: false,
      error: 'The server stopped before the change could be confirmed.',
    })
  })

  it('surfaces a failed send', async () => {
    fakeBackend({ sendError: 'Server stdin is not available.' })
    expect(await syncAccount(good, noWait)).toEqual({ ok: false, error: 'Error: Server stdin is not available.' })
  })

  it('warns, but succeeds, when making the account an admin is not confirmed', async () => {
    const b = fakeBackend({})
    const base = invokeMock.getMockImplementation()!
    invokeMock.mockImplementation(async (cmd: string, args?: any) =>
      cmd === 'send_command' && args.cmd.includes('userlevel') ? undefined : base(cmd, args))
    const r = await syncAccount({ ...good, makeAdmin: true }, noWait)
    expect(r).toMatchObject({ ok: true, created: true })
    expect(r.ok && r.warning).toBeTruthy()
    expect(b.accounts['a@b.co']).toEqual(['Player', 'pw1'])
  })

  it('treats a missing Account.db as no account and creates one', async () => {
    fakeBackend({})
    const base = invokeMock.getMockImplementation()!
    let dbThere = false
    invokeMock.mockImplementation(async (cmd: string, args?: any) => {
      if (cmd === 'send_command') dbThere = true
      if (!dbThere && ['account_exists', 'list_accounts_for_import', 'account_password_matches'].includes(cmd)) {
        throw new Error('Account.db not found.')
      }
      return base(cmd, args)
    })
    expect(await syncAccount(good, noWait)).toMatchObject({ ok: true, created: true })
  })
})
