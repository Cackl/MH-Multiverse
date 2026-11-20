import { describe, it, expect } from 'vitest'
import {
  parsePairs, pairsText, emptyKeys, withParent, pathHint, problems, entryJson, parseEntryJson, valueError,
  RECIPES, PARENTS, CHOICE_FIELDS, LOAD_INT, LOAD_FLOAT,
} from '../../src/lib/patchEntryEdit'
import type { PatchEntry } from '../../src/lib/patchesMeta'

// Above 2^53, so a Number would round it.
const BIG = '17436925256515063008'
const entry = (x: Partial<PatchEntry>): PatchEntry => ({ Enabled: true, Prototype: 'P.prototype', Path: 'F', Description: 'd', ValueType: 'Integer', Value: 1, ...x })

describe('parsePairs', () => {
  it('keeps repeated keys, exact ids and nested arrays as text', () => {
    const text = `{"CooldownModifierPctForPower":[${BIG},-0.5], "CooldownModifierPctForPower" : [42,-0.1],"Op":"LoadInt","Name":"a,}b\\"c"}`
    expect(parsePairs(text)).toEqual([
      { key: 'CooldownModifierPctForPower', text: `[${BIG},-0.5]`, str: false },
      { key: 'CooldownModifierPctForPower', text: '[42,-0.1]', str: false },
      { key: 'Op', text: 'LoadInt', str: true },
      { key: 'Name', text: 'a,}b"c', str: true },
    ])
  })

  it('reads multi-line text and an empty object', () => {
    expect(parsePairs(`{\n  "ParentDataRef": ${BIG},\n  "Value": 8\n}`)).toEqual([
      { key: 'ParentDataRef', text: BIG, str: false },
      { key: 'Value', text: '8', str: false },
    ])
    expect(parsePairs(' {} ')).toEqual([])
    expect(parsePairs('{"a":{"b":1,"c":2},"d":3}')).toEqual([
      { key: 'a', text: '{"b":1,"c":2}', str: false },
      { key: 'd', text: '3', str: false },
    ])
  })

  it('is null for anything but one object', () => {
    for (const bad of ['[1,2]', '{"a":1', '{"a" 1}', '{a:1}', '{"a":1} x', '{"a":1,}x', '']) {
      expect(parsePairs(bad)).toBeNull()
    }
  })

  it('round-trips through pairsText', () => {
    const text = `{"ParentDataRef":${BIG},"Value":8,"Op":"LoadInt","K":[1,2]}`
    expect(pairsText(parsePairs(text)!)).toBe(text)
  })
})

describe('emptyKeys', () => {
  it('lists unfilled values but not empty strings', () => {
    expect(emptyKeys([{ key: 'Value', text: ' ', str: false }, { key: 'Op', text: '', str: true }, { key: 'X', text: '1', str: false }])).toEqual(['Value'])
  })
})

describe('withParent', () => {
  it('swaps the fields and keeps Value', () => {
    const intPairs = parsePairs(`{"ParentDataRef":${LOAD_INT},"Value":8,"Op":"LoadInt"}`)!
    expect(pairsText(withParent(intPairs, LOAD_FLOAT))).toBe(`{"ParentDataRef":${LOAD_FLOAT},"Value":8,"Op":"LoadFloat"}`)
  })

  it('keeps the fields of a parent it does not know', () => {
    const pairs = parsePairs('{"ParentDataRef":1,"Weight":2}')!
    expect(pairsText(withParent(pairs, '99'))).toBe('{"ParentDataRef":99,"Weight":2}')
  })
})

describe('RECIPES', () => {
  it('fills the summon cap with a whole-number formula', () => {
    const r = RECIPES.find(x => x.entry.Path === 'SummonMaxSimultaneous')!
    expect(r.entry).toEqual({ Path: 'SummonMaxSimultaneous', ValueType: 'Prototype', Value: `{"ParentDataRef":${LOAD_INT},"Value":,"Op":"LoadInt"}` })
    expect(emptyKeys(parsePairs(r.entry.Value as string)!)).toEqual(['Value'])
  })

  it('gives every structured recipe a value the editor can read', () => {
    for (const r of RECIPES.filter(x => x.entry.ValueType === 'Prototype' || x.entry.ValueType === 'Properties')) {
      expect(parsePairs(r.entry.Value as string), r.name).not.toBeNull()
    }
  })
})

