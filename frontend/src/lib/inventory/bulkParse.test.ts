import { describe, expect, it } from 'vitest'
import {
  CSV_TEMPLATE,
  parseDeviceList,
  splitCsvLine,
  splitHostPort,
  type BulkDefaults,
} from './bulkParse'

const d: BulkDefaults = {
  protocol: 'ssh',
  port: null,
  username: '',
  folder: '',
  tags: [],
  credential: '',
  jumpHost: '',
}

describe('plain lists', () => {
  it('reads host, host:port, name host and name host port', () => {
    const r = parseDeviceList(
      '# core switches\n10.0.0.1\n\nsw-02 10.0.0.2\nfw.lab:2222\nrtr-04, 10.0.0.4, 830\n[2001:db8::5]:22\n',
      d,
    )
    expect(r.kind).toBe('list')
    expect(r.rows.map((x) => [x.line, x.row.name, x.row.host, x.row.port, x.problem])).toEqual([
      [2, '10.0.0.1', '10.0.0.1', 22, undefined],
      [4, 'sw-02', '10.0.0.2', 22, undefined],
      [5, 'fw.lab', 'fw.lab', 2222, undefined],
      [6, 'rtr-04', '10.0.0.4', 830, undefined],
      [7, '2001:db8::5', '2001:db8::5', 22, undefined],
    ])
  })

  it('takes `host port` when the second value is a number', () => {
    const r = parseDeviceList('10.0.0.9 2323', { ...d, protocol: 'telnet' })
    expect(r.rows[0]!.row).toMatchObject({ host: '10.0.0.9', port: 2323, protocol: 'telnet' })
  })

  it('fills what a line leaves out from the defaults', () => {
    const r = parseDeviceList('sw-01 10.0.0.1', {
      protocol: 'telnet',
      port: null,
      username: 'netops',
      folder: 'Lab/Rack 2',
      tags: ['lab'],
      credential: 'Lab admin',
      jumpHost: 'bastion',
    })
    expect(r.rows[0]!.row).toEqual({
      name: 'sw-01',
      host: '10.0.0.1',
      protocol: 'telnet',
      port: 23,
      username: 'netops',
      folder: 'Lab/Rack 2',
      tags: ['lab'],
      credential: 'Lab admin',
      jumpHost: 'bastion',
    })
  })

  it('reports lines it cannot read instead of guessing', () => {
    const r = parseDeviceList('a b c d\nsw 10.0.0.1 99999\nsw2 10.0.0.2 ssh', d)
    expect(r.rows[0]!.problem).toMatch(/4 values/)
    expect(r.rows[1]!.problem).toMatch(/out of range/)
    expect(r.rows[2]!.problem).toMatch(/isn't a number/)
  })
})

describe('CSV files', () => {
  it('maps the header, in any order and with common names', () => {
    const csv =
      'Device Name;Mgmt IP;Proto;User;Site;Tags;Description\n' +
      'edge-01;192.0.2.1;telnet;;WAN;wan|edge;"Main; uplink"\n' +
      'edge-02;192.0.2.2;;admin;WAN;;\n'
    const r = parseDeviceList(csv, { ...d, tags: ['imported'] })
    expect(r.kind).toBe('csv')
    expect(r.rows.map((x) => x.row)).toEqual([
      {
        name: 'edge-01',
        host: '192.0.2.1',
        protocol: 'telnet',
        port: 23,
        folder: 'WAN',
        tags: ['imported', 'wan', 'edge'],
        notes: 'Main; uplink',
      },
      {
        name: 'edge-02',
        host: '192.0.2.2',
        protocol: 'ssh',
        port: 22,
        username: 'admin',
        folder: 'WAN',
        tags: ['imported'],
      },
    ])
    expect(r.rows[0]!.line).toBe(2)
  })

  it('reads tab-separated text pasted from a spreadsheet, and lists unknown columns', () => {
    const r = parseDeviceList('hostname\tip\track\nsw-01\t10.1.1.1\tR4\n', d)
    expect(r.rows[0]!.row).toMatchObject({ name: 'sw-01', host: '10.1.1.1' })
    expect(r.ignoredColumns).toEqual(['rack'])
  })

  it('keeps quoted commas and doubled quotes, and newlines inside quotes', () => {
    expect(splitCsvLine('a,"b, c","say ""hi""",d', ',')).toEqual(['a', 'b, c', 'say "hi"', 'd'])
    const r = parseDeviceList('name,host,notes\nsw,10.0.0.1,"two\nlines"\nsw2,10.0.0.2,\n', d)
    expect(r.rows).toHaveLength(2)
    expect(r.rows[0]!.row.notes).toBe('two\nlines')
    expect(r.rows[1]!.line).toBe(4)
  })

  it('uses the name as the host when only a host column exists, and flags rows without host', () => {
    const r = parseDeviceList('ip\n10.0.0.1\n', d)
    expect(r.rows[0]!.row).toMatchObject({ name: '10.0.0.1', host: '10.0.0.1' })
    const r2 = parseDeviceList('name,host\nsw-01,\n', d)
    expect(r2.rows[0]!.problem).toBe('no host')
  })

  it('reads a host:port cell, and an explicit port column wins', () => {
    const r = parseDeviceList('name,host,port\na,10.0.0.1:2222,\nb,10.0.0.2:2222,22\n', d)
    expect(r.rows.map((x) => x.row.port)).toEqual([2222, 22])
  })

  it('ignores a byte order mark, Windows line ends and comment lines', () => {
    const r = parseDeviceList('﻿name,host\r\n# old\r\nsw,10.0.0.1\r\n', d)
    expect(r.rows).toHaveLength(1)
    expect(r.rows[0]!.row.host).toBe('10.0.0.1')
  })

  it('reads its own template', () => {
    const r = parseDeviceList(CSV_TEMPLATE, d)
    expect(r.ignoredColumns).toEqual([])
    expect(r.rows[0]!.row).toEqual({
      name: 'core-sw-01',
      host: '10.10.0.2',
      port: 22,
      protocol: 'ssh',
      username: 'netops',
      folder: 'DC/Madrid',
      tags: ['core', 'mad'],
      notes: 'Rack 4',
      credential: 'Lab admin',
      jumpHost: 'bastion',
    })
  })
})

describe('splitHostPort', () => {
  it('leaves a bare IPv6 address alone', () => {
    expect(splitHostPort('2001:db8::1')).toEqual({ host: '2001:db8::1' })
    expect(splitHostPort('[2001:db8::1]')).toEqual({ host: '2001:db8::1', port: undefined })
  })

  it('returns empty for empty input', () => {
    expect(parseDeviceList('  \n# nothing\n', d)).toEqual({
      kind: 'empty',
      rows: [],
      ignoredColumns: [],
    })
  })
})
