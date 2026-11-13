import { describe, expect, it } from 'vitest'
import { platformForShell } from './localShellPlatform'

describe('platformForShell', () => {
  it('gives the Windows lists to PowerShell and cmd', () => {
    expect(platformForShell('C:\\Program Files\\PowerShell\\7\\pwsh.exe', 'windows')).toBe(
      'windows',
    )
    expect(
      platformForShell('C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe', 'windows'),
    ).toBe('windows')
    expect(platformForShell('C:\\WINDOWS\\system32\\cmd.exe', 'windows')).toBe('windows')
  })

  it('gives the Linux lists to unix shells on Linux and Windows (Git Bash)', () => {
    expect(platformForShell('/bin/zsh', 'linux')).toBe('linux')
    expect(platformForShell('/usr/bin/fish', 'linux')).toBe('linux')
    expect(platformForShell('C:\\Program Files\\Git\\bin\\bash.exe', 'windows')).toBe('linux')
  })

  it('gives the macOS lists to unix shells on a Mac', () => {
    expect(platformForShell('/bin/zsh', 'macos')).toBe('macos')
    expect(platformForShell('/opt/homebrew/bin/bash', 'macos')).toBe('macos')
    expect(platformForShell('/usr/local/bin/pwsh', 'macos')).toBe('windows')
  })

  it('falls back to the host OS when the shell is unknown', () => {
    expect(platformForShell(undefined, 'windows')).toBe('windows')
    expect(platformForShell('', 'linux')).toBe('linux')
    expect(platformForShell(null, 'macos')).toBe('macos')
  })
})
