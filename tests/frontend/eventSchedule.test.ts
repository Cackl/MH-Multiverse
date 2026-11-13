import { describe, it, expect } from 'vitest'
import { ruleIsValid, ruleActiveIds, activeEventIds, nextInRotation, lunarToGregorian } from '../../src/lib/eventSchedule'
import type { ScheduleRule } from '../../src/lib/tuningMeta'

/** A server clock time; the module reads it with getUTC* methods. */
const at = (iso: string) => new Date(`${iso}Z`)

const rule = (r: Partial<ScheduleRule>): ScheduleRule => ({
  name: 'R', is_enabled: true, rule_type: 'AlwaysOn', start_day_of_week: null,
  start_month: null, start_day: null, duration_days: null, events: ['A'], ...r,
})

// Stock v52 rotations (EventSchedule.json).
const core = rule({ rule_type: 'WeeklyRotation', start_day_of_week: 'Friday',
  events: ['CosmicChaos', 'OdinsBounty', 'OperationOmega', 'MysticMayhem', 'ArmorIncursion'] })
const secondary = rule({ rule_type: 'WeeklyRotation', start_day_of_week: 'Friday',
  events: ['HeroesForHire', 'WealthOfKingsCounty', 'OrdersFromTheHand'] })
const winter = rule({ rule_type: 'SpecialDate', start_month: 12, start_day: 20, duration_days: 14, events: ['WinterHoliday'] })
const lunar = rule({ rule_type: 'SpecialDateLunar', start_month: 1, start_day: 1, duration_days: 7, events: ['LunarNewYear'] })

describe('ruleIsValid', () => {
  const known = new Set(['A', 'B'])
  it.each([
    ['AlwaysOn with known ids',            rule({ events: ['A', 'B'] }),                                  true],
    ['blank slots are allowed',            rule({ events: ['A', '', ' '] }),                              true],
    ['disabled',                           rule({ is_enabled: false }),                                   false],
    ['no events',                          rule({ events: [] }),                                          false],
    ['one unknown id drops the whole rule', rule({ events: ['A', 'Gone'] }),                              false],
    ['DayOfWeek without a day',            rule({ rule_type: 'DayOfWeek' }),                              false],
    ['WeeklyRotation with a day',          rule({ rule_type: 'WeeklyRotation', start_day_of_week: 'Friday' }), true],
    ['SpecialDate without a duration',     rule({ rule_type: 'SpecialDate', start_month: 1, start_day: 1 }), false],
    ['unknown rule type',                  rule({ rule_type: 'Monthly' }),                                false],
  ])('%s', (_, r, expected) => {
    expect(ruleIsValid(r, known)).toBe(expected)
  })
})

describe('ruleActiveIds', () => {
  it.each([
    ['AlwaysOn skips blank slots',              rule({ events: ['A', '', 'B'] }),                                      '2026-11-09T12:00:00', ['A', 'B']],
    ['DayOfWeek on its day',                    rule({ rule_type: 'DayOfWeek', start_day_of_week: 'Saturday' }),      '2026-11-14T00:00:00', ['A']],
    ['DayOfWeek the day before',                rule({ rule_type: 'DayOfWeek', start_day_of_week: 'Saturday' }),      '2026-11-13T23:59:59', []],
    ['SpecialDate before its start',            winter,                                                                '2026-12-19T23:59:59', []],
    ['SpecialDate on its start',                winter,                                                                '2026-12-20T00:00:00', ['WinterHoliday']],
    ['SpecialDate on 31 Dec',                   winter,                                                                '2026-12-31T23:59:59', ['WinterHoliday']],
    ['SpecialDate ends at the year cut-off',    winter,                                                                '2027-01-02T12:00:00', []],
    ['Lunar starts a day before the lunar date', lunar,                                                                '2027-02-05T00:00:00', ['LunarNewYear']],
    ['Lunar not yet',                           lunar,                                                                 '2027-02-04T23:59:59', []],
    ['Lunar last day',                          lunar,                                                                 '2027-02-11T23:59:59', ['LunarNewYear']],
    ['Lunar over',                              lunar,                                                                 '2027-02-12T00:00:00', []],
    ['Lunar in a month the table lacks',        rule({ ...lunar, start_month: 8 }),                                    '2027-09-01T00:00:00', []],
    // The research doc's worked example: server day 2026-11-09 and Friday 2026-11-13.
    ['core rotation, Monday',                   core,                                                                  '2026-11-09T12:00:00', ['CosmicChaos']],
    ['secondary rotation, Monday',              secondary,                                                             '2026-11-09T12:00:00', ['OrdersFromTheHand']],
    ['core rotation, Thursday night',           core,                                                                  '2026-11-12T23:59:59', ['CosmicChaos']],
    ['core rotation flips at Friday midnight',  core,                                                                  '2026-11-13T00:00:00', ['OdinsBounty']],
    ['secondary rotation, Friday',              secondary,                                                             '2026-11-13T00:00:00', ['HeroesForHire']],
    ['a blank rotation slot runs nothing',      rule({ ...core, events: ['CosmicChaos', ''] }),                        '2026-11-13T00:00:00', []],
  ])('%s', (_, r, now, expected) => {
    expect(ruleActiveIds(r, at(now))).toEqual(expected)
  })
})

