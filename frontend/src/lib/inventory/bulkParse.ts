// Parse a device list for File → Add devices in bulk…
//
// Two inputs are accepted:
//   • a CSV file (or pasted CSV) whose first line names the columns —
//     separated by commas, semicolons (what spreadsheets save in many
//     locales) or tabs, with "quoted, values" as spreadsheets write them;
//   • a plain list, one device per line: `host`, `host:port`, `name host`
//     or `name host port` (spaces, tabs or commas between them).
// Blank lines and lines starting with `#` are skipped. Values a line leaves
// out come from the dialog's defaults; the backend then checks every row
// against the inventory (duplicates, folders, vault, jump host).

export type Protocol = 'ssh' | 'telnet'

export interface BulkDefaults {
  protocol: Protocol
  /** Empty → 22 for SSH, 23 for Telnet. */
  port: number | null
  username: string
  folder: string
  tags: string[]
  credential: string
  jumpHost: string
}

/** What the backend's `bulk_add_sessions` takes. */
export interface BulkRow {
  name: string
  protocol: string
  host: string
  port: number
  username?: string
  folder?: string
  tags: string[]
  notes?: string
  credential?: string
  jumpHost?: string
}

export interface ParsedRow {
  /** 1-based line number in the input, for messages. */
  line: number
  row: BulkRow
  /** A problem found while reading the line (bad port, missing host…). */
  problem?: string
}

export interface ParseResult {
  kind: 'csv' | 'list' | 'empty'
  rows: ParsedRow[]
  /** Header names that weren't recognised (CSV only); their values are ignored. */
  ignoredColumns: string[]
}

export const DEFAULT_PORT: Record<Protocol, number> = { ssh: 22, telnet: 23 }

type Field =
  | 'name'
  | 'host'
  | 'port'
  | 'protocol'
  | 'username'
  | 'folder'
  | 'tags'
  | 'notes'
  | 'credential'
  | 'jumpHost'

/** Accepted header names (lower-case, spaces/dashes/underscores removed). */
const ALIASES: Record<string, Field> = {
  name: 'name',
  device: 'name',
  devicename: 'name',
  hostname: 'name',
  session: 'name',
  sessionname: 'name',
  label: 'name',
  host: 'host',
  ip: 'host',
  ipaddress: 'host',
  address: 'host',
  mgmtip: 'host',
  managementip: 'host',
  fqdn: 'host',
  port: 'port',
  protocol: 'protocol',
  proto: 'protocol',
  username: 'username',
  user: 'username',
  login: 'username',
  folder: 'folder',
  path: 'folder',
  group: 'folder',
  site: 'folder',
  tags: 'tags',
  tag: 'tags',
  notes: 'notes',
  note: 'notes',
  description: 'notes',
  comment: 'notes',
  credential: 'credential',
  vault: 'credential',
  vaultcredential: 'credential',
  jumphost: 'jumpHost',
  jump: 'jumpHost',
  via: 'jumpHost',
  proxyjump: 'jumpHost',
  bastion: 'jumpHost',
}

const norm = (h: string) => h.toLowerCase().replace(/[\s_-]+/g, '')

/** Split one CSV record, honouring "quoted ""values""" with `delim` inside. */
export function splitCsvLine(line: string, delim: string): string[] {
  const out: string[] = []
  let cur = ''
  let quoted = false
  for (let i = 0; i < line.length; i++) {
    const c = line[i]!
    if (quoted) {
      if (c === '"') {
        if (line[i + 1] === '"') {
          cur += '"'
          i++
        } else quoted = false
      } else cur += c
    } else if (c === '"' && cur.trim() === '') {
      quoted = true
      cur = ''
    } else if (c === delim) {
      out.push(cur.trim())
      cur = ''
    } else cur += c
  }
  out.push(cur.trim())
  return out
}

/** Records of a CSV text: newlines inside quotes stay in the value. */
function csvRecords(text: string): { line: number; text: string }[] {
  const out: { line: number; text: string }[] = []
  let cur = ''
  let start = 1
  let lineNo = 1
  let quoted = false
  for (const ch of text) {
    if (ch === '"') quoted = !quoted
    if (ch === '\n' && !quoted) {
      out.push({ line: start, text: cur.replace(/\r$/, '') })
      cur = ''
      lineNo++
      start = lineNo
      continue
    }
    if (ch === '\n') lineNo++
    cur += ch
  }
  if (cur.trim()) out.push({ line: start, text: cur.replace(/\r$/, '') })
  return out
}

const skip = (l: string) => l.trim() === '' || l.trimStart().startsWith('#')

/** The header's delimiter and column mapping, when the first line is a header. */
function detectHeader(
  first: string,
): { delim: string; fields: (Field | null)[]; raw: string[] } | null {
  for (const delim of ['\t', ';', ',']) {
    if (!first.includes(delim)) continue
    const raw = splitCsvLine(first, delim)
    const fields = raw.map((h) => ALIASES[norm(h)] ?? null)
    if (fields.includes('host')) return { delim, fields, raw }
  }
  // A single column called host/ip/address is a header too.
  const only = ALIASES[norm(first.trim())]
  if (only === 'host') return { delim: ',', fields: ['host'], raw: [first.trim()] }
  return null
}

/** `[v6]:port`, `host:port` (one colon) → host and port; anything else as is. */
export function splitHostPort(s: string): { host: string; port?: string } {
  const v6 = /^\[([^\]]+)\](?::(\d*))?$/.exec(s)
  if (v6) return { host: v6[1]!, port: v6[2] || undefined }
  const parts = s.split(':')
  if (parts.length === 2 && parts[1] !== '') return { host: parts[0]!, port: parts[1] }
  return { host: s }
}

