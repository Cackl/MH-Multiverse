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