describe('activeEventIds', () => {
  it('leaves out rules the server drops', () => {
    const rules = [rule({ events: ['A'] }), rule({ events: ['B', 'Gone'] }), core]
    const known = new Set(['A', 'B', ...core.events])
    expect([...activeEventIds(rules, known, at('2026-11-09T12:00:00'))]).toEqual(['A', 'CosmicChaos'])
  })
})

describe('nextInRotation', () => {
  it.each([
    ['the next slot',               core,                                                                   { id: 'OdinsBounty', weeks: 1 }],
    ['skips blank slots',           rule({ ...core, events: ['CosmicChaos', '', 'OperationOmega', '', ''] }), { id: 'OperationOmega', weeks: 2 }],
    ['wraps round to the start',    rule({ ...core, events: ['CosmicChaos', '', '', '', ''] }),              { id: 'CosmicChaos', weeks: 5 }],
    ['all blank',                   rule({ ...core, events: ['', ''] }),                                     null],
  ])('%s', (_, r, expected) => {
    expect(nextInRotation(r, at('2026-11-09T12:00:00'))).toEqual(expected)
  })
})

describe('lunarToGregorian', () => {
  it.each([
    // .NET's dates; ICU says 7 Feb 2027 and 2 Feb 2030.
    [2027, 1, 1,  '2027-02-06'],
    [2030, 1, 1,  '2030-02-03'],
    [2026, 1, 15, '2026-03-03'],
    [2100, 1, 1,  '2100-02-09'],
    [2101, 1, 1,  null],
    [2026, 2, 1,  null],
    [2026, 1, 30, null],
  ])('%i month %i day %i', (y, m, d, expected) => {
    expect(lunarToGregorian(y, m, d)?.toISOString().slice(0, 10) ?? null).toBe(expected)
  })
})

// ── Simple view ──────────────────────────────────────────────────────────────────

import { eventMode, setEventMode, scheduleText, ALWAYS_RULE } from '../../src/lib/eventSchedule'

const permanent = rule({ name: 'Permanent Events', events: ['DailyBugle'] })
const midtown = rule({ name: 'Midtown Madness', rule_type: 'DayOfWeek', start_day_of_week: 'Monday', events: ['MidtownMadness'] })
const stock = [
  permanent, midtown,
  { ...core, name: 'Core Weekly Rotation' }, { ...secondary, name: 'Secondary Weekly Rotation' },
  { ...winter, name: 'Winter Holiday' }, { ...lunar, name: 'Lunar New Year' },
]

describe('eventMode', () => {
  it.each([
    ['in an AlwaysOn stock rule',   'DailyBugle',     'schedule'],
    ['in a rotation',               'CosmicChaos',    'schedule'],
    ['in no rule',                  'Cowpocalypse',   'off'],
  ])('%s', (_, id, expected) => {
    expect(eventMode(stock, id)).toBe(expected)
  })
})

