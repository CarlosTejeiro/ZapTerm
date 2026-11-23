// "live" mode of the master input bar streams the line to every session as it
// is typed. This works out the bytes that turn what the devices already have
// (`sent`) into what the box now holds (`now`), using Backspace (DEL, 0x7f —
// what the terminal itself sends for that key).

const BS = '\x7f'

export function liveDelta(sent: string, now: string): string {
  if (now.startsWith(sent)) return now.slice(sent.length)
  // Erase back to the longest common start, then type the rest.
  let common = 0
  while (common < sent.length && common < now.length && sent[common] === now[common]) common++
  return BS.repeat([...sent.slice(common)].length) + now.slice(common)
}
