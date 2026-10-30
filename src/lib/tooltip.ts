// Svelte action: hover tooltip in the app's established style (see ConfigPanel's
// .tooltip). Usage: <span use:tip={'text'}>. Text is set via textContent, never HTML.

const OFFSET = 12
const PAD = 10

export function tip(node: HTMLElement, text: string) {
  let current = text
  let el: HTMLDivElement | null = null

  // Keyboard focus has no pointer: anchor to the element's top-right corner instead
  function pos(e: MouseEvent | FocusEvent) {
    if (e instanceof MouseEvent) return e
    const r = node.getBoundingClientRect()
    return { clientX: r.right, clientY: r.top }
  }

  function place(e: { clientX: number; clientY: number }) {
    if (!el) return
    const r = el.getBoundingClientRect()
    let x = e.clientX + OFFSET
    let y = e.clientY - 8
    if (x + r.width + PAD > window.innerWidth) x = e.clientX - r.width - OFFSET
    x = Math.max(PAD, Math.min(x, window.innerWidth - r.width - PAD))
    if (y < PAD) y = e.clientY + OFFSET
    y = Math.max(PAD, Math.min(y, window.innerHeight - r.height - PAD))
    el.style.left = `${x}px`
    el.style.top = `${y}px`
  }

  function show(e: MouseEvent | FocusEvent) {
    if (!current || el) return
    el = document.createElement('div')
    el.className = 'mhm-tooltip'
    el.textContent = current
    document.body.appendChild(el)
    place(pos(e))
  }

  function hide() {
    el?.remove()
    el = null
  }

  node.addEventListener('mouseenter', show)
  node.addEventListener('mousemove', place)
  node.addEventListener('mouseleave', hide)
  node.addEventListener('focus', show)
  node.addEventListener('blur', hide)

  return {
    update(next: string) {
      current = next
      if (el) el.textContent = next
    },
    destroy() {
      hide()
      node.removeEventListener('mouseenter', show)
      node.removeEventListener('mousemove', place)
      node.removeEventListener('mouseleave', hide)
      node.removeEventListener('focus', show)
      node.removeEventListener('blur', hide)
    },
  }
}
