// ZapTerm website: gallery tabs, lightbox, scroll reveal and download links
// resolved from the latest GitHub release (static links remain as fallback).

// Gallery tabs: swap the image of the window that follows the tab row.
document.querySelectorAll('[data-tabs]').forEach((row) => {
  const img = row.nextElementSibling.querySelector('img')
  row.querySelectorAll('button').forEach((btn) => {
    if (btn.dataset.swatch) {
      const sw = document.createElement('span')
      sw.className = 'sw'
      sw.style.background = btn.dataset.swatch
      btn.prepend(sw)
    }
    // Warm the cache so switching is instant.
    new Image().src = btn.dataset.src
    btn.addEventListener('click', () => {
      row.querySelectorAll('button').forEach((b) => b.setAttribute('aria-selected', String(b === btn)))
      img.classList.add('fading')
      setTimeout(() => {
        img.src = btn.dataset.src
        img.alt = btn.dataset.alt
        img.onload = () => img.classList.remove('fading')
      }, 180)
    })
  })
})

// Lightbox: click any screenshot to see it full size.
const box = document.querySelector('.lightbox')
const boxImg = box.querySelector('img')
document.addEventListener('click', (e) => {
  const t = e.target
  if (t instanceof HTMLImageElement && t.hasAttribute('data-zoom')) {
    boxImg.src = t.currentSrc || t.src
    boxImg.alt = t.alt
    box.hidden = false
  } else if (!box.hidden && box.contains(t)) {
    box.hidden = true
  }
})
document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') box.hidden = true
})

// Reveal sections as they scroll into view.
if ('IntersectionObserver' in window) {
  const io = new IntersectionObserver(
    (entries) => entries.forEach((en) => {
      if (en.isIntersecting) {
        en.target.classList.add('in')
        io.unobserve(en.target)
      }
    }),
    { rootMargin: '0px 0px -10% 0px' },
  )
  document.querySelectorAll('.feature, .card, .dl-card, .gallery .window, .zapx-inner').forEach((el) => {
    el.classList.add('reveal')
    io.observe(el)
  })
}

// Highlight the visitor's platform.
const ua = navigator.userAgent
const os = /Windows/.test(ua) ? 'windows' : /Mac OS X|Macintosh/.test(ua) ? 'mac' : /Linux|X11/.test(ua) ? 'linux' : null
if (os) document.querySelector(`.dl-card[data-os="${os}"]`)?.classList.add('is-you')

// Point each download button at the matching asset of the latest release.
fetch('https://api.github.com/repos/CarlosTejeiro/ZapTerm/releases/latest', {
  headers: { Accept: 'application/vnd.github+json' },
})
  .then((r) => (r.ok ? r.json() : Promise.reject(r.status)))
  .then((rel) => {
    document.querySelectorAll('[data-version]').forEach((el) => (el.textContent = rel.tag_name))
    document.querySelectorAll('[data-asset]').forEach((a) => {
      const asset = rel.assets.find((x) => x.name.endsWith(a.dataset.asset))
      if (asset) a.href = asset.browser_download_url
    })
    const primary = os && document.querySelector(`.dl-card[data-os="${os}"] .btn-primary`)
    const hero = document.querySelector('[data-dl-primary]')
    if (primary && hero && primary.href.includes('/download/')) {
      hero.href = primary.href
      hero.textContent = `Download for ${{ windows: 'Windows', mac: 'macOS', linux: 'Linux' }[os]}`
    }
  })
  .catch(() => {
    /* offline or rate-limited: the static links to the releases page stay */
  })
