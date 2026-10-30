// Svelte action: hover tooltip in the app's established style (see ConfigPanel's
// .tooltip). Usage: <span use:tip={'text'}>. Text is set via textContent, never HTML.

const OFFSET = 12
const PAD = 10
const DELAY = 400 // ms of hovering before a tooltip appears

export function tip(node: HTMLElement, text: string) {
  let current = text
  let el: HTMLDivElement | null = null
  let timer: ReturnType<typeof setTimeout> | undefined
  let last = { clientX: 0, clientY: 0 }

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

  function open(at: { clientX: number; clientY: number }) {
    timer = undefined
    if (!current || el) return
    el = document.createElement('div')
    el.className = 'mhm-tooltip'
    el.textContent = current
    document.body.appendChild(el)
    place(at)
  }

  // Mouse waits like a native tooltip; keyboard focus has no hover gesture, so it is instant
  function showMouse(e: MouseEvent) {
    last = e
    if (!el && timer === undefined) timer = setTimeout(() => open(last), DELAY)
  }

  function showFocus(e: FocusEvent) {
    if (node.matches(':focus-visible')) open(pos(e)) // a click also focuses; only keyboard focus counts
  }

  function move(e: MouseEvent) {
    last = e
    place(e)
  }

  function hide() {
    clearTimeout(timer)
    timer = undefined
    el?.remove()
    el = null
  }

  node.addEventListener('mouseenter', showMouse)
  node.addEventListener('mousemove', move)
  node.addEventListener('mouseleave', hide)
  node.addEventListener('focus', showFocus)
  node.addEventListener('blur', hide)

  return {
    update(next: string) {
      current = next
      if (el) el.textContent = next
    },
    destroy() {
      hide()
      node.removeEventListener('mouseenter', showMouse)
      node.removeEventListener('mousemove', move)
      node.removeEventListener('mouseleave', hide)
      node.removeEventListener('focus', showFocus)
      node.removeEventListener('blur', hide)
    },
  }
}
