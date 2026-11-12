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
