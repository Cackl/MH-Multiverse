// Helpers for the Advanced patch entry editor: the Start from list, structured Prototype and
// Properties values, the JSON view and checks for mistakes the server rejects or skips.
//
// Prototype, Prototype[] and Properties values travel as the file's own JSON text (see
// patches.rs RAW_TYPES): parsing them in JS would round 64-bit ids and drop repeated
// Properties keys. So these helpers split that text into key/value pairs without ever
// turning a value into a number.

import type { PatchEntry } from './patchesMeta'

export const LOAD_INT = '17436925256515063008'   // Eval/Constants/LoadInt.defaults
export const LOAD_FLOAT = '5074715296837012907'  // Eval/Constants/LoadFloat.defaults
const ABILITY = '10449030269642610036'           // Entity/Characters/Avatars/AbilityAssignment.defaults
const POWER_EVENT = '16732810408560760597'       // Powers/Blueprints/PowerEvents/PowerEventAction.defaults
const SPAWN_ENTRY = '5388673266762714986'        // Entity/SpawnerSequenceEntry.defaults

/** One key of a flat JSON object. `str`: the value is a JSON string, shown and edited without quotes. */
export interface Pair { key: string; text: string; str: boolean }

/** What a `Prototype` value can be based on (its ParentDataRef), with the fields each one starts with. */
export const PARENTS: { id: string; label: string; fields: Pair[] }[] = [
  { id: LOAD_INT, label: 'Whole-number formula (LoadInt)', fields: [p('Value'), p('Op', 'LoadInt')] },
  { id: LOAD_FLOAT, label: 'Decimal formula (LoadFloat)', fields: [p('Value'), p('Op', 'LoadFloat')] },
  { id: ABILITY, label: 'Ability assignment', fields: [p('Ability')] },
  { id: POWER_EVENT, label: 'Power event action', fields: [p('EventAction', 'UsePower'), p('Power'), p('PowerEvent', 'OnPowerStart')] },
  { id: SPAWN_ENTRY, label: 'Spawner sequence entry', fields: [p('ObjectTemplate'), { key: 'Count', text: '1', str: false }] },
]

/** Fields that hold a prototype id, picked by name in the editor. */
export const ID_FIELDS = ['Ability', 'Power', 'ObjectTemplate']
/**
 * Fields picked from a list. PowerEvent leaves out OnPowerPivot and
 * OnOutOfRangeActivateMovementPower, which nothing in the server fires.
 */
export const CHOICE_FIELDS: Record<string, string[]> = {
  PowerEvent: [
    'OnContactTime', 'OnCriticalHit', 'OnHitKeyword', 'OnPowerApply', 'OnPowerEnd', 'OnPowerHit', 'OnPowerStart',
    'OnProjectileHit', 'OnStackCount', 'OnTargetKill', 'OnSummonEntity', 'OnHoldBegin', 'OnMissileHit', 'OnMissileKilled',
    'OnHotspotNegated', 'OnHotspotNegatedByOther', 'OnHotspotOverlapBegin', 'OnHotspotOverlapEnd', 'OnRemoveCondition',
    'OnRemoveNegStatusEffect', 'OnPowerToggleOn', 'OnPowerToggleOff', 'OnPowerStopped', 'OnExtraActivationCooldown',
    'OnPowerLoopEnd', 'OnSpecializationPowerAssigned', 'OnSpecializationPowerUnassigned', 'OnEntityControlled',
  ],
}
/** A note under a picked choice that behaves differently. */
export const CHOICE_NOTES: Record<string, string> = {
  OnHitKeyword: "Fires only when the target has a condition with this action's Keywords.",
  OnStackCount: 'Fires when the stack count equals EventParam.',
  OnTargetKill: 'Only one triggered power per parent power is allowed.',
}
/** Fields the parent fixes; editing them breaks the formula. */
export const FIXED_FIELDS = ['Op']

function p(key: string, str?: string): Pair {
  return str === undefined ? { key, text: '', str: false } : { key, text: str, str: true }
}

export interface Recipe { name: string; hint: string; entry: Omit<PatchEntry, 'Enabled' | 'Prototype' | 'Description'> }

const struct = (parent: string, fields?: Pair[]) => pairsText([{ key: 'ParentDataRef', text: parent, str: false }, ...(fields ?? PARENTS.find(x => x.id === parent)!.fields)])

