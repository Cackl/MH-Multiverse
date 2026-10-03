// Minimal Markdown → blocks for GitHub release notes. Rendered as plain Svelte
// elements (never {@html}), so release text can't inject markup into the webview.

export interface Span { text: string; bold?: boolean; code?: boolean }

export type Block =
  | { kind: 'h'; level: number; spans: Span[] }
  | { kind: 'li'; depth: number; spans: Span[] }
  | { kind: 'p'; spans: Span[] }
  | { kind: 'hr' }

function spans(line: string): Span[] {
  return line
    .split(/(\*\*[^*]+\*\*|`[^`]+`)/)
    .filter(Boolean)
    .map(part =>
      part.startsWith('**') && part.endsWith('**') && part.length > 4 ? { text: part.slice(2, -2), bold: true }
      : part.startsWith('`') && part.endsWith('`') && part.length > 2 ? { text: part.slice(1, -1), code: true }
      : { text: part })
}

export function parseReleaseNotes(body: string): Block[] {
  const blocks: Block[] = []
  for (const raw of body.split(/\r?\n/)) {
    const line = raw.trim()
    if (!line || /^<br\s*\/?>$/i.test(line)) continue
    if (/^(-{3,}|\*{3,}|_{3,})$/.test(line)) { blocks.push({ kind: 'hr' }); continue }
    const h = /^(#{1,6})\s+(.*)$/.exec(line)
    if (h) { blocks.push({ kind: 'h', level: h[1].length, spans: spans(h[2]) }); continue }
    const li = /^[-*+]\s+(.*)$/.exec(line)
    if (li) {
      const indent = raw.length - raw.trimStart().length
      blocks.push({ kind: 'li', depth: Math.min(Math.floor(indent / 2), 3), spans: spans(li[1]) })
      continue
    }
    blocks.push({ kind: 'p', spans: spans(line) })
  }
  return blocks
}
