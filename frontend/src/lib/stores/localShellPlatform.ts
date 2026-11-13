/** Whether ZapTerm runs on Windows (WebView2's user agent says so). */
export const isWindowsHost = typeof navigator !== 'undefined' && /Windows/.test(navigator.userAgent)

/** The command lists for a local shell: Windows shells (PowerShell, cmd) get
 *  the Windows lists, anything else (bash, zsh, fish, Git Bash…) the Linux
 *  ones. Unknown shell: whatever the host OS suggests. */
export function platformForShell(
  shell: string | null | undefined,
  windowsHost = isWindowsHost,
): 'windows' | 'linux' {
  if (!shell) return windowsHost ? 'windows' : 'linux'
  const base = (shell.split(/[\\/]/).pop() ?? '').toLowerCase()
  return /^(pwsh|powershell|cmd)(\.exe)?$/.test(base) ? 'windows' : 'linux'
}