/** The Start from list for a new entry. Each fills in the path, value type, ParentDataRef and Op. */
export const RECIPES: Recipe[] = [
  { name: 'Set a value', hint: 'Path = value', entry: { Path: '', ValueType: 'Integer', Value: '' } },
  { name: 'Replace a formula with a fixed number', hint: 'LoadInt, LoadFloat', entry: { Path: '', ValueType: 'Prototype', Value: struct(LOAD_INT) } },
  { name: 'Add properties to a hero', hint: 'Properties', entry: { Path: 'Properties', ValueType: 'Properties', Value: pairsText([p('CooldownModifierPctGlobal')]) } },
  { name: 'Give a hero another passive', hint: 'HiddenPassivePowers[]', entry: { Path: 'HiddenPassivePowers[]', ValueType: 'Prototype', Value: struct(ABILITY) } },
  { name: 'Trigger a power from a power', hint: 'ActionsTriggeredOnPowerEvent[]', entry: { Path: 'ActionsTriggeredOnPowerEvent[]', ValueType: 'Prototype', Value: struct(POWER_EVENT) } },
  { name: 'Add a keyword to a power', hint: 'Keywords[]', entry: { Path: 'Keywords[]', ValueType: 'PrototypeId', Value: '' } },
  { name: 'Change a summon power cap', hint: 'SummonMaxSimultaneous', entry: { Path: 'SummonMaxSimultaneous', ValueType: 'Prototype', Value: struct(LOAD_INT) } },
  { name: 'Change what a power summons', hint: 'SummonEntityContexts[0]', entry: { Path: 'SummonEntityContexts[0].SummonEntity', ValueType: 'PrototypeDataRef', Value: '' } },
  { name: 'Add a spawn to a spawner', hint: 'SpawnSequence[]', entry: { Path: 'SpawnSequence[]', ValueType: 'Prototype', Value: struct(SPAWN_ENTRY) } },
]

// ── Pairs ────────────────────────────────────────────────────────────────────

/**
 * The keys of a flat JSON object in order, repeats kept, each value as its own text.
 * Null when the text isn't an object (an array, or broken JSON).
 */
export function parsePairs(text: string): Pair[] | null {
  let i = 0
  const ws = () => { while (/\s/.test(text[i] ?? '')) i++ }
  const readString = (): string | null => {
    const start = i
    if (text[i++] !== '"') return null
    while (i < text.length && text[i] !== '"') i += text[i] === '\\' ? 2 : 1
    if (text[i++] !== '"') return null
    try { return JSON.parse(text.slice(start, i)) } catch { return null }
  }
  ws()
  if (text[i++] !== '{') return null
  const out: Pair[] = []
  ws()
  if (text[i] === '}') { i++; ws(); return i === text.length ? out : null }
  for (;;) {
    ws()
    const key = readString()
    if (key === null) return null
    ws()
    if (text[i++] !== ':') return null
    ws()
    // The value runs to the next comma or closing brace outside strings and brackets.
    const start = i
    let depth = 0
    while (i < text.length) {
      const c = text[i]
      if (c === '"') { if (readString() === null) return null; continue }
      if (c === '[' || c === '{') depth++
      else if (c === ']' || c === '}') { if (depth === 0) break; depth-- }
      else if (c === ',' && depth === 0) break
      i++
    }
    const raw = text.slice(start, i).trim()
    const str = raw.startsWith('"')
    out.push({ key, text: str ? JSON.parse(raw) : raw, str })
    ws()
    if (text[i] === ',') { i++; continue }
    if (text[i++] !== '}') return null
    ws()
    return i === text.length ? out : null
  }
}

/** The pairs as one-line JSON text. An empty non-string value is left empty, so the JSON is invalid until it's filled. */
export function pairsText(pairs: Pair[]): string {
  return `{${pairs.map(x => `${JSON.stringify(x.key)}:${x.str ? JSON.stringify(x.text) : x.text.trim()}`).join(',')}}`
}

/** Keys still waiting for a value. */
export function emptyKeys(pairs: Pair[]): string[] {
  return pairs.filter(x => !x.str && !x.text.trim()).map(x => x.key)
}

/** The Prototype value with its ParentDataRef swapped, keeping Value when both parents have one. */
export function withParent(pairs: Pair[], parent: string): Pair[] {
  const fields = PARENTS.find(x => x.id === parent)?.fields ?? pairs.filter(x => x.key !== 'ParentDataRef')
  const value = pairs.find(x => x.key === 'Value')
  return [
    { key: 'ParentDataRef', text: parent, str: false },
    ...fields.map(f => f.key === 'Value' && value ? value : { ...f }),
  ]
}

// ── Hints and checks ─────────────────────────────────────────────────────────

