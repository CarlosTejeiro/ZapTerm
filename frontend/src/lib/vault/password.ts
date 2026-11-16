/** What stops a new master password from being accepted, or '' when it is fine. */
export function passwordProblem(password: string, repeat: string, minChars: number): string {
  // Counted in characters, like the backend, so "ñ" is one.
  if ([...password].length < minChars) return `At least ${minChars} characters.`
  if (password !== repeat) return "The passwords don't match."
  return ''
}
