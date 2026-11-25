import { describe, expect, it } from 'vitest'
import { asksForSecret, maskSecrets } from './redact'

describe('maskSecrets', () => {
  it('leaves commands without secrets alone', () => {
    for (const c of [
      'get system status',
      'show ip interface brief',
      'show running-config | include password',
      'show snmp community',
      'execute dhcp lease-list',
      'config system admin',
      'ls -la /etc/passwd',
      '',
    ]) {
      expect(maskSecrets(c)).toBe(c)
    }
  })

  it('masks the value after a secret keyword (FortiOS)', () => {
    expect(maskSecrets('set password Sup3rS3cret!')).toBe('set password ******')
    expect(maskSecrets('set password ENC AbCdEf123==')).toBe('set password ENC ******')
    expect(maskSecrets('set psksecret "my shared key"')).toBe('set psksecret ******')
    expect(maskSecrets('set auth-pwd abc123')).toBe('set auth-pwd ******')
    expect(maskSecrets('set passwd s3cret')).toBe('set passwd ******')
    expect(maskSecrets('    set  passphrase   hunter2')).toBe('    set  passphrase   ******')
  })

  it('masks Cisco-style secrets, keeping the encoding type', () => {
    expect(maskSecrets('username admin privilege 15 secret 0 Cisco123')).toBe(
      'username admin privilege 15 secret 0 ******',
    )
    expect(maskSecrets('enable password 7 0822455D0A16')).toBe('enable password 7 ******')
    expect(maskSecrets('snmp-server community Pr1vate RO')).toBe('snmp-server community ****** RO')
    expect(maskSecrets('key-string MyOspfKey')).toBe('key-string ******')
    expect(maskSecrets('tacacs-server key 0 tacKey')).toBe('tacacs-server key 0 ******')
    expect(maskSecrets('neighbor 10.0.0.1 password bgpPass')).toBe(
      'neighbor 10.0.0.1 password ******',
    )
    expect(maskSecrets('crypto isakmp key IkeKey address 192.0.2.1')).toBe(
      'crypto isakmp key ****** address 192.0.2.1',
    )
  })

  it('masks Junos set commands', () => {
    expect(
      maskSecrets('set system login user ops authentication encrypted-password "$6$abc"'),
    ).toBe('set system login user ops authentication encrypted-password ******')
    expect(maskSecrets('set security ike policy p pre-shared-key ascii-text Secr3t')).toBe(
      'set security ike policy p pre-shared-key ascii-text ******',
    )
  })

  it('masks shell forms: assignments, flags, URLs', () => {
    expect(maskSecrets('mysqldump --password=hunter2 db')).toBe('mysqldump --password=****** db')
    expect(maskSecrets('PGPASSWORD=hunter2 psql -h db')).toBe('PGPASSWORD=****** psql -h db')
    expect(maskSecrets('tool --password hunter2 --verbose')).toBe(
      'tool --password ****** --verbose',
    )
    expect(maskSecrets('curl -u admin:hunter2 https://fw/api')).toBe(
      'curl -u admin:****** https://fw/api',
    )
    expect(maskSecrets('sshpass -p hunter2 ssh admin@fw')).toBe('sshpass -p ****** ssh admin@fw')
    expect(maskSecrets('copy ftp://ops:hunter2@10.0.0.9/cfg running-config')).toBe(
      'copy ftp://ops:******@10.0.0.9/cfg running-config',
    )
    expect(maskSecrets('export API_TOKEN=abcdef')).toBe('export API_TOKEN=******')
  })

  it('masks the password argument of a FortiOS backup over FTP', () => {
    expect(maskSecrets('execute backup config ftp fw.conf 10.0.0.9 ops hunter2')).toBe(
      'execute backup config ftp fw.conf 10.0.0.9 ops ******',
    )
    expect(maskSecrets('execute backup config ftp fw.conf 10.0.0.9')).toBe(
      'execute backup config ftp fw.conf 10.0.0.9',
    )
  })

  it('does not take a pipe for a value', () => {
    expect(maskSecrets('show run | include secret | count')).toBe(
      'show run | include secret | count',
    )
  })
})

describe('asksForSecret', () => {
  it('spots a prompt asking for a password', () => {
    for (const p of [
      'Password: ',
      'password:',
      'Enter new password: ',
      'New Password:',
      "admin@10.0.0.1's password: ",
      'Enter passphrase for key: ',
      'Enable secret: ',
      '[sudo] password for carlos: ',
    ]) {
      expect(asksForSecret(p), p).toBe(true)
    }
  })
  it('does not take an ordinary prompt or output for one', () => {
    for (const p of ['fw1 # ', 'core-rtr-01#', 'Private Encryption: Disable', 'Username: ', '']) {
      expect(asksForSecret(p), p).toBe(false)
    }
  })
})