/** A note under Path for paths that behave differently from a plain field. */
export function pathHint(e: Pick<PatchEntry, 'Path' | 'ValueType' | 'Value'>): string {
  if (/(^|\.)ActionsTriggeredOnPowerEvent\[/.test(e.Path.trim())) return "The power it triggers must be a ComboEffect power that isn't passive and isn't already on the hero. The server refuses anything else."
  if (e.Path.trim().endsWith('[]')) return 'Ends in [], so this adds to the list. [n] would replace element n.'
  if (e.ValueType === 'Properties') return 'Only the first Properties entry for a hero counts, across all files.'
  if (e.ValueType === 'Prototype' && typeof e.Value === 'string' && /"Op"\s*:/.test(e.Value)) return 'Replaces the whole formula, so powers that inherit it keep theirs.'
  return ''
}

export interface Problem {
  level: 'error' | 'warning'
  message: string
  /** Label of the one-click fix, when there is one. */
  fix?: string
  apply?: (e: PatchEntry) => PatchEntry
}

/** Mistakes the server rejects (error) or skips without a message (warning). */
export function problems(e: PatchEntry): Problem[] {
  const out: Problem[] = []
  const path = e.Path.trim()
  const list = path.match(/(?:^|\.)(Keywords|ApplyStates)\[/)?.[1]
  if (list && e.ValueType === 'PrototypeDataRef') {
    out.push({
      level: 'error',
      message: `${list} holds plain ids, so the server rejects PrototypeDataRef here and logs Failed UpdateValue.`,
      fix: 'Use PrototypeId',
      apply: x => ({ ...x, ValueType: 'PrototypeId' }),
    })
  }
  if (/(?:^|\.)AppliesConditions\[/.test(path)) {
    out.push({ level: 'warning', message: 'AppliesConditions is a list mixin, so this path never matches. The server skips it without a message.' })
  }
  if (/\.Value$/.test(path) && (e.ValueType === 'Integer' || e.ValueType === 'Float')) {
    out.push({
      level: 'warning',
      message: 'If this formula is inherited, editing .Value also changes its parent and every sibling. Replacing the whole formula is safer.',
      fix: 'Replace the whole formula',
      apply: x => {
        const whole = x.ValueType === 'Integer'
        const value = String(x.Value ?? '').trim()
        return {
          ...x,
          Path: x.Path.trim().slice(0, -'.Value'.length),
          ValueType: 'Prototype',
          Value: pairsText([
            { key: 'ParentDataRef', text: whole ? LOAD_INT : LOAD_FLOAT, str: false },
            { key: 'Value', text: value, str: false },
            { key: 'Op', text: whole ? 'LoadInt' : 'LoadFloat', str: true },
          ]),
        }
      },
    })
  }
  return out
}

// ── JSON view ────────────────────────────────────────────────────────────────

const RAW = new Set(['Prototype', 'Prototype[]', 'Properties'])
const ID = new Set(['PrototypeId', 'PrototypeDataRef', 'PrototypeGuid', 'LocaleStringId'])
const ID_LIST = new Set(['PrototypeId[]', 'PrototypeDataRef[]'])

/** The Value as it is written to the file: raw text kept, ids bare. */
function valueText(e: PatchEntry): string {
  const v = e.Value
  if (RAW.has(e.ValueType) && typeof v === 'string') return v.trim()
  if (ID.has(e.ValueType) && typeof v === 'string' && /^\d+$/.test(v.trim())) return v.trim()
  if (ID_LIST.has(e.ValueType) && Array.isArray(v)) return `[${v.map(x => /^\d+$/.test(String(x)) ? String(x) : JSON.stringify(x)).join(',')}]`
  return JSON.stringify(v ?? null)
}

/** The whole entry as the JSON the file holds for it. */
export function entryJson(e: PatchEntry): string {
  return [
    '{',
    `  "Enabled": ${e.Enabled},`,
    `  "Prototype": ${JSON.stringify(e.Prototype)},`,
    `  "Path": ${JSON.stringify(e.Path)},`,
    `  "Description": ${JSON.stringify(e.Description)},`,
    `  "ValueType": ${JSON.stringify(e.ValueType)},`,
    `  "Value": ${valueText(e)}`,
    '}',
  ].join('\n')
}

const KEYS = ['Enabled', 'Prototype', 'Path', 'Description', 'ValueType', 'Value']

/** The entry from JSON text, or an error message. Ids and raw values stay text. */
export function parseEntryJson(text: string): { entry: PatchEntry } | { error: string } {
  const pairs = parsePairs(text)
  if (!pairs) return { error: 'Not a JSON object.' }
  const get = (k: string) => pairs.find(x => x.key === k)
  const missing = KEYS.filter(k => !get(k))
  if (missing.length) return { error: `Missing ${missing.join(', ')}. The server skips the whole file without them.` }
  const extra = pairs.filter(x => !KEYS.includes(x.key)).map(x => x.key)
  if (extra.length) return { error: `Unknown key ${extra[0]}.` }
  const enabled = get('Enabled')!.text
  if (enabled !== 'true' && enabled !== 'false') return { error: 'Enabled must be true or false.' }
  for (const k of ['Prototype', 'Path', 'Description', 'ValueType']) {
    if (!get(k)!.str) return { error: `${k} must be text in quotes.` }
  }
  const type = get('ValueType')!.text
  const v = get('Value')!
  const raw = v.str ? JSON.stringify(v.text) : v.text
  let value: unknown
  if (RAW.has(type)) value = raw
  else if (ID.has(type)) value = v.text
  else if (ID_LIST.has(type)) value = raw.replace(/^\[|\]$/g, '').split(',').map(x => x.trim().replace(/^"|"$/g, '')).filter(Boolean)
  else {
    try { value = JSON.parse(raw) } catch { return { error: 'Value is not valid JSON.' } }
  }
  return {
    entry: {
      Enabled: enabled === 'true',
      Prototype: get('Prototype')!.text,
      Path: get('Path')!.text,
      Description: get('Description')!.text,
      ValueType: type,
      Value: value,
    },
  }
}