describe('CHOICE_FIELDS', () => {
  it('lists the 28 power events the server fires, including every default', () => {
    const events = CHOICE_FIELDS.PowerEvent
    expect(events).toHaveLength(28)
    expect(new Set(events).size).toBe(28)
    expect(events).not.toContain('OnPowerPivot')
    expect(events).not.toContain('OnOutOfRangeActivateMovementPower')
    for (const f of PARENTS.flatMap(x => x.fields)) if (CHOICE_FIELDS[f.key]) expect(CHOICE_FIELDS[f.key]).toContain(f.text)
  })
})

describe('pathHint', () => {
  it('explains appends, Properties and whole formulas', () => {
    expect(pathHint(entry({ Path: 'Keywords[]' }))).toMatch(/adds to the list/)
    expect(pathHint(entry({ Path: 'Keywords[0]' }))).toBe('')
    expect(pathHint(entry({ Path: 'ActionsTriggeredOnPowerEvent[]' }))).toMatch(/ComboEffect/)
    expect(pathHint(entry({ Path: 'ActionsTriggeredOnPowerEvent[2]' }))).toMatch(/ComboEffect/)
    expect(pathHint(entry({ Path: 'XActionsTriggeredOnPowerEvent[]' }))).toMatch(/adds to the list/)
    expect(pathHint(entry({ ValueType: 'Properties' }))).toMatch(/first Properties entry/)
    expect(pathHint(entry({ ValueType: 'Prototype', Value: '{"Op":"LoadInt"}' }))).toMatch(/whole formula/)
    expect(pathHint(entry({ ValueType: 'Prototype', Value: '{"Ability":1}' }))).toBe('')
    expect(pathHint(entry({}))).toBe('')
  })
})

describe('problems', () => {
  it('flags PrototypeDataRef on Keywords and fixes it', () => {
    const e = entry({ Path: 'Keywords[]', ValueType: 'PrototypeDataRef', Value: '8001975789840962157' })
    const [p] = problems(e)
    expect(p.level).toBe('error')
    expect(p.apply!(e).ValueType).toBe('PrototypeId')
    expect(problems(entry({ Path: 'X.ApplyStates[0]', ValueType: 'PrototypeDataRef' }))).toHaveLength(1)
    expect(problems(entry({ Path: 'Keywords[]', ValueType: 'PrototypeId' }))).toEqual([])
    expect(problems(entry({ Path: 'MyKeywords[]', ValueType: 'PrototypeDataRef' }))).toEqual([])
  })

  it('warns on AppliesConditions paths', () => {
    expect(problems(entry({ Path: 'AppliesConditions[0].DurationMS' }))[0].level).toBe('warning')
    expect(problems(entry({ Path: 'X.AppliesConditions[1]' }))).toHaveLength(1)
  })

  it('replaces a .Value edit with the whole formula', () => {
    const int = entry({ Path: 'SummonMaxSimultaneous.Value', ValueType: 'Integer', Value: 4 })
    expect(problems(int)[0].apply!(int)).toEqual(entry({
      Path: 'SummonMaxSimultaneous', ValueType: 'Prototype', Value: `{"ParentDataRef":${LOAD_INT},"Value":4,"Op":"LoadInt"}`,
    }))
    const float = entry({ Path: 'CooldownTimeMS.Value', ValueType: 'Float', Value: 1.5 })
    expect(problems(float)[0].apply!(float).Value).toBe(`{"ParentDataRef":${LOAD_FLOAT},"Value":1.5,"Op":"LoadFloat"}`)
    expect(problems(entry({ Path: 'X.Value', ValueType: 'Prototype', Value: '{}' }))).toEqual([])
  })
})

