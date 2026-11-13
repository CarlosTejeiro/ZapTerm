import { describe, expect, it } from 'vitest'
import { platformForShell } from './localShellPlatform'

describe('platformForShell', () => {
  it('gives the Windows lists to PowerShell and cmd', () => {
    expect(platformForShell('C:\\Program Files\\PowerShell\\7\\pwsh.exe')).toBe('windows')
    expect(platformForShell('C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe')).toBe(
      'windows',
    )
    expect(platformForShell('C:\\WINDOWS\\system32\\cmd.exe')).toBe('windows')
  })

  it('gives the Linux lists to unix shells, Git Bash included', () => {
    expect(platformForShell('/bin/zsh')).toBe('linux')
    expect(platformForShell('/usr/bin/fish')).toBe('linux')
    expect(platformForShell('C:\\Program Files\\Git\\bin\\bash.exe')).toBe('linux')
  })

  it('falls back to the host OS when the shell is unknown', () => {
    expect(platformForShell(undefined, true)).toBe('windows')
    expect(platformForShell('', false)).toBe('linux')
  })
})
