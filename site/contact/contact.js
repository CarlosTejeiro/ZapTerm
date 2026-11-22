// The contact page is both the form and, right after sending, the thank-you
// message. The form service sends the visitor back here: with ?sent=1 when it
// honours the form's own redirect field, or to the plain address when it uses
// the one set in its dashboard — then the referrer tells.
(function () {
  var sent = /[?&]sent=1(&|$)/.test(location.search)
  if (!sent) {
    try {
      sent = new URL(document.referrer).hostname.endsWith('web3forms.com')
    } catch (e) {
      sent = false
    }
  }
  if (!sent) return
  document.getElementById('contact-form').hidden = true
  document.getElementById('contact-sent').hidden = false
})()
