import { describe, it, expect } from 'vitest'
import { isPasswordPromptLine } from './passwordPrompt'

describe('isPasswordPromptLine', () => {
  it('matches common password prompts', () => {
    expect(isPasswordPromptLine('password:')).toBe(true)
    expect(isPasswordPromptLine('Password:')).toBe(true)
    expect(isPasswordPromptLine('passphrase:')).toBe(true)
    expect(isPasswordPromptLine('passcode:')).toBe(true)
    expect(isPasswordPromptLine('pin:')).toBe(true)
    expect(isPasswordPromptLine('secret:')).toBe(true)
    expect(isPasswordPromptLine('[sudo] password for user:')).toBe(true)
    expect(isPasswordPromptLine("Enter passphrase for key '/home/u/.ssh/id_ed25519':")).toBe(true)
    // Trailing whitespace after the colon is common and must still match.
    expect(isPasswordPromptLine('Password: ')).toBe(true)
  })

  it('matches common non-English password prompts', () => {
    expect(isPasswordPromptLine('Contraseña:')).toBe(true)
    expect(isPasswordPromptLine('Passwort:')).toBe(true)
    expect(isPasswordPromptLine('Senha:')).toBe(true)
    expect(isPasswordPromptLine('Mot de passe :')).toBe(true)
    expect(isPasswordPromptLine('пароль:')).toBe(true)
    expect(isPasswordPromptLine('密码：')).toBe(true) // full-width colon
    expect(isPasswordPromptLine('パスワード:')).toBe(true)
  })

  it('matches non-colon terminators when a keyword is present', () => {
    expect(isPasswordPromptLine('Enter your password >')).toBe(true)
    expect(isPasswordPromptLine('password?')).toBe(true)
    expect(isPasswordPromptLine('密码？')).toBe(true) // full-width question mark
  })

  it('does not match normal command lines or username prompts', () => {
    expect(isPasswordPromptLine('login:')).toBe(false)
    expect(isPasswordPromptLine('username:')).toBe(false)
    expect(isPasswordPromptLine('admin@host:~$')).toBe(false)
    expect(isPasswordPromptLine('Router#')).toBe(false)
    expect(isPasswordPromptLine('router> show version')).toBe(false)
    expect(isPasswordPromptLine('spinning:')).toBe(false)
    // Bare terminators with no password keyword must NOT match.
    expect(isPasswordPromptLine('router>')).toBe(false)
    expect(isPasswordPromptLine('show ?')).toBe(false)
  })
})
