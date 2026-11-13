// Cisco IOS XR command lists.
//
// Every command below was checked against Cisco's IOS XR command references
// (command syntax or a documented example) — nothing was added from memory:
//  • Cisco 8000: BGP, Routing, MPLS and System Setup command references
//  • ASR 9000: General Administration, System Setup, Interface and Hardware
//    Component, IP Addresses and Services, Routing, System Security, System
//    Management, System Monitoring, Advanced System and L2VPN command
//    references
//  • NCS 5500: Interfaces, System Monitoring, System Management and System
//    Security command references
// Left out: debug/undebug commands (not found in those references), commands
// that reload or erase the device, and ones not found there.
import type { CommandItem, VendorLists } from './types'

const c = (cmd: string, desc: string, extra: Partial<CommandItem> = {}): CommandItem => ({
  kind: 'command',
  cmd,
  desc,
  ...extra,
})

export const ciscoIosXr: VendorLists = {
  label: 'Cisco IOS XR',
  stopDebug: [],
  lists: [
    {
      id: 'system',
      name: 'System',
      icon: 'system',
      color: '#5B4FC4',
      summary: 'Version, nodes, resources, software, commits and licenses',
      sections: [
        {
          title: 'Status',
          items: [
            c('show version', 'Software version, uptime and hardware'),
            c('show clock', 'Current time'),
            c('show platform', 'Nodes (RPs, line cards) and their state'),
            c('show inventory', 'Hardware inventory and serial numbers'),
            c('show running-config', 'Running configuration'),
            c('show logging', 'Logging settings and buffered messages'),
            c('show alarms brief', 'Active and historical alarms'),
            c('show users', 'Users logged in'),
            c('show ntp associations', 'NTP peers'),
          ],
        },
        {
          title: 'Resources',
          items: [
            c('show processes cpu', 'CPU usage per process'),
            c('show memory summary', 'Memory summary'),
            c('show environment', 'Temperature, voltage, fans and power'),
            c('show watchdog memory-state', 'Memory state (normal, minor, severe…)'),
            c('show context', 'Core dumps of crashed processes'),
          ],
        },
        {
          title: 'Redundancy & software',
          items: [
            c('show redundancy', 'Redundancy state of the route processors'),
            c('show redundancy summary', 'Redundancy summary'),
            c('show install active summary', 'Active software packages'),
            c('show install committed summary', 'Committed software packages'),
            c('show hw-module fpd', 'FPD (firmware) versions per card'),
          ],
        },
        {
          title: 'Configuration commits',
          items: [
            c('show configuration commit list', 'Last configuration commits'),
            c('show configuration commit changes <commit-id>', 'What a commit changed'),
          ],
        },
        {
          title: 'Licenses',
          items: [
            c('show license status', 'Smart Licensing status'),
            c('show license summary', 'Licenses in use and their status'),
          ],
        },
      ],
    },
    {
      id: 'interfaces',
      name: 'Interfaces',
      icon: 'interfaces',
      color: '#2F7A8C',
      summary: 'Interfaces, optics, bundles, neighbors and ARP',
      sections: [
        {
          title: 'Interfaces',
          items: [
            c('show ipv4 interface brief', 'Interfaces, IPv4 and status in one line each'),
            c('show ipv6 interface brief', 'Interfaces, IPv6 and status in one line each'),
            c('show interfaces brief', 'Interfaces, state, encapsulation and speed'),
            c('show interfaces description', 'Interfaces with their descriptions'),
            c('show interfaces summary', 'Interface counts per type and state'),
            c('show interfaces <interface>', 'Counters and details of one interface'),
            c('monitor interface <interface>', 'Live traffic rates of one interface'),
            c('show controllers optics <port>', 'Optics readings of one port (e.g. 0/0/0/0)'),
          ],
        },
        {
          title: 'Bundles',
          items: [
            c('show bundle', 'Bundles and their members'),
            c('show bundle brief', 'Bundles, one line each'),
            c('show bundle Bundle-Ether <id>', 'One bundle and its members'),
          ],
        },
        {
          title: 'Neighbors',
          items: [
            c('show lldp neighbors', 'LLDP neighbors'),
            c('show lldp neighbors detail', 'LLDP neighbors in detail'),
            c('show cdp neighbors', 'CDP neighbors'),
            c('show cdp neighbors detail', 'CDP neighbors in detail (IP, platform)'),
          ],
        },
        {
          title: 'ARP & ND',
          items: [c('show arp', 'ARP table'), c('show ipv6 neighbors', 'IPv6 neighbor table')],
        },
      ],
    },
    {
      id: 'routing',
      name: 'Routing',
      icon: 'routing',
      color: '#B26A1E',
      summary: 'RIB, CEF, VRFs, BFD and FHRP',
      sections: [
        {
          title: 'Routing table',
          items: [
            c('show route', 'IPv4 routing table'),
            c('show route <ip>', 'Route lookup for a destination'),
            c('show route summary', 'Routes per protocol'),
            c('show route vrf <vrf>', 'Routing table of a VRF'),
            c('show route ipv6', 'IPv6 routing table'),
            c('show route bgp', 'BGP routes in the RIB'),
            c('show route static', 'Static routes'),
            c('show route connected', 'Connected routes'),
            c('show cef <ip>', 'CEF forwarding entry for a destination'),
            c('show vrf all', 'VRFs and their route distinguishers'),
            c('show protocols', 'Routing protocols running'),
          ],
        },
        {
          title: 'BFD & FHRP',
          items: [
            c('show bfd session', 'BFD sessions'),
            c('show hsrp brief', 'HSRP groups, one line each'),
            c('show vrrp brief', 'VRRP groups, one line each'),
          ],
        },
      ],
    },
    {
      id: 'bgp',
      name: 'BGP',
      icon: 'bgp',
      color: '#3E6FC2',
      summary: 'Peers per address family, received and advertised routes',
      sections: [
        {
          title: 'Peers',
          items: [
            c('show bgp summary', 'BGP neighbors and their state'),
            c('show bgp ipv4 unicast summary', 'IPv4 unicast neighbors'),
            c('show bgp vpnv4 unicast summary', 'VPNv4 neighbors'),
            c('show bgp vrf all summary', 'Neighbors of every VRF'),
            c('show bgp vrf <vrf> summary', 'Neighbors of one VRF'),
            c('show bgp neighbors', 'Every BGP neighbor in detail'),
            c('show bgp neighbors <peer-ip>', 'One BGP neighbor in detail'),
          ],
        },
        {
          title: 'Routes of a peer',
          items: [
            c(
              'show bgp neighbors <peer-ip> received routes',
              'Routes received from the peer (needs soft-reconfiguration inbound)',
            ),
            c('show bgp neighbors <peer-ip> advertised-routes', 'Routes advertised to the peer'),
            c('show bgp neighbors <peer-ip> routes', 'Routes learned from the peer'),
            c('show bgp <prefix>', 'BGP entry for a prefix'),
          ],
        },
        {
          title: 'Actions',
          items: [
            c('clear bgp ipv4 unicast <peer-ip> soft', 'Soft reset of one IPv4 neighbor', {
              action: true,
            }),
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
            c('show ospf neighbor detail', 'OSPF neighbors in detail'),
            c('show ospf interface', 'Interfaces running OSPF'),
            c('show ospf database', 'OSPF link-state database'),
            c('show ospfv3 neighbor', 'OSPFv3 neighbors (IPv6)'),
          ],
        },
        {
          title: 'IS-IS',
          items: [
            c('show isis adjacency', 'IS-IS adjacencies'),
            c('show isis neighbors', 'IS-IS neighbors'),
            c('show isis interface brief', 'Interfaces running IS-IS'),
            c('show isis database', 'IS-IS link-state database'),
          ],
        },
      ],
    },
    {
      id: 'mpls',
      name: 'MPLS & L2VPN',
      icon: 'vpn',
      color: '#1F7A55',
      summary: 'LDP, forwarding, TE tunnels, xconnects and bridge domains',
      sections: [
        {
          title: 'MPLS',
          items: [
            c('show mpls ldp neighbor brief', 'LDP neighbors, one line each'),
            c('show mpls ldp neighbor', 'LDP neighbors in detail'),
            c('show mpls interfaces', 'Interfaces with MPLS enabled'),
            c('show mpls forwarding', 'Label forwarding table'),
            c('show mpls traffic-eng tunnels brief', 'TE tunnels, one line each'),
          ],
        },
        {
          title: 'L2VPN',
          items: [
            c('show l2vpn xconnect', 'Point-to-point xconnects and their state'),
            c('show l2vpn xconnect detail', 'Xconnects in detail'),
            c('show l2vpn bridge-domain brief', 'Bridge domains, one line each'),
          ],
        },
      ],
    },
    {
      id: 'security',
      name: 'Security',
      icon: 'security',
      color: '#A23B72',
      summary: 'Access lists, SSH, TACACS+ and RADIUS',
      sections: [
        {
          title: 'Access lists',
          items: [
            c('show access-lists', 'Access lists and their entries'),
            c('show access-lists ipv4', 'IPv4 access lists'),
          ],
        },
        {
          title: 'Management access',
          items: [
            c('show ssh', 'SSH sessions in and out'),
            c('show ssh session details', 'SSH sessions with their algorithms'),
            c('show tacacs', 'TACACS+ servers and their counters'),
            c('show radius', 'RADIUS servers and their counters'),
          ],
        },
      ],
    },
    {
      id: 'monitor',
      name: 'Monitor',
      icon: 'debug',
      color: '#B23B3B',
      summary: 'Live output in this session',
      sections: [
        {
          title: 'Live output',
          items: [
            c('terminal monitor', 'Show logging output in this (SSH/Telnet) session'),
            c('terminal monitor disable', 'Stop showing logging output here'),
            c('monitor interface <interface>', 'Live traffic rates of one interface'),
          ],
        },
      ],
    },
  ],
}