describe('setEventMode', () => {
  const byName = (rules: ScheduleRule[], name: string) => rules.find(r => r.name === name)

  it('Off switches off a stock rule that only runs this event and keeps its contents', () => {
    const out = setEventMode(stock, stock, 'MidtownMadness', 'off')
    expect(byName(out, 'Midtown Madness')).toEqual({ ...midtown, is_enabled: false })
    expect(eventMode(out, 'MidtownMadness')).toBe('off')
  })

  it('Off blanks a rotation slot so the other weeks keep their place', () => {
    const out = setEventMode(stock, stock, 'OdinsBounty', 'off')
    expect(byName(out, 'Core Weekly Rotation')!.events).toEqual(['CosmicChaos', '', 'OperationOmega', 'MysticMayhem', 'ArmorIncursion'])
    expect(eventMode(out, 'OdinsBounty')).toBe('off')
  })

  it.each(['MidtownMadness', 'OdinsBounty', 'DailyBugle', 'WinterHoliday', 'Cowpocalypse'])(
    'Off then On schedule gives back the stock schedule for %s', id => {
      expect(setEventMode(setEventMode(stock, stock, id, 'off'), stock, id, 'schedule')).toEqual(stock)
    })

  it('Always on adds Simple\'s own rule and keeps the stock date rule', () => {
    const out = setEventMode(stock, stock, 'WinterHoliday', 'always')
    expect(out.slice(0, stock.length)).toEqual(stock)
    expect(out[stock.length]).toMatchObject({ name: ALWAYS_RULE, is_enabled: true, rule_type: 'AlwaysOn', events: ['WinterHoliday'] })
    expect(eventMode(out, 'WinterHoliday')).toBe('always')
  })

  it('Always on events share one rule, and the rule goes when the last one leaves', () => {
    let out = setEventMode(stock, stock, 'Cowpocalypse', 'always')
    out = setEventMode(out, stock, 'GuardiansOfTheGalaxy', 'always')
    expect(byName(out, ALWAYS_RULE)!.events).toEqual(['Cowpocalypse', 'GuardiansOfTheGalaxy'])
    out = setEventMode(out, stock, 'Cowpocalypse', 'off')
    expect(byName(out, ALWAYS_RULE)!.events).toEqual(['GuardiansOfTheGalaxy'])
    expect(setEventMode(out, stock, 'GuardiansOfTheGalaxy', 'schedule')).toEqual(stock)
  })

  it('a switched-off Always on rule counts as off, and Always on switches it back on', () => {
    const disabled = setEventMode(stock, stock, 'Cowpocalypse', 'always').map(r => r.name === ALWAYS_RULE ? { ...r, is_enabled: false } : r)
    expect(eventMode(disabled, 'Cowpocalypse')).toBe('off')
    const out = setEventMode(disabled, stock, 'GuardiansOfTheGalaxy', 'always')
    expect(eventMode(out, 'GuardiansOfTheGalaxy')).toBe('always')
  })

  it('On schedule leaves a stock rule removed in Advanced removed', () => {
    const edited = stock.filter(r => r.name !== 'Midtown Madness')
    expect(setEventMode(edited, stock, 'MidtownMadness', 'schedule')).toEqual(edited)
  })
})

describe('scheduleText', () => {
  const now = at('2026-11-09T12:00:00')
  const dated = (m: number, d: number, n: number) => [rule({ rule_type: 'SpecialDate', start_month: m, start_day: d, duration_days: n })]
  it.each([
    ['every day',                 stock,                                                       'DailyBugle',     'Every day'],
    ['one weekday',               stock,                                                       'MidtownMadness', 'Every Monday'],
    ['a weekend',                 [rule({ rule_type: 'DayOfWeek', start_day_of_week: 'Sunday' }), rule({ rule_type: 'DayOfWeek', start_day_of_week: 'Saturday' })], 'A', 'Every Saturday and Sunday'],
    ['a date range',              dated(2, 13, 7),                                             'A',              '13 to 19 Feb'],
    ['one day',                   dated(4, 1, 1),                                              'A',              '1 Apr'],
    ['across months',             dated(10, 26, 7),                                            'A',              '26 Oct to 1 Nov'],
    ['cut off at 31 Dec',         stock,                                                       'WinterHoliday',  '20 to 31 Dec'],
    ['lunar',                     stock,                                                       'LunarNewYear',   'Lunar calendar, date varies'],
    ['rotation, this week',       stock,                                                       'CosmicChaos',    'Weekly rotation, this week'],
    ['rotation, next week',       stock,                                                       'OdinsBounty',    'Weekly rotation, next week'],
    ['rotation, later',           stock,                                                       'OperationOmega', 'Weekly rotation, in 2 weeks'],
    ['not on the calendar',       stock,                                                       'Cowpocalypse',   'Not on the calendar'],
    ['always on and a date',      setEventMode(stock, stock, 'WinterHoliday', 'always'),       'WinterHoliday',  'Every day; 20 to 31 Dec'],
  ])('%s', (_, rules, id, expected) => {
    expect(scheduleText(rules, id, now)).toBe(expected)
  })
})

