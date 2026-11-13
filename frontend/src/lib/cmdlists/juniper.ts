// Juniper Junos OS command lists.
//
// Every command below was checked against its page in the Junos OS CLI
// Reference (Juniper documentation, now served from networkingdocs.hpe.com):
// its syntax line, options or a sample — nothing was added from memory.
// Left out: commands whose page was not found there (EX/QFX ethernet-switching
// and VLAN tables, "show interfaces descriptions"…) and commands that reboot
// or wipe the device.
import type { CommandItem, VendorLists } from './types'

const c = (cmd: string, desc: string, extra: Partial<CommandItem> = {}): CommandItem => ({
  kind: 'command',
  cmd,
  desc,
  ...extra,
})

export const juniper: VendorLists = {
  label: 'Juniper Junos',
  // Ctrl+C ends `monitor traffic`; `monitor stop` ends every `monitor start`.
  stopDebug: ['monitor stop'],
  stopKeys: '\x03',
  lists: [
    {
      id: 'system',
      name: 'System',
      icon: 'system',
      color: '#5B4FC4',
      summary: 'Version, chassis, alarms, resources, configuration and logs',
      sections: [
        {
          title: 'Status',
          items: [
            c('show version', 'Junos version and model'),
            c('show system uptime', 'Uptime and time of last configuration change'),
            c('show system users', 'Users logged in'),
            c('show ntp associations', 'NTP peers'),
            c('show system storage', 'Disk usage per filesystem'),
            c('show system processes extensive', 'Processes, CPU and memory'),
          ],
        },
        {
          title: 'Chassis',
          items: [
            c('show chassis hardware', 'Hardware inventory and serial numbers'),
            c('show chassis alarms', 'Chassis alarms'),
            c('show system alarms', 'System alarms'),
            c('show chassis environment', 'Temperature, fans and power'),
            c('show chassis routing-engine', 'Routing Engine CPU, memory and uptime'),
            c('show chassis fpc', 'FPC state, CPU and memory'),
            c('show virtual-chassis', 'Virtual Chassis members and roles'),
          ],
        },
        {
          title: 'Configuration & logs',
          items: [
            c('show configuration', 'Committed configuration'),
            c('show system commit', 'Commit history'),
            c('show log messages', 'The messages log'),
          ],
        },
      ],
    },
    {
      id: 'interfaces',
      name: 'Interfaces',
      icon: 'interfaces',
      color: '#2F7A8C',
      summary: 'Interfaces, optics, LACP, LLDP and ARP',
      sections: [
        {
          title: 'Interfaces',
          items: [
            c('show interfaces terse', 'Interfaces, addresses and state in one line each'),
            c(
              'show interfaces statistics <interface> extensive',
              'Counters and errors of one interface',
            ),
            c('show interfaces media', 'Media details (speed, duplex, autonegotiation)'),
            c('show interfaces diagnostics optics <interface>', 'Optical transceiver readings'),
            c('monitor interface <interface>', 'Live counters of one interface'),
          ],
        },
        {
          title: 'LACP, LLDP & ARP',
          items: [
            c('show lacp interfaces', 'LACP state of aggregated interfaces'),
            c('show lldp neighbors', 'LLDP neighbors'),
            c('show arp no-resolve', 'ARP table without name lookups'),
            c('show ipv6 neighbors', 'IPv6 neighbor table'),
          ],
        },
      ],
    },
    {
      id: 'routing',
      name: 'Routing',
      icon: 'routing',
      color: '#B26A1E',
      summary: 'Routing tables, forwarding, instances, BFD and VRRP',
      sections: [
        {
          title: 'Routing tables',
          items: [
            c('show route', 'Active routes of every table'),
            c('show route <ip>', 'Route lookup for a destination'),
            c('show route summary', 'Routes per table and protocol'),
            c('show route table <table>', 'Routes of one table (e.g. inet.0, VRF.inet.0)'),
            c('show route protocol <protocol>', 'Routes learned by one protocol'),
            c('show route forwarding-table destination <ip>', 'Forwarding entry for a destination'),
            c('show route instance', 'Routing instances'),
          ],
        },
        {
          title: 'BFD & VRRP',
          items: [c('show bfd session', 'BFD sessions'), c('show vrrp summary', 'VRRP groups')],
        },
      ],
    },
    {
      id: 'bgp',
      name: 'BGP',
      icon: 'bgp',
      color: '#3E6FC2',
      summary: 'Peers, received and advertised routes',
      sections: [
        {
          title: 'Peers',
          items: [
            c('show bgp summary', 'BGP neighbors and their state'),
            c('show bgp neighbor', 'Every BGP neighbor in detail'),
            c('show bgp neighbor <peer-ip>', 'One BGP neighbor in detail'),
          ],
        },
        {
          title: 'Routes of a peer',
          items: [
            c('show route receive-protocol bgp <peer-ip>', 'Routes received from the peer'),
            c('show route advertising-protocol bgp <peer-ip>', 'Routes advertised to the peer'),
          ],
        },
        {
          title: 'Actions',
          items: [
            c('clear bgp neighbor <peer-ip> soft', 'Soft reset of one neighbor', { action: true }),
          ],
        },
      ],
    },
    {
      id: 'igp',
      name: 'OSPF & IS-IS',
      icon: 'ospf',
      color: '#6B53B8',
      summary: 'Neighbors, interfaces and link-state databases',
      sections: [
        {
          title: 'OSPF',
          items: [
            c('show ospf neighbor', 'OSPF neighbors'),
            c('show ospf interface', 'Interfaces running OSPF'),
            c('show ospf database', 'OSPF link-state database'),
            c('show ospf3 neighbor', 'OSPFv3 neighbors (IPv6)'),
          ],
        },
        {
          title: 'IS-IS',
          items: [
            c('show isis adjacency', 'IS-IS adjacencies'),
            c('show isis database', 'IS-IS link-state database'),
          ],
        },
      ],
    },
    {
      id: 'mpls',
      name: 'MPLS',
      icon: 'vpn',
      color: '#1F7A55',
      summary: 'LSPs, LDP and RSVP',
      sections: [
        {
          title: 'MPLS',
          items: [
            c('show mpls lsp', 'MPLS LSPs and their state'),
            c('show ldp neighbor', 'LDP neighbors'),
            c('show ldp session', 'LDP sessions'),
            c('show rsvp session', 'RSVP sessions'),
          ],
        },
      ],
    },
    {
      id: 'security',
      name: 'Security',
      icon: 'security',
      color: '#A23B72',
      summary: 'SRX zones, policies, sessions, IPsec and firewall filters',
      sections: [
        {
          title: 'SRX',
          items: [
            c('show security zones', 'Security zones and their interfaces'),
            c('show security policies hit-count', 'Policy hit counters'),
            c('show security flow session summary', 'Session counts'),
            c('show security flow session destination-prefix <ip>', 'Sessions to a destination'),
            c('show security ike security-associations', 'IKE security associations'),
            c('show security ipsec security-associations', 'IPsec security associations'),
          ],
        },
        {
          title: 'Firewall filters',
          items: [c('show firewall', 'Firewall filter counters')],
        },
      ],
    },
    {
      id: 'monitor',
      name: 'Monitor',
      icon: 'debug',
      color: '#B23B3B',
      summary: 'Live capture and log follow — "Stop debug" ends them',
      debug: true,
      sections: [
        {
          title: 'Live output',
          items: [
            c('monitor traffic interface <interface> no-resolve', 'Packet headers on an interface'),
            c('monitor start messages', 'Follow the messages log in this session'),
            c('monitor stop', 'Stop following logs'),
            c('Ctrl+C', 'Stop monitor traffic', { keys: '\x03' }),
          ],
        },
      ],
    },
  ],
}
