// Q&A page: search across questions and answers, filter by topic, deep
// links (#question-id) and shareable searches (?q=…).

const input = document.querySelector('[data-faq-search]')
const count = document.querySelector('[data-faq-count]')
const empty = document.querySelector('[data-faq-empty]')
const chips = [...document.querySelectorAll('.faq-chips button')]
const groups = [...document.querySelectorAll('.faq-group')]
const items = [...document.querySelectorAll('.faq-group details')]
const total = items.length

// Lower-case and drop accents, so "contraseña" finds "contrasena" and the
// other way round.
const fold = (s) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase()

// Each item's searchable text, and its original HTML so marks can be undone.
const index = items.map((d) => ({ d, text: fold(d.textContent), html: d.innerHTML }))

let topic = ''

function mark(el, terms) {
  if (!terms.length) return
  const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT)
  const nodes = []
  while (walker.nextNode()) nodes.push(walker.currentNode)
  for (const node of nodes) {
    const folded = fold(node.data)
    // Folding keeps one character per character for the scripts used here,
    // so positions in `folded` match positions in `node.data`.
    if (folded.length !== node.data.length) continue
    const ranges = []
    for (const t of terms) {
      let at = folded.indexOf(t)
      while (at !== -1) {
        ranges.push([at, at + t.length])
        at = folded.indexOf(t, at + t.length)
      }
    }
    if (!ranges.length) continue
    ranges.sort((a, b) => a[0] - b[0])
    const frag = document.createDocumentFragment()
    let pos = 0
    for (const [from, to] of ranges) {
      if (from < pos) continue
      frag.append(node.data.slice(pos, from))
      const m = document.createElement('mark')
      m.textContent = node.data.slice(from, to)
      frag.append(m)
      pos = to
    }
    frag.append(node.data.slice(pos))
    node.replaceWith(frag)
  }
}

function apply({ fromUser = false } = {}) {
  const query = fold(input.value.trim())
  const terms = query.split(/\s+/).filter(Boolean)
  let shown = 0
  for (const it of index) {
    const wasOpen = it.d.open
    it.d.innerHTML = it.html
    const inTopic = !topic || it.d.closest('.faq-group').dataset.group === topic
    const hit = terms.every((t) => it.text.includes(t))
    const visible = inTopic && hit
    it.d.hidden = !visible
    if (visible) {
      shown++
      mark(it.d, terms)
      // Searching opens what matches; clearing the search closes them again.
      it.d.open = terms.length ? true : fromUser ? false : wasOpen
    }
  }
  for (const g of groups) g.hidden = !g.querySelector('details:not([hidden])')
  empty.hidden = shown > 0
  count.textContent =
    shown === total ? `${total} questions` : `${shown} of ${total} question${total === 1 ? '' : 's'}`
}

input.addEventListener('input', () => {
  apply({ fromUser: true })
  const url = new URL(location.href)
  if (input.value.trim()) url.searchParams.set('q', input.value.trim())
  else url.searchParams.delete('q')
  history.replaceState(null, '', url)
})

chips.forEach((btn) =>
  btn.addEventListener('click', () => {
    topic = btn.dataset.cat
    chips.forEach((b) => b.setAttribute('aria-pressed', String(b === btn)))
    apply()
  }),
)

// "/" focuses the search box, as on many documentation sites.
document.addEventListener('keydown', (e) => {
  if (e.key === '/' && document.activeElement !== input) {
    e.preventDefault()
    input.focus()
  } else if (e.key === 'Escape' && document.activeElement === input && input.value) {
    input.value = ''
    input.dispatchEvent(new Event('input'))
  }
})

// Opening a question puts its link in the address bar, ready to share.
items.forEach((d) =>
  d.addEventListener('toggle', () => {
    if (d.open && !input.value.trim()) history.replaceState(null, '', `#${d.id}`)
  }),
)

const q = new URL(location.href).searchParams.get('q')
if (q) input.value = q
apply()
const target = location.hash && document.getElementById(location.hash.slice(1))
if (target instanceof HTMLDetailsElement) {
  target.open = true
  target.scrollIntoView({ block: 'start' })
}