// ── Custom events ────────────────────────────────────────────────────────────────

import {
  eventNameError, slugFor, yearlyMaxDays, putCustomEvent, readCustomEvent, deleteCustomEvent, setCustomEnabled,
  type CustomEvent,
} from '../../src/lib/eventSchedule'
import type { EventDefinition } from '../../src/lib/tuningMeta'

const ID = 'zzMultiverse_DoubleXPWeekend'
const weekend: CustomEvent = {
  name: 'Double XP Weekend', when: { kind: 'weekly', days: ['Sunday', 'Saturday'] },
  values: { eGTV_XPGain: 2, eGTV_LootDropRate: 1 }, gift: '', announce: true,
}
const stockDef: EventDefinition = { id: 'DailyBugle', display_name: 'Daily Bugle', file_path: 'x', daily_gift: null, instanced_missions: null, is_hidden: true }

describe('eventNameError', () => {
  it.each([
    ['Double XP Weekend', ''],
    ["D'Arcy's Day - 2", ''],
    ['Fête', ''],
    ['XP', 'Use 3 to 40 characters.'],
    ['x'.repeat(41), 'Use 3 to 40 characters.'],
    ['../evil', 'Use letters, digits, spaces, apostrophes and hyphens.'],
    ['Loot!', 'Use letters, digits, spaces, apostrophes and hyphens.'],
  ])('%s', (name, expected) => {
    expect(eventNameError(name)).toBe(expected)
  })
})

describe('slugFor', () => {
  it.each([
    ['Double XP Weekend', [],              'DoubleXPWeekend'],
    ['double xp weekend', [ID],            'DoubleXpWeekend2'],
    ['Double XP Weekend', [ID, `${ID}2`],  'DoubleXPWeekend3'],
    ["D'Arcy's Day",      [],              'DArcysDay'],
    ['Fête de la Loot',   [],              'FeteDeLaLoot'],
    ['日本',      [],              'Event'],
  ])('%s', (name, taken, expected) => {
    expect(slugFor(name, new Set(taken))).toBe(expected)
  })
})

describe('yearlyMaxDays', () => {
  it.each([[12, 20, 12], [12, 31, 1], [1, 1, 365], [2, 28, 307]])('%i/%i', (m, d, expected) => {
    expect(yearlyMaxDays(m, d)).toBe(expected)
  })
})

