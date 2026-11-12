// Which events the server runs on a given day, mirroring LiveTuningEventRule.cs in MHServerEmu.
// Dates are server clock times: UTC shifted by the server's offset and read with getUTC* methods.

import type { ScheduleRule } from './tuningMeta'

export const DAYS = ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday']
const DAY_MS = 86_400_000

/** The server clock: UTC plus its offset in hours (the game data sets -10). */
export function serverNow(offsetHours: number, utcMs = Date.now()): Date {
  return new Date(utcMs + offsetHours * 3_600_000)
}

const blank = (id: string) => id.trim() === ''

/**
 * Whether the server keeps the rule at start-up (IsValid). One unknown event id drops the whole
 * rule; blank slots are allowed.
 */
export function ruleIsValid(rule: ScheduleRule, knownIds: Set<string>): boolean {
  if (!rule.is_enabled || rule.events.length === 0) return false
  if (rule.events.some(id => !blank(id) && !knownIds.has(id))) return false
  switch (rule.rule_type) {
    case 'AlwaysOn':         return true
    case 'DayOfWeek':
    case 'WeeklyRotation':   return DAYS.includes(rule.start_day_of_week ?? '')
    case 'SpecialDate':
    case 'SpecialDateLunar': return rule.start_month != null && rule.start_day != null && rule.duration_days != null
    default:                 return false
  }
}

/** The rotation's week number, counted from the first start weekday of 2000, flipping at server midnight. */
function rotationWeek(rule: ScheduleRule, now: Date): number {
  const epoch = Date.UTC(2000, 0, 2 + DAYS.indexOf(rule.start_day_of_week ?? 'Sunday'))
  return Math.max(0, Math.trunc((now.getTime() - epoch) / DAY_MS / 7))
}

// Lunar New Year for 2000 to 2100 as month and day ("205" is 5 Feb), from .NET's
// ChineseLunisolarCalendar, which the server uses. ICU's Chinese calendar is off by a day in some
// years (2027, 2030), so Intl can't stand in for it.
const LUNAR_NEW_YEAR = ('205 124 212 201 122 209 129 218 207 126 214 203 123 210 131 219 208 128 216 205 ' +
  '125 212 201 122 210 129 217 206 126 214 203 123 211 131 219 208 128 215 204 124 212 201 122 210 130 ' +
  '217 206 126 214 202 123 211 201 219 208 128 215 204 124 212 202 121 209 129 217 205 126 214 203 123 ' +
  '211 131 219 207 127 215 205 124 212 202 122 209 129 217 206 126 214 203 124 210 130 218 207 127 215 ' +
  '205 125 212 201 121 209').split(' ')

/**
 * The Gregorian date (UTC midnight) of a lunar date in the lunar year that starts in `year`, or
 * null when it isn't known. ponytail: first lunar month only (every month has at least 29 days),
 * which covers the stock Lunar New Year rule; other months need .NET's month table.
 */
export function lunarToGregorian(year: number, month: number, day: number): Date | null {
  const md = LUNAR_NEW_YEAR[year - 2000]
  if (!md || month !== 1 || day < 1 || day > 29) return null
  return new Date(Date.UTC(year, Number(md.slice(0, -2)) - 1, Number(md.slice(-2)) + day - 1))
}

/** The event ids this rule switches on at `now`, assuming the server kept it (see ruleIsValid). */
export function ruleActiveIds(rule: ScheduleRule, now: Date): string[] {
  let on = false
  switch (rule.rule_type) {
    case 'AlwaysOn':
      on = true
      break
    case 'DayOfWeek':
      on = DAYS[now.getUTCDay()] === rule.start_day_of_week
      break
    case 'SpecialDate':
    case 'SpecialDateLunar': {
      // Only this year's window counts, so a window running past 31 Dec ends there.
      const year = now.getUTCFullYear()
      const start = rule.rule_type === 'SpecialDate'
        ? new Date(Date.UTC(year, rule.start_month! - 1, rule.start_day!))
        : lunarToGregorian(year, rule.start_month!, rule.start_day!)
      if (!start) return []
      // The server starts lunar windows a day early "to account for time zone differences".
      const from = start.getTime() - (rule.rule_type === 'SpecialDateLunar' ? DAY_MS : 0)
      on = now.getTime() >= from && now.getTime() < from + rule.duration_days! * DAY_MS
      break
    }
    case 'WeeklyRotation': {
      const id = rule.events[rotationWeek(rule, now) % rule.events.length]
      return blank(id) ? [] : [id]
    }
  }
  return on ? rule.events.filter(id => !blank(id)) : []
}

/** Every event id the server runs at `now`. */
export function activeEventIds(rules: ScheduleRule[], knownIds: Set<string>, now: Date): Set<string> {
  return new Set(rules.filter(r => ruleIsValid(r, knownIds)).flatMap(r => ruleActiveIds(r, now)))
}

