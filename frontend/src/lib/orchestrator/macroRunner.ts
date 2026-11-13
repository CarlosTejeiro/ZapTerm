// On-demand macro execution (the snippet-as-macro path).
//
// Runs a saved expect/send/wait step list against ONE live session, entirely in
// the frontend — the same approach the audit CommandRunner uses: subscribe to
// `terminal-data` for that session, buffer the (ANSI-stripped) output, and send
// keystrokes back via `send_input`. The connect-time login script is the
// backend twin of this (it must run inside the session I/O task); this one is
// user-triggered, so the frontend drives it.

import { invoke } from '@tauri-apps/api/core'
import { matchEnd } from './macroMatch'
import { sendVaultSecret, sendVaultField } from '$lib/bridge/commands'
import { onTerminalData } from '$lib/bridge/events'
import { getFocusedSessionId, focusSession } from '$lib/stores/sessionRuntime.svelte'
import { showToast, type ToastKind } from '$lib/ui/toast-store.svelte'
import type { LoginStep, Snippet } from '$lib/bridge/types'

function stripAnsi(s: string): string {
  return s
    .replace(/\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)/g, '')
    .replace(/\x1b\[[0-9;?]*[ -/]*[@-~]/g, '')
    .replace(/\x1b[@-Z\\-_]/g, '')
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))

/**
 * Decode C-style backslash escapes (`\r` `\n` `\t` `\b` `\e` `\0` `\\` `\xHH`)
 * so a step typed as `cmd\r` presses Enter instead of sending the two literal
 * characters. Unknown/malformed escapes are kept verbatim (backslash included).
 */
export function decodeEscapes(s: string): string {
  let out = ''
  for (let i = 0; i < s.length; i++) {
    if (s[i] !== '\\') {
      out += s[i]
      continue
    }
    const n = s[i + 1]
    switch (n) {
      case 'r':
        out += '\r'
        i++
        break
      case 'n':
        out += '\n'
        i++
        break
      case 't':
        out += '\t'
        i++
        break
      case 'b':
        out += '\b'
        i++
        break
      case 'e':
        out += '\x1b'
        i++
        break
      case '0':
        out += '\0'
        i++
        break
      case '\\':
        out += '\\'
        i++
        break
      case 'x': {
        const hex = s.slice(i + 2, i + 4)
        if (/^[0-9a-fA-F]{2}$/.test(hex)) {
          out += String.fromCharCode(parseInt(hex, 16))
          i += 3
        } else {
          out += '\\' // not a valid \xHH — keep the backslash literally
        }
        break
      }
      default:
        out += '\\' // unknown escape — keep backslash; next iteration emits the char
    }
  }
  return out
}

/** Encode `text` for the PTY, appending Enter unless the user ended the line. */
function withEnter(text: string): number[] {
  let t = text
  if (t.length > 0 && !t.endsWith('\n') && !t.endsWith('\r')) t += '\r'
  return Array.from(new TextEncoder().encode(t))
}

/** A send that is exactly a vault reference `{{vault:<inner>}}` optionally
 *  followed by a trailing `\r`. The inner is either a legacy numeric id (→
 *  password) or `<Name>.<Field>` where Field is Username/Password. Matched
 *  before any escape decoding so neither the secret NOR the username is ever
 *  materialised in JS. */
const VAULT_REF_RE = /^\{\{vault:([^}]+)\}\}(\\r)?$/

async function send(sessionId: string, text: string): Promise<void> {
  const vault = VAULT_REF_RE.exec(text)
  if (vault) {
    // The plaintext / username stays backend-side: ask the host to write it to
    // the PTY directly. `enter` is true when the step had a trailing `\r`.
    const inner = vault[1]!
    const enter = vault[2] !== undefined
    if (/^\d+$/.test(inner)) {
      // Legacy id reference → always the password.
      await sendVaultSecret(sessionId, parseInt(inner, 10), enter)
      return
    }
    // Name+field: split on the LAST dot. A recognised Username/Password suffix
    // selects the field; otherwise the whole inner is the name and we default
    // to the password. A vault name literally ending in `.username`/`.password`
    // would misparse here — the backend forbids creating such names (see
    // `reserved_name_error` in commands/vault.rs), so this stays unambiguous.
    const dot = inner.lastIndexOf('.')
    let name = inner
    let field = 'password'
    if (dot > 0) {
      const suffix = inner.slice(dot + 1).toLowerCase()
      if (suffix === 'username' || suffix === 'password') {
        name = inner.slice(0, dot)
        field = suffix
      }
    }
    await sendVaultField(sessionId, name, field, enter)
    return
  }
  const decoded = decodeEscapes(text)
  if (decoded.length === 0) return
  await invoke('send_input', { sessionId, data: withEnter(decoded) })
}