describe('putCustomEvent', () => {
  it('adds the definition, one rule per day after every other rule, and only sliders off 1', () => {
    const out = putCustomEvent([stockDef], stock, ID, weekend, [])
    expect(out.definitions).toEqual([stockDef, {
      id: ID, display_name: 'Double XP Weekend', file_path: 'Events/Multiverse/LiveTuningDataDoubleXPWeekend.json',
      daily_gift: null, instanced_missions: null, is_hidden: null,
    }])
    expect(out.rules.slice(0, stock.length)).toEqual(stock)
    expect(out.rules.slice(stock.length).map(r => [r.name, r.rule_type, r.start_day_of_week, r.events, r.is_enabled])).toEqual([
      ['Multiverse: Double XP Weekend (Sat)', 'DayOfWeek', 'Saturday', [ID], true],
      ['Multiverse: Double XP Weekend (Sun)', 'DayOfWeek', 'Sunday', [ID], true],
    ])
    expect(out.entries).toEqual([{ prototype: '', setting: 'eGTV_XPGain', value: 2 }])
  })

  it.each([
    ['every day',  { kind: 'daily' } as const,                             [['Multiverse: Party', 'AlwaysOn', null, null]]],
    ['one day',    { kind: 'weekly', days: ['Friday'] } as const,          [['Multiverse: Party', 'DayOfWeek', 'Friday', null]]],
    ['every year', { kind: 'yearly', month: 7, day: 4, days: 3 } as const, [['Multiverse: Party', 'SpecialDate', null, 3]]],
  ])('rules for %s', (_, when, expected) => {
    const out = putCustomEvent([], [], 'zzMultiverse_Party', { ...weekend, name: 'Party', when }, [])
    expect(out.rules.map(r => [r.name, r.rule_type, r.start_day_of_week, r.duration_days])).toEqual(expected)
  })

  it('a gift and announce off go into the definition', () => {
    const def = putCustomEvent([], [], ID, { ...weekend, gift: ' Entity/Items/Gift.prototype ', announce: false }, []).definitions[0]
    expect([def.daily_gift, def.is_hidden]).toEqual(['Entity/Items/Gift.prototype', true])
  })

  it('editing replaces its own rules, keeps it off, and keeps other rules and entries', () => {
    const first = putCustomEvent([], stock, ID, weekend, [])
    const off = setCustomEnabled([...first.rules, { ...permanent, name: ALWAYS_RULE, events: [ID] }], ID, false)
    const extra = { prototype: 'Entity/Characters/Mobs/Doom.prototype', setting: 'eWETV_MobHealth', value: 2 }
    const out = putCustomEvent(first.definitions, off, ID, { ...weekend, when: { kind: 'daily' }, values: { eGTV_LootRarity: 3 } },
      [...first.entries, extra])
    expect(out.rules.filter(r => r.events.includes(ID)).map(r => [r.name, r.is_enabled])).toEqual([
      [ALWAYS_RULE, true], ['Multiverse: Double XP Weekend', false],
    ])
    expect(out.entries).toEqual([extra, { prototype: '', setting: 'eGTV_LootRarity', value: 3 }])
    expect(out.definitions).toHaveLength(1)
  })
})

describe('readCustomEvent', () => {
  const sortDays = (ev: CustomEvent) => ev.when.kind === 'weekly' ? { ...ev, when: { ...ev.when, days: [...ev.when.days].sort() } } : ev
  it.each([
    ['weekly', weekend],
    ['daily', { ...weekend, when: { kind: 'daily' } }],
    ['yearly', { ...weekend, when: { kind: 'yearly', month: 12, day: 20, days: 12 }, gift: 'G', announce: false }],
  ] as [string, CustomEvent][])('reads back a %s event', (_, ev) => {
    const out = putCustomEvent([], stock, ID, ev, [])
    const read = readCustomEvent(out.definitions[0], out.rules, out.entries)!
    expect(sortDays(read)).toEqual(sortDays({ ...ev, values: { eGTV_XPGain: 2 } }))
  })

  it.each([
    ['a rule that also runs another event',   (r: ScheduleRule) => ({ ...r, events: [ID, 'DailyBugle'] })],
    ['a rule renamed in Advanced',            (r: ScheduleRule) => ({ ...r, name: 'My weekend' })],
    ['a weekday rule turned into a rotation', (r: ScheduleRule) => ({ ...r, rule_type: 'WeeklyRotation' })],
  ])('gives null for %s', (_, edit) => {
    const out = putCustomEvent([], [], ID, weekend, [])
    expect(readCustomEvent(out.definitions[0], [edit(out.rules[0]), out.rules[1]], out.entries)).toBeNull()
  })
})

describe('deleteCustomEvent', () => {
  it('removes the definition, its own rules and its place in the Always on rule', () => {
    const added = putCustomEvent([stockDef], stock, ID, weekend, [])
    const always = setEventMode(added.rules, stock, 'Cowpocalypse', 'always')
    const withUs = always.map(r => r.name === ALWAYS_RULE ? { ...r, events: [...r.events, ID] } : r)
    const out = deleteCustomEvent(added.definitions, withUs, ID)
    expect(out.definitions).toEqual([stockDef])
    expect(out.rules).toEqual(setEventMode(stock, stock, 'Cowpocalypse', 'always'))
  })
})

describe('setCustomEnabled', () => {
  it('switches only its own rules', () => {
    const added = putCustomEvent([], stock, ID, weekend, []).rules
    const off = setCustomEnabled(added, ID, false)
    expect(off.slice(0, stock.length)).toEqual(stock)
    expect(eventMode(off, ID)).toBe('off')
    expect(setCustomEnabled(off, ID, true)).toEqual(added)
  })
})