/** The next event a rotation switches to after this week, skipping blank slots, and how many weeks away. */
export function nextInRotation(rule: ScheduleRule, now: Date): { id: string; weeks: number } | null {
  const week = rotationWeek(rule, now)
  for (let k = 1; k <= rule.events.length; k++) {
    const id = rule.events[(week + k) % rule.events.length]
    if (!blank(id)) return { id, weeks: k }
  }
  return null
}

// ── Simple view: one control per event ──────────────────────────────────────────

export type EventMode = 'schedule' | 'off' | 'always'

/** The rule Simple adds "Always on" events to, kept apart from the stock "Permanent Events". */
export const ALWAYS_RULE = 'Multiverse: Always on'

const lists = (r: ScheduleRule, id: string) => r.events.includes(id)
const named = (r: ScheduleRule) => r.events.filter(id => !blank(id))

/** Always on (in Simple's own rule), On schedule (in another enabled rule) or Off. */
export function eventMode(rules: ScheduleRule[], id: string): EventMode {
  if (rules.some(r => r.name === ALWAYS_RULE && r.is_enabled && lists(r, id))) return 'always'
  return rules.some(r => r.is_enabled && lists(r, id)) ? 'schedule' : 'off'
}

/**
 * The schedule after switching one event. Off takes it out of every rule: a stock rule that
 * only runs this event is switched off (so On schedule can bring it back), any other rule of
 * its own is removed, and rotation slots are blanked so the other weeks don't shift. On
 * schedule then puts it back where the stock schedule has it. Always on keeps that and adds it
 * to ALWAYS_RULE. ponytail: stock rules are matched by name, which is unique in v52.
 */
export function setEventMode(rules: ScheduleRule[], stock: ScheduleRule[], id: string, mode: EventMode): ScheduleRule[] {
  let out = rules.flatMap(r => {
    if (!lists(r, id)) return [r]
    if (named(r).some(x => x !== id)) {
      const events = r.rule_type === 'WeeklyRotation' ? r.events.map(x => x === id ? '' : x) : r.events.filter(x => x !== id)
      return [{ ...r, events }]
    }
    return stock.some(s => s.name === r.name) ? [{ ...r, is_enabled: false }] : []
  })
  if (mode === 'off') return out

  for (const s of stock.filter(s => lists(s, id))) {
    const i = out.findIndex(r => r.name === s.name)
    if (i < 0) continue // removed in Advanced
    const r = out[i]
    out[i] = named(s).every(x => x === id)
      ? { ...r, is_enabled: s.is_enabled, events: s.events }
      : r.rule_type === 'WeeklyRotation' && r.events.length === s.events.length
        ? { ...r, events: r.events.map((x, k) => s.events[k] === id ? id : x) }
        : { ...r, events: [...r.events, id] }
  }
  if (mode === 'schedule') return out

  const i = out.findIndex(r => r.name === ALWAYS_RULE)
  if (i >= 0) out[i] = { ...out[i], is_enabled: true, events: [...named(out[i]), id] }
  else out = [...out, {
    name: ALWAYS_RULE, is_enabled: true, rule_type: 'AlwaysOn', start_day_of_week: null,
    start_month: null, start_day: null, duration_days: null, events: [id],
  }]
  return out
}

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec']

function dateRange(month: number, day: number, days: number): string {
  const start = new Date(Date.UTC(2023, month - 1, day))
  // The window ends at 31 Dec, whatever its length (the server only checks this year).
  const end = new Date(Math.min(start.getTime() + (days - 1) * DAY_MS, Date.UTC(2023, 11, 31)))
  const d = (x: Date) => `${x.getUTCDate()} ${MONTHS[x.getUTCMonth()]}`
  if (days <= 1) return d(start)
  return start.getUTCMonth() === end.getUTCMonth() ? `${start.getUTCDate()} to ${d(end)}` : `${d(start)} to ${d(end)}`
}

/** When an event runs, in words, from the rules that list it. */
export function scheduleText(rules: ScheduleRule[], id: string, now: Date): string {
  const own = rules.filter(r => lists(r, id))
  const days = [...DAYS.slice(1), DAYS[0]].filter(day => own.some(r => r.rule_type === 'DayOfWeek' && r.start_day_of_week === day))
  const parts: string[] = []
  if (own.some(r => r.rule_type === 'AlwaysOn')) parts.push('Every day')
  if (days.length) parts.push(`Every ${days.join(' and ')}`)
  for (const r of own) {
    if (r.rule_type === 'SpecialDate') parts.push(dateRange(r.start_month ?? 1, r.start_day ?? 1, r.duration_days ?? 1))
    if (r.rule_type === 'SpecialDateLunar') parts.push('Lunar calendar, date varies')
    if (r.rule_type === 'WeeklyRotation' && r.events.length) {
      const week = rotationWeek(r, now)
      const k = r.events.findIndex((_, n) => r.events[(week + n) % r.events.length] === id)
      parts.push(k === 0 ? 'Weekly rotation, this week' : k === 1 ? 'Weekly rotation, next week' : `Weekly rotation, in ${k} weeks`)
    }
  }
  return parts.join('; ') || 'Not on the calendar'
}