function parsePort(s: string | undefined): { port?: number; problem?: string } {
  if (s === undefined || s.trim() === '') return {}
  const t = s.trim()
  if (!/^\d+$/.test(t)) return { problem: `port “${t}” isn't a number` }
  const n = Number(t)
  if (n < 1 || n > 65535) return { problem: `port ${n} is out of range (1–65535)` }
  return { port: n }
}

function parseProtocol(s: string | undefined): { protocol?: Protocol; problem?: string } {
  if (s === undefined || s.trim() === '') return {}
  const t = s.trim().toLowerCase()
  if (t === 'ssh' || t === 'ssh2') return { protocol: 'ssh' }
  if (t === 'telnet') return { protocol: 'telnet' }
  return { problem: `protocol “${s.trim()}” — use ssh or telnet` }
}

const splitTags = (s: string) =>
  s
    .split(/[,;|]/)
    .map((t) => t.trim())
    .filter(Boolean)

function build(
  line: number,
  v: Partial<Record<Field, string>>,
  d: BulkDefaults,
  hostPort: { host: string; port?: string },
): ParsedRow {
  const problems: string[] = []
  const proto = parseProtocol(v.protocol)
  if (proto.problem) problems.push(proto.problem)
  const protocol = proto.protocol ?? d.protocol
  // An explicit port column wins over a `host:port`.
  const p = parsePort(v.port?.trim() ? v.port : hostPort.port)
  if (p.problem) problems.push(p.problem)
  const port = p.port ?? d.port ?? DEFAULT_PORT[protocol]
  const host = hostPort.host.trim()
  if (!host) problems.push('no host')
  const name = (v.name ?? '').trim() || host
  const tags = [...d.tags, ...splitTags(v.tags ?? '')].filter((t, i, a) => a.indexOf(t) === i)
  const pick = (val: string | undefined, def: string) => {
    const s = (val ?? '').trim()
    return s || def.trim() || undefined
  }
  const row: BulkRow = { name, protocol, host, port, tags }
  const username = pick(v.username, d.username)
  if (username) row.username = username
  const folder = pick(v.folder, d.folder)
  if (folder) row.folder = folder
  const notes = (v.notes ?? '').trim()
  if (notes) row.notes = notes
  const credential = pick(v.credential, d.credential)
  if (credential) row.credential = credential
  const jumpHost = pick(v.jumpHost, d.jumpHost)
  if (jumpHost) row.jumpHost = jumpHost
  const out: ParsedRow = { line, row }
  if (problems.length) out.problem = problems.join(' · ')
  return out
}

/** One line of a plain list → its values. */
function listLine(text: string): {
  values: Partial<Record<Field, string>>
  hostPort: { host: string; port?: string }
  problem?: string
} {
  const tokens = text
    .split(/[\s,;]+/)
    .map((t) => t.trim())
    .filter(Boolean)
  if (tokens.length === 1) return { values: {}, hostPort: splitHostPort(tokens[0]!) }
  if (tokens.length === 2) {
    // `host port` or `name host`.
    if (/^\d+$/.test(tokens[1]!)) {
      return { values: { port: tokens[1]! }, hostPort: splitHostPort(tokens[0]!) }
    }
    return { values: { name: tokens[0]! }, hostPort: splitHostPort(tokens[1]!) }
  }
  if (tokens.length === 3) {
    return { values: { name: tokens[0]!, port: tokens[2]! }, hostPort: splitHostPort(tokens[1]!) }
  }
  return {
    values: { name: tokens[0]! },
    hostPort: splitHostPort(tokens[1] ?? ''),
    problem: `${tokens.length} values on the line — expected host, name host or name host port`,
  }
}

export function parseDeviceList(text: string, defaults: BulkDefaults): ParseResult {
  const clean = text.replace(/^﻿/, '')
  const records = csvRecords(clean).filter((r) => !skip(r.text))
  if (records.length === 0) return { kind: 'empty', rows: [], ignoredColumns: [] }

  const header = detectHeader(records[0]!.text)
  if (header) {
    const ignoredColumns = header.raw.filter((_, i) => header.fields[i] === null && header.raw[i])
    const rows = records.slice(1).map((r) => {
      const cells = header.delim ? splitCsvLine(r.text, header.delim) : [r.text]
      const v: Partial<Record<Field, string>> = {}
      header.fields.forEach((f, i) => {
        if (f && cells[i] !== undefined && v[f] === undefined) v[f] = cells[i]
      })
      return build(r.line, v, defaults, splitHostPort((v.host ?? '').trim()))
    })
    return { kind: 'csv', rows, ignoredColumns }
  }

  const rows = records.map((r) => {
    const l = listLine(r.text)
    const parsed = build(r.line, l.values, defaults, l.hostPort)
    if (l.problem) parsed.problem = parsed.problem ? `${l.problem} · ${parsed.problem}` : l.problem
    return parsed
  })
  return { kind: 'list', rows, ignoredColumns: [] }
}

/** A CSV template with every column the import understands. */
export const CSV_TEMPLATE =
  'name,host,port,protocol,username,folder,tags,notes,credential,jump_host\n' +
  'core-sw-01,10.10.0.2,22,ssh,netops,DC/Madrid,"core,mad",Rack 4,Lab admin,bastion\n'
