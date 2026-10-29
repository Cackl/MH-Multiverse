import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'

// WCAG 2.x contrast gate for the theme tokens in src/app.css.
// Text needs 4.5:1; borders that identify a control need 3:1 (WCAG 1.4.11).

type Rgba = { r: number; g: number; b: number; a: number }

const css = readFileSync(new URL('../../src/app.css', import.meta.url), 'utf8').replace(/\r/g, '')

function parseBlock(body: string): Record<string, string> {
  const out: Record<string, string> = {}
  for (const m of body.matchAll(/^\s*(--[\w-]+)\s*:\s*([^;]+);/gm)) out[m[1]] = m[2].trim()
  return out
}

let rootTokens: Record<string, string> = {}
const themeOverrides: Record<string, Record<string, string>> = {}
for (const m of css.matchAll(/^(:root|\[data-theme="([^"]+)"\])\s*\{([\s\S]*?)^\}/gm)) {
  if (m[1] === ':root') rootTokens = parseBlock(m[3])
  else themeOverrides[m[2]] = parseBlock(m[3])
}
const themes: Record<string, Record<string, string>> = { default: rootTokens }
for (const id of Object.keys(themeOverrides)) themes[id] = { ...rootTokens, ...themeOverrides[id] }

function resolve(tokens: Record<string, string>, value: string, depth = 0): string {
  const m = value.match(/^var\((--[\w-]+)\)$/)
  if (!m) return value
  if (depth > 10 || !(m[1] in tokens)) throw new Error(`cannot resolve ${value}`)
  return resolve(tokens, tokens[m[1]], depth + 1)
}

function parseColour(s: string): Rgba {
  const hex = s.match(/^#([0-9a-f]{3}|[0-9a-f]{6})$/i)
  if (hex) {
    const h = hex[1].length === 3 ? hex[1].replace(/./g, (c) => c + c) : hex[1]
    return { r: parseInt(h.slice(0, 2), 16), g: parseInt(h.slice(2, 4), 16), b: parseInt(h.slice(4, 6), 16), a: 1 }
  }
  const rgb = s.match(/^rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*(?:,\s*([\d.]+)\s*)?\)$/)
  if (rgb) return { r: +rgb[1], g: +rgb[2], b: +rgb[3], a: rgb[4] === undefined ? 1 : +rgb[4] }
  throw new Error(`unsupported colour: ${s}`)
}

function over(fg: Rgba, bg: Rgba): Rgba {
  const a = fg.a
  return { r: fg.r * a + bg.r * (1 - a), g: fg.g * a + bg.g * (1 - a), b: fg.b * a + bg.b * (1 - a), a: 1 }
}

function luminance({ r, g, b }: Rgba): number {
  const lin = (v: number) => {
    const c = v / 255
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4
  }
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

export function contrast(fg: Rgba, bg: Rgba): number {
  const [hi, lo] = [luminance(over(fg, bg)), luminance(bg)].sort((x, y) => y - x)
  return (hi + 0.05) / (lo + 0.05)
}

type Pair = { fg: string; bg: string; min: number }

const TEXT = 4.5
const EDGE = 3
const pairs: Pair[] = []
const add = (fgs: string[], bgs: string[], min: number) =>
  fgs.forEach((fg) => bgs.forEach((bg) => pairs.push({ fg, bg, min })))

add(['--text-0', '--text-1', '--text-2', '--text-3'], ['--bg-0', '--bg-1', '--bg-2', '--bg-3'], TEXT)
add(
  ['--accent', '--accent-bright', '--text-error', '--text-success', '--text-warning', '--amber', '--amber-bright', '--green-bright'],
  ['--bg-1', '--bg-2'],
  TEXT,
)
// .btn-red: error text on the translucent --red-dim tint ("tint@base" composites the tint over the base)
add(['--text-error'], ['--red-dim@--bg-1', '--red-dim@--bg-2'], TEXT)
add(['--border-lit'], ['--bg-1'], EDGE)
add(['--border-mid'], ['--bg-0'], EDGE)

describe('contrast.test: sanity of the maths', () => {
  it('white on black is 21:1 and a colour on itself is 1:1', () => {
    const white = parseColour('#fff')
    const black = parseColour('#000')
    expect(contrast(white, black)).toBeCloseTo(21, 5)
    expect(contrast(white, white)).toBeCloseTo(1, 5)
  })

  it('composites a translucent foreground over its background first', () => {
    // 50% white over black is mid-grey #808080, 5.32:1 against black
    expect(contrast(parseColour('rgba(255,255,255,0.5)'), parseColour('#000'))).toBeCloseTo(5.32, 1)
  })
})

describe('dimmed rows (off / disabled / banned items that still show readable text)', () => {
  // Names in these rows must stay legible: primary text at the full 4.5, secondary names at 3:1.
  const opacity = parseFloat(rootTokens['--dimmed-opacity'])

  it('defines --dimmed-opacity as a plain number', () => {
    expect(opacity).toBeGreaterThan(0)
    expect(opacity).toBeLessThanOrEqual(1)
  })

  for (const [id, tokens] of Object.entries(themes)) {
    it(`${id}: dimmed text-0 and text-1 stay legible`, () => {
      const colourOf = (name: string) => parseColour(resolve(tokens, tokens[name]))
      const failures: string[] = []
      for (const bgName of ['--bg-1', '--bg-2']) {
        const bg = colourOf(bgName)
        for (const [fg, min] of [['--text-0', TEXT], ['--text-1', 3]] as const) {
          const dimmed = over({ ...colourOf(fg), a: opacity }, bg)
          const ratio = contrast(dimmed, bg)
          if (ratio < min) failures.push(`${fg} at ${opacity} on ${bgName}: ${ratio.toFixed(2)} < ${min}`)
        }
      }
      expect(failures).toEqual([])
    })
  }
})

describe('theme contrast', () => {
  it('finds every shipped theme block', () => {
    expect(Object.keys(themes).sort()).toEqual(
      ['blue-grey', 'default', 'grey', 'light', 'mh-itembase', 'phoenix', 'tahiti'].sort(),
    )
  })

  for (const [id, tokens] of Object.entries(themes)) {
    it(`${id}: every pair meets its target`, () => {
      const failures: string[] = []
      for (const { fg, bg, min } of pairs) {
        if (!(fg in tokens)) {
          failures.push(`${fg} is not defined`)
          continue
        }
        const colourOf = (name: string) => parseColour(resolve(tokens, tokens[name]))
        const [tint, base] = bg.split('@')
        const bgColour = base ? over(colourOf(tint), colourOf(base)) : colourOf(bg)
        const ratio = contrast(colourOf(fg), bgColour)
        if (ratio < min) failures.push(`${fg} on ${bg}: ${ratio.toFixed(2)} < ${min}`)
      }
      expect(failures).toEqual([])
    })
  }
})
