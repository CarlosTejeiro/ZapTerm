// Command lists: grouped, per-vendor commands shown as menus in the bottom
// bar. A command may carry `<placeholders>`; a recipe is several lines sent
// in order after a small form fills its fields.

export type ListIcon =
  | 'system'
  | 'interfaces'
  | 'routing'
  | 'sdwan'
  | 'bgp'
  | 'ospf'
  | 'vpn'
  | 'ha'
  | 'security'
  | 'users'
  | 'fortilink'
  | 'logs'
  | 'debug'

export interface CommandItem {
  kind: 'command'
  /** The command; `<name>` marks a part the user fills in. */
  cmd: string
  desc: string
  /** Changes state on the device (flush, clear…): asks before sending. */
  action?: boolean
  /** Can load the device (a real-time debug, a capture): shown as a warning
   *  and confirmed before sending. Says why, citing the vendor. */
  caution?: string
  /** Raw keys sent instead of `cmd` (e.g. "\x03" for Ctrl+C); `cmd` is then
   *  only the label. */
  keys?: string
}

export interface RecipeField {
  key: string
  label: string
  default?: string
  placeholder?: string
}

export interface RecipeItem {
  kind: 'recipe'
  name: string
  desc: string
  fields: RecipeField[]
  /** Sent one by one; `{{key}}` takes the field's value. */
  lines: string[]
  /** As for commands: a warning confirmed before the lines are sent. */
  caution?: string
}

export type ListItem = CommandItem | RecipeItem

export interface ListSection {
  title: string
  items: ListItem[]
}

export interface CommandList {
  id: string
  name: string
  icon: ListIcon
  /** Accent for the chip icon and the open panel. */
  color: string
  summary: string
  sections: ListSection[]
  /** Debug lists show a "Stop debug" button and mark the chip while a
   *  recipe is running. */
  debug?: boolean
}

export interface VendorLists {
  /** Shown next to the chips, e.g. "FortiGate · FortiOS 7.6". */
  label: string
  /** Lines that stop every running debug (debug lists). */
  stopDebug: string[]
  /** Raw keys sent before `stopDebug` (FortiOS: Ctrl+C, so a sniffer or
   *  trace still printing doesn't swallow the lines). */
  stopKeys?: string
  lists: CommandList[]
}
