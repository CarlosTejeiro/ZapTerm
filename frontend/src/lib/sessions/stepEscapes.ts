// Login-step / trigger text is edited in one-line inputs, so control
// characters are written as escapes: `\r` `\n` `\t`, and `\\` for a backslash.

/** Escapes typed in the form → the characters stored. Unknown escapes stay. */
export function unescapeControls(s: string): string {
  let out = ''
  for (let i = 0; i < s.length; i += 1) {
    if (s[i] === '\\' && i + 1 < s.length) {
      const next = s[i + 1]
      const ch =
        next === 'n'
          ? '\n'
          : next === 'r'
            ? '\r'
            : next === 't'
              ? '\t'
              : next === '\\'
                ? '\\'
                : null
      if (ch !== null) {
        out += ch
        i += 1
        continue
      }
    }
    out += s[i]
  }
  return out
}

/** Stored characters → the escapes shown in the form (inverse of the above). */
export function escapeControls(s: string): string {
  return s.replace(/\\/g, '\\\\').replace(/\r/g, '\\r').replace(/\n/g, '\\n').replace(/\t/g, '\\t')
}
