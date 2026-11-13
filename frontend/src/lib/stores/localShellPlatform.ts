/** Whether ZapTerm runs on Windows (WebView2's user agent says so). */
export const isWindowsHost = typeof navigator !== 'undefined' && /Windows/.test(navigator.userAgent)

/** Whether ZapTerm runs on macOS (WKWebView's user agent says "Macintosh"). */
export const isMacHost =
  typeof navigator !== 'undefined' && /Macintosh|Mac OS X/.test(navigator.userAgent)

export type LocalHost = 'windows' | 'macos' | 'linux'

export const localHost: LocalHost = isWindowsHost ? 'windows' : isMacHost ? 'macos' : 'linux'

/** The command lists for a local shell: Windows shells (PowerShell, cmd) get
 *  the Windows lists; any other shell (bash, zsh, fish, Git Bash…) gets the
 *  macOS lists on a Mac and the Linux ones elsewhere. Unknown shell: whatever
 *  the host OS suggests. */
export function platformForShell(
  shell: string | null | undefined,
  host: LocalHost = localHost,
): LocalHost {
  const unix = host === 'macos' ? 'macos' : 'linux'
  if (!shell) return host === 'windows' ? 'windows' : unix
  const base = (shell.split(/[\\/]/).pop() ?? '').toLowerCase()
  return /^(pwsh|powershell|cmd)(\.exe)?$/.test(base) ? 'windows' : unix
}
