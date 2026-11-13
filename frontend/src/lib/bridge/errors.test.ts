import { describe, expect, it } from 'vitest'
import { errorText } from './errors'

describe('errorText', () => {
  it('unwraps the serialized AppError a Tauri command rejects with', () => {
    expect(errorText({ Internal: 'sftp: request timed out' })).toBe('sftp: request timed out')
    expect(errorText('{"Internal":"no such file"}')).toBe('no such file')
  })
  it('keeps Errors, plain strings and other objects readable', () => {
    expect(errorText(new Error('boom'))).toBe('boom')
    expect(errorText('plain')).toBe('plain')
    expect(errorText({ code: 7 })).toBe('{"code":7}')
    expect(errorText(undefined)).toBe('undefined')
  })
})
