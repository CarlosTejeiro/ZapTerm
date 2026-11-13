// Linux command lists (also shown for local shell tabs).
//
// Every command and option below was checked against its man page on
// man7.org — nothing was added from memory: uname(1), hostnamectl(1),
// uptime(1), timedatectl(1), lscpu(1), free(1), df(1), lsblk(8), dmesg(1),
// who(1), w(1), last(1), systemctl(1), journalctl(1), ps(1), kill(1),
// lsof(8), ip(8), ip-address(8), ip-route(8), ip-rule(8), ip-neighbour(8),
// ip-netns(8), ss(8), resolvectl(1), ethtool(8), ping(8), tracepath(8),
// traceroute(8), arping(8), curl(1), watch(1), iptables(8), tcpdump(1).
// Left out: tools without a man7.org page (nft, nmcli, mtr, dig, nc…) and
// commands that reboot or wipe the machine. Commands that need root are
// listed as is: prefix them with sudo when needed.
import type { CommandItem, VendorLists } from './types'

const c = (cmd: string, desc: string, extra: Partial<CommandItem> = {}): CommandItem => ({
  kind: 'command',
  cmd,
  desc,
  ...extra,
})

export const linux: VendorLists = {
  label: 'Linux',
  // A running capture holds the prompt: Ctrl+C is all it takes to stop it.
  stopDebug: [],
  stopKeys: '\x03',
  lists: [
    {
      id: 'system',
      name: 'System',
      icon: 'system',
      color: '#5B4FC4',
      summary: 'Host, memory, disks, kernel messages and logins',
      sections: [
        {
          title: 'Host',
          items: [
            c('uname -a', 'Kernel name, release and architecture'),
            c('hostnamectl', 'Hostname, OS, kernel and hardware'),
            c('uptime', 'Uptime and load average'),
            c('timedatectl', 'Time, time zone and NTP sync'),
            c('lscpu', 'CPU architecture and cores'),
          ],
        },
        {
          title: 'Memory & disks',
          items: [
            c('free -h', 'Memory and swap usage'),
            c('df -h', 'Disk space per filesystem'),
            c('lsblk', 'Block devices and partitions'),
          ],
        },
        {
          title: 'Kernel',
          items: [
            c('dmesg -T', 'Kernel messages with readable times'),
            c('dmesg --level=err,warn', 'Kernel errors and warnings only'),
          ],
        },
        {
          title: 'Logins',
          items: [
            c('who', 'Users logged in'),
            c('w', 'Users logged in and what they run'),
            c('last -n 20', 'Last 20 logins'),
          ],
        },
      ],
    },
    {
      id: 'services',
      name: 'Services',
      icon: 'logs',
      color: '#2E7D6B',
      summary: 'systemd units, journal and processes',
      sections: [
        {
          title: 'Units',
          items: [
            c('systemctl --failed', 'Units that failed'),
            c('systemctl list-units --type=service --state=running', 'Services running'),
            c('systemctl status <unit>', 'State and last log lines of a unit'),
            c('systemctl restart <unit>', 'Restart a unit', { action: true }),
          ],
        },
        {
          title: 'Journal',
          items: [
            c('journalctl -n 100 -u <unit>', 'Last 100 log lines of a unit'),
            c('journalctl -f', 'Follow the journal'),
            c('journalctl -p err -b', 'Errors since this boot'),
            c('journalctl -k', 'Kernel messages from the journal'),
          ],
        },
        {
          title: 'Processes',
          items: [
            c('ps aux', 'Every process'),
            c('ps aux --sort=-pcpu', 'Processes, most CPU first'),
            c('ps aux --sort=-pmem', 'Processes, most memory first'),
            c('kill <pid>', 'Ask a process to stop (SIGTERM)', { action: true }),
          ],
        },
      ],
    },
    {
      id: 'network',
      name: 'Network',
      icon: 'interfaces',
      color: '#2F7A8C',
      summary: 'Addresses, links, neighbors, DNS and sockets',
      sections: [
        {
          title: 'Addresses & links',
          items: [
            c('ip -brief address', 'Interfaces and addresses, one line each'),
            c('ip address show dev <dev>', 'Addresses of one interface'),
            c('ip -brief link', 'Links and their state, one line each'),
            c('ip -s link show dev <dev>', 'Counters of one interface'),
            c('ethtool <dev>', 'Speed, duplex and link of a NIC'),
            c('ethtool -S <dev>', 'NIC statistics'),
          ],
        },
        {
          title: 'Neighbors & DNS',
          items: [
            c('ip neighbour show', 'ARP / NDP table'),
            c('resolvectl status', 'DNS servers per interface'),
          ],
        },
        {
          title: 'Sockets',
          items: [
            c('ss -tulpn', 'Listening TCP/UDP ports and their processes'),
            c('ss -tan', 'All TCP sockets'),
            c('ss -s', 'Socket summary'),
            c('lsof -i -P -n', 'Network files open per process'),
            c('lsof -i :<port>', 'Who uses a port'),
          ],
        },
      ],
    },
    {
      id: 'routing',
      name: 'Routing',
      icon: 'routing',
      color: '#B26A1E',
      summary: 'Routes, policy rules and network namespaces',
      sections: [
        {
          title: 'Routing',
          items: [
            c('ip route', 'IPv4 routing table'),
            c('ip route get <ip>', 'Route used for a destination'),
            c('ip -6 route', 'IPv6 routing table'),
            c('ip rule', 'Policy routing rules'),
            c('ip netns list', 'Network namespaces'),
          ],
        },
      ],
    },
    {
      id: 'diagnostics',
      name: 'Diagnostics',
      icon: 'sdwan',
      color: '#3E6FC2',
      summary: 'Reachability, path and HTTP checks',
      sections: [
        {
          title: 'Reachability',
          items: [
            c('ping -c 4 <host>', 'Four pings to a host'),
            c('tracepath -n <host>', 'Path and MTU to a host (no root)'),
            c('traceroute -n <host>', 'Path to a host'),
            c('arping -c 3 -I <iface> <ip>', 'ARP pings on one interface'),
          ],
        },
        {
          title: 'HTTP & watch',
          items: [
            c('curl -v <url>', 'Request with headers and TLS details'),
            c('curl -I <url>', 'Response headers only'),
            c('watch -n 1 <command>', 'Re-run a command every second'),
          ],
        },
      ],
    },
    {
      id: 'firewall',
      name: 'Firewall',
      icon: 'security',
      color: '#A23B72',
      summary: 'iptables rules and counters',
      sections: [
        {
          title: 'iptables',
          items: [
            c('iptables -L -n -v', 'Filter rules with counters'),
            c('iptables -t nat -L -n -v', 'NAT rules with counters'),
            c('iptables -S', 'Rules as iptables commands'),
          ],
        },
      ],
    },
    {
      id: 'capture',
      name: 'Capture',
      icon: 'debug',
      color: '#B23B3B',
      summary: 'tcpdump — Ctrl+C or "Stop debug" ends it',
      debug: true,
      sections: [
        {
          title: 'tcpdump',
          items: [
            c('tcpdump -D', 'Interfaces tcpdump can capture on'),
            c('tcpdump -nn -i <iface>', 'Capture on an interface, no name lookups'),
            c('tcpdump -nn -i <iface> host <ip>', 'Only traffic to or from a host'),
            c('tcpdump -nn -i <iface> port <port>', 'Only traffic on a port'),
            c('tcpdump -nn -i <iface> -w <file>', 'Save the capture to a pcap file'),
            c('Ctrl+C', 'Stop the capture', { keys: '\x03' }),
          ],
        },
      ],
    },
  ],
}