describe('entryJson and parseEntryJson', () => {
  it('round-trips raw values and ids exactly', () => {
    const cases: PatchEntry[] = [
      entry({ ValueType: 'Prototype', Value: `{"ParentDataRef":${BIG},"Value":8,"Op":"LoadInt"}` }),
      entry({ ValueType: 'Properties', Value: `{"CooldownModifierPctForPower":[${BIG},-0.5],"CooldownModifierPctForPower":[42,-0.1]}` }),
      entry({ ValueType: 'PrototypeId', Value: BIG }),
      entry({ ValueType: 'PrototypeDataRef[]', Value: [BIG, '42'] }),
      entry({ ValueType: 'Vector3', Value: [1, 2.5, -3] }),
      entry({ Enabled: false, ValueType: 'Enum', Value: 'ComboEffect', Description: 'says "hi"' }),
    ]
    for (const e of cases) expect(parseEntryJson(entryJson(e))).toEqual({ entry: e })
  })

  it('writes ids bare', () => {
    expect(entryJson(entry({ ValueType: 'PrototypeId', Value: BIG }))).toContain(`"Value": ${BIG}`)
    expect(entryJson(entry({ ValueType: 'PrototypeId[]', Value: [BIG] }))).toContain(`"Value": [${BIG}]`)
  })

  it('explains what is wrong', () => {
    expect(parseEntryJson('[]')).toEqual({ error: 'Not a JSON object.' })
    expect(parseEntryJson('{"Enabled":true}')).toEqual({ error: 'Missing Prototype, Path, Description, ValueType, Value. The server skips the whole file without them.' })
    const ok = entryJson(entry({}))
    expect(parseEntryJson(ok.replace('"Enabled": true', '"Enabled": 1'))).toEqual({ error: 'Enabled must be true or false.' })
    expect(parseEntryJson(ok.replace('"Path": "F"', '"Path": 5'))).toEqual({ error: 'Path must be text in quotes.' })
    expect(parseEntryJson(ok.replace('"Value": 1', '"Value": [1'))).toEqual({ error: 'Not a JSON object.' })
    expect(parseEntryJson(ok.replace('"Value": 1', '"Value": 1,\n  "Extra": 2'))).toEqual({ error: 'Unknown key Extra.' })
  })
})

describe('valueError', () => {
  it('passes values the server reads', () => {
    for (const [type, text] of [
      ['Integer', '-2147483648'], ['Integer', ' 2147483647 '], ['Float', '1.5'], ['Float', '-3.4028235e38'],
      ['PrototypeId', BIG], ['LocaleStringId', '18446744073709551615'], ['PrototypeId[]', `["${BIG}", 5]`],
      ['PrototypeDataRef[]', '[]'], ['Vector3', '[1, 2.5, -3]'], ['String', ''], ['Boolean', ''], ['Prototype', '{"x":1}'],
    ]) expect(valueError(type, text), `${type} ${text}`).toBe('')
  })
  it('refuses values that make the server skip the file', () => {
    for (const [type, text] of [
      ['Integer', ''], ['Integer', '1.5'], ['Integer', '2147483648'], ['Integer', 'abc'],
      ['Float', ''], ['Float', 'abc'], ['Float', '3.5e38'], ['Float', 'Infinity'],
      ['PrototypeId', 'Powers/Foo.prototype'], ['PrototypeGuid', '-1'], ['PrototypeDataRef', '18446744073709551616'],
      ['PrototypeId[]', '[1, "abc"]'], ['PrototypeId[]', '5'], ['PrototypeId[]', '[1,'],
      ['Vector3', '[1, 2]'], ['Vector3', '[1, 2, "3"]'], ['Vector3', '[1, 2, 1e39]'], ['Prototype[]', ''],
    ]) expect(valueError(type, text), `${type} ${text}`).not.toBe('')
  })
})