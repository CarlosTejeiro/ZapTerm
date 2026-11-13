// Detect whether a terminal line is a password prompt.
//
// SECURITY — PRIMARY defense: the backend `is_sensitive` filter only catches
// lines that CONTAIN a keyword like "password"/"secret"/"token" together with
// `:`/`=` (i.e. the prompt label itself). It does NOT catch a bare secret VALUE
// typed at a no-echo password prompt — there the typed characters aren't echoed
// and never appear as a "sensitive"-looking line. So for a secret entered at a
// prompt, this frontend gate is the ONLY thing that stops it from landing in
// command history / recents; it is not a redundant backstop.
//
// At a no-echo prompt the visible cursor line stays as the prompt label, so
// matching that label reliably signals "the next submit is a secret".

// Password keywords whose script uses ASCII word characters at their edges, so
// `\b` boundaries apply (prevents `spinning:` matching `pin`). Includes a few
// common non-English labels (Latin script): Spanish/Portuguese/German/French.
const KW_BOUNDED =
  'password|passphrase|passcode|passwd|pin|secret|contraseña|passwort|senha|mot de passe'

// Keywords in scripts without ASCII word boundaries (`\b` can't anchor them):
// Russian, Chinese, Japanese. Matched as bare substrings.
const KW_UNBOUNDED = 'пароль|密码|パスワード'

// A line ending in a password-style prompt: a password keyword followed by any
// non-terminator text and a terminator. Terminators: `:` `：` (full-width) `>`
// `?` `？`. A keyword is ALWAYS required, so plain prompts like `router>` or
// `show ?` do NOT match (they match only if they also carry a keyword, e.g.
// `Enter your password >`).
const TERM = ':：>?？'
const PASSWORD_PROMPT_RE = new RegExp(
  `(?:\\b(?:${KW_BOUNDED})\\b|${KW_UNBOUNDED})[^${TERM}\\n]*[${TERM}]\\s*$`,
  'i',
)

export function isPasswordPromptLine(line: string): boolean {
  return PASSWORD_PROMPT_RE.test(line.trim())
}