/** Unwrap a Tauri command error (`{"Internal":"…"}`) to a readable message. */
function errMsg(e: unknown): string {
  const raw =
    e instanceof Error
      ? e.message
      : typeof e === 'string'
        ? e
        : (() => {
            try {
              return JSON.stringify(e)
            } catch {
              return String(e)
            }
          })()
  try {
    const o = JSON.parse(raw) as Record<string, unknown>
    if (typeof o.Internal === 'string') return o.Internal
  } catch {
    /* not JSON */
  }
  return raw
}

export interface MacroResult {
  ok: boolean
  /** Index of the step that timed out or failed, when `ok` is false. */
  failedStep?: number
  /** Human-readable reason when `ok` is false (send/vault error or no match). */
  error?: string
}

/**
 * Execute `steps` against `sessionId`. Resolves when the macro finishes or a
 * step times out. `expect` waits for its pattern (then sends), `send` fires
 * immediately, `wait` pauses. Best-effort: PTY send failures are swallowed.
 */
export async function runMacro(sessionId: string, steps: LoginStep[]): Promise<MacroResult> {
  if (steps.length === 0) return { ok: true }
  const decoder = new TextDecoder()
  let buffer = ''
  const unlisten = await onTerminalData((p) => {
    if (p.session_id === sessionId) {
      buffer += stripAnsi(decoder.decode(new Uint8Array(p.data), { stream: true }))
      if (buffer.length > 64 * 1024) buffer = buffer.slice(-64 * 1024)
    }
  })
  try {
    for (let i = 0; i < steps.length; i++) {
      const step = steps[i]!
      if (step.kind === 'send') {
        try {
          await send(sessionId, step.send)
        } catch (e) {
          return { ok: false, failedStep: i, error: errMsg(e) }
        }
      } else if (step.kind === 'wait') {
        await sleep(step.timeout_ms)
      } else {
        // expect: poll the streaming buffer (which keeps accumulating from the
        // macro's start — including output that arrived during the previous
        // step's send) until the pattern shows or the per-step timeout elapses.
        // On match, CONSUME up to the match end so a later expect can't re-match
        // already-seen output, then send the step's payload.
        const deadline = Date.now() + step.timeout_ms
        let matched = false
        while (Date.now() < deadline) {
          const end = matchEnd(step, buffer)
          if (end >= 0) {
            matched = true
            buffer = buffer.slice(end)
            break
          }
          await sleep(50)
        }
        if (!matched) {
          return {
            ok: false,
            failedStep: i,
            error: `no match for ${step.is_regex ? 'regex ' : ''}"${step.expect}"`,
          }
        }
        try {
          await send(sessionId, step.send)
        } catch (e) {
          return { ok: false, failedStep: i, error: errMsg(e) }
        }
      }
    }
    return { ok: true }
  } finally {
    unlisten()
  }
}

/** Status sink for {@link runMacroOnFocused}. Defaults to toasts; the button
 *  bar passes its own so feedback shows inline next to the bar instead. */
export type MacroNotify = (level: ToastKind, message: string) => void

const toastNotify: MacroNotify = (level, message) => showToast({ kind: level, title: message })

/**
 * Run a macro snippet against the currently-focused session. Shared by the
 * snippet button bar and the sidebar Macros section: resolves the focused
 * session, parses the snippet's steps, runs them, hands focus back to the
 * terminal, and reports progress through `notify`.
 */
export async function runMacroOnFocused(
  snippet: Snippet,
  notify: MacroNotify = toastNotify,
): Promise<void> {
  const focused = getFocusedSessionId()
  if (!focused) {
    notify('warning', 'No session focused — click a terminal first.')
    return
  }
  let steps: LoginStep[]
  try {
    steps = JSON.parse(snippet.steps_json ?? '[]')
  } catch {
    notify('error', `Macro "${snippet.name}" is corrupt`)
    return
  }
  notify('info', `Running macro "${snippet.name}"…`)
  const res = await runMacro(focused, steps)
  focusSession(focused)
  if (res.ok) {
    notify('success', `Macro "${snippet.name}" done`)
  } else {
    const where = `step ${(res.failedStep ?? 0) + 1}`
    notify('error', `Macro "${snippet.name}" failed at ${where}: ${res.error ?? 'timed out'}`)
  }
}
