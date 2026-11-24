// Keeps passwords out of what ZapTerm writes down about automation (the
// activity file and the panel's log). A command line is kept as typed, with
// the value after a secret keyword replaced by MASK. Best effort: a secret in
// a place no rule knows (an unusual command, a positional argument) is not
// recognised. What is sent to the device is never changed.

export const MASK = '******'

/** A word that names a secret: the value after it is masked. Matched against
 *  one whitespace-separated token, lowercased, without leading dashes. */
const SECRET_WORD =
  /^(?:.*(?:password|passwd|passphrase|secret|psk)|pwd|.*[-_]pwd|key|.*[-_]key|authkey|key-string|community|token|.*[-_]token)$/

/** Words that may stand between the keyword and its value: how the value is
 *  encoded (`password 7 0822455D0A16`, `set password ENC …`, `key ascii …`). */
const ENCODING_WORD =
  /^(?:\d|enc|md5|sha|sha1|sha256|sha384|sha512|ascii|ascii-text|hex|hexadecimal|plain|plaintext|plain-text|clear|cleartext|encrypted|unencrypted|local|simple|cipher)$/

/** `--password=x`, `password=x`, `PGPASSWORD=x`, `token: x` inside one token. */
const ASSIGNMENT =
  /^(-{0,2}[\w.-]*(?:password|passwd|passphrase|secret|pwd|token|psk|apikey|api[-_]key)[\w.-]*[=:])(.+)$/i

/** `user:password@host` in a URL, and `curl -u user:password`. */
const URL_CREDENTIALS = /^(\w[\w+.-]*:\/\/[^\s:/@]+:)[^\s@/]+(@.*)$/

/** Split into tokens keeping the whitespace between them, quotes kept whole. */
function tokens(line: string): string[] {
  return line.match(/\s+|"(?:[^"\\]|\\.)*"?|'[^']*'?|[^\s"']+/g) ?? []
}

const isSpace = (t: string) => /^\s+$/.test(t)

/**
 * `command` with the values of passwords, keys, secrets, community strings
 * and tokens replaced by `******`.
 */
export function maskSecrets(command: string): string {
  const parts = tokens(command)
  const words = parts.map((p, i) => (isSpace(p) ? -1 : i)).filter((i) => i >= 0)
  /** Index in `words` of the next maskable value after position `w`. */
  const maskAfter = (w: number) => {
    let v = w + 1
    while (v < words.length && ENCODING_WORD.test(parts[words[v]!]!.toLowerCase())) v++
    // A pipe or redirection ends the command: nothing after it is a value.
    if (v < words.length && !/^[|;&<>]/.test(parts[words[v]!]!)) parts[words[v]!] = MASK
  }

  for (let w = 0; w < words.length; w++) {
    const raw = parts[words[w]!]!
    if (raw === MASK) continue
    const assigned = ASSIGNMENT.exec(raw)
    if (assigned) {
      parts[words[w]!] = assigned[1] + MASK
      continue
    }
    const url = URL_CREDENTIALS.exec(raw)
    if (url) {
      parts[words[w]!] = url[1] + MASK + url[2]
      continue
    }
    const word = raw.toLowerCase().replace(/^-+/, '')
    if (SECRET_WORD.test(word)) {
      maskAfter(w)
      continue
    }
    // `curl -u user:password`, `sshpass -p password`.
    if ((word === 'u' || word === 'user') && raw.startsWith('-') && w + 1 < words.length) {
      const next = parts[words[w + 1]!]!
      const colon = next.indexOf(':')
      if (colon > 0) parts[words[w + 1]!] = next.slice(0, colon + 1) + MASK
    }
    if (word === 'p' && raw === '-p' && w > 0 && /sshpass$/i.test(parts[words[w - 1]!]!)) {
      maskAfter(w)
    }
  }

  // FortiOS `execute backup|restore … ftp|sftp <file> <server> <user> <password>`:
  // the password is the fourth argument after the protocol.
  const lower = words.map((i) => parts[i]!.toLowerCase())
  if (lower[0] === 'execute' && (lower[1] === 'backup' || lower[1] === 'restore')) {
    const proto = lower.findIndex((t) => t === 'ftp' || t === 'sftp')
    if (proto > 0 && proto + 4 < words.length) parts[words[proto + 4]!] = MASK
  }
  return parts.join('')
}

/** True when `line` (the last line a device printed) is asking for a secret:
 *  what is typed next is one, whatever it looks like. */
export function asksForSecret(line: string): boolean {
  return /(?:password|passphrase|passcode|secret|pass ?key|\bpin\b)[^\n]{0,40}[:?>]\s*$/i.test(line)
}
