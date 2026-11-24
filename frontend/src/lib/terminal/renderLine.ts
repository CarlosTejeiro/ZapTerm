// One line of terminal output as the terminal shows it.

/**
 * A carriage return goes back to the start of the line and a backspace one
 * column left; neither erases anything — what is written next overwrites
 * what was there. Devices use this to redraw a line or to erase a pager
 * prompt (`--More--` followed by backspaces, or by `\r` and spaces). A `\r`
 * at the very end of a line (the first half of a `\r\r\n` line ending)
 * therefore changes nothing.
 */
export function renderLine(line: string): string {
  if (!/[\r\b]/.test(line)) return line
  const cells: string[] = []
  let col = 0
  for (const ch of line) {
    if (ch === '\r') col = 0
    else if (ch === '\b') col = Math.max(0, col - 1)
    else cells[col++] = ch
  }
  return Array.from(cells, (c) => c ?? ' ').join('')
}
