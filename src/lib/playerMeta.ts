export type PlayerSession = {
  session_id: string
  username: string
  email?: string
  user_level?: number       // 0 = Player, 1 = Moderator, 2 = Admin
  flags?: number            // bit 0 = banned
  gazillionite_balance?: number
  last_logout_time?: number // unix timestamp
  avatar_count?: number
  guild_name?: string
}

export const USER_LEVEL_LABELS: Record<number, string> = {
  0: 'Player',
  1: 'Moderator',
  2: 'Admin',
}

export const USER_LEVEL_OPTIONS = [
  { value: 0, label: 'Player' },
  { value: 1, label: 'Moderator' },
  { value: 2, label: 'Admin' },
]

export function userLevelLabel(level?: number): string {
  return USER_LEVEL_LABELS[level ?? 0] ?? 'Unknown'
}

export function isBanned(flags?: number): boolean {
  return (flags === 2);
}

export function isWhitelisted(flags?: number): boolean {
  return (flags === 16);
}

export function formatLastSeen(ts?: number): string {
  if (!ts || ts === 0) return 'Never'
  return new Date(ts * 1000).toLocaleString()
}