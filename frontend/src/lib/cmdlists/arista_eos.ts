// Arista EOS command lists.
//
// Every command below was checked against the Arista EOS 4.37.0F User Manual
// on arista.com (its command syntax or a documented example) — nothing was
// added from memory. Chapters: Command-Line Interface, Switch Administration
// Commands, Environment Commands, Ethernet Ports, Port Channels and LACP,
// Multi-Chassis Link Aggregation, Spanning Tree Protocol, LLDP, VLANs, IPv4,
// ACLs and Route Maps, VRRP and VARP, BFD, OSPFv2, BGP and VXLAN Commands.
// Left out: commands not found there (MAC address table, inventory,
// processes, debugs, tcpdump…) and commands that reload or erase the switch.
import type { CommandItem, VendorLists } from './types'

const c = (cmd: string, desc: string, extra: Partial<CommandItem> = {}): CommandItem => ({
  kind: 'command',
  cmd,
  desc,
  ...extra,
})

export const aristaEos: VendorLists = {
  label: 'Arista EOS',
  stopDebug: [],
  lists: [
    {
      id: 'system',
      name: 'System',
      icon: 'system',
      color: '#5B4FC4',
      summary: 'Version, time, configuration, logs and environment',
      sections: [
        {
          title: 'Status',
          items: [
            c('show version', 'Model, serial number, EOS version and uptime'),
            c('show clock', 'Current time'),
            c('show hostname', 'Hostname and FQDN'),
            c('show running-config', 'Running configuration'),
            c('show logging', 'Logging settings and buffered messages'),
            c('show ntp status', 'NTP synchronization'),
            c('show ntp associations', 'NTP peers'),
          ],
        },
        {
          title: 'Environment',
          items: [
            c('show system environment all', 'Temperature, cooling and power'),
            c('show system environment cooling', 'Fans'),
            c('show environment temperature', 'Temperature sensors'),
            c('show environment power', 'Power supplies'),
          ],
        },
      ],
    },
    {
      id: 'interfaces',
      name: 'Interfaces',
      icon: 'interfaces',
      color: '#2F7A8C',
      summary: 'Ports, counters, optics, port-channels, LLDP and ARP',
      sections: [
        {
          title: 'Interfaces',
          items: [
            c('show interfaces status', 'Port status, VLAN, duplex and speed'),
            c('show interfaces status errdisabled', 'Error-disabled ports'),
            c('show ip interface brief', 'Interfaces, IP and status in one line each'),
            c('show interfaces <interface>', 'Counters and details of one interface'),
            c('show interfaces counters', 'Packet counters per port'),
            c('show interfaces counters errors', 'Error counters per port'),
            c('show interfaces counters rates', 'Traffic rates per port'),
            c('show interfaces transceiver', 'Optical transceiver readings'),
          ],
        },
        {
          title: 'Port-channels',
          items: [
            c('show port-channel', 'Port-channels and their members'),
            c('show port-channel dense', 'Port-channels, compact view'),
            c('show lacp interface', 'LACP state per interface'),
            c('show lacp peer', 'LACP partners'),
          ],
        },
        {
          title: 'Neighbors & ARP',
          items: [
            c('show lldp neighbors', 'LLDP neighbors'),
            c('show lldp neighbors detail', 'LLDP neighbors in detail'),
            c('show lldp local-info', 'What this switch advertises over LLDP'),
            c('show ip arp', 'ARP table'),
          ],
        },
      ],
    },
    {
      id: 'switching',
      name: 'Switching',
      icon: 'fortilink',
      color: '#5C7A2E',
      summary: 'VLANs, trunks, spanning tree and MLAG',
      sections: [
        {
          title: 'Layer 2',
          items: [
            c('show vlan', 'VLANs and their ports'),
            c('show interfaces trunk', 'Trunk ports and allowed VLANs'),
            c('show interfaces switchport', 'Switchport mode and VLANs per port'),
            c('show spanning-tree', 'Spanning tree per instance'),
            c('show spanning-tree blockedports', 'Ports blocked by spanning tree'),
            c('show spanning-tree root', 'Root bridge per instance'),
            c('show spanning-tree topology status', 'Forwarding state per port and instance'),
          ],
        },
        {
          title: 'MLAG',
          items: [
            c('show mlag', 'MLAG state and peer'),
            c('show mlag detail', 'MLAG state in detail'),
            c('show mlag config-sanity', 'Config differences with the MLAG peer'),
            c('show mlag interfaces', 'MLAG interfaces and their state'),
            c('show mlag interfaces detail', 'MLAG interfaces in detail'),
          ],
        },
      ],
    },
    {
      id: 'routing',
      name: 'Routing',
      icon: 'routing',
      color: '#B26A1E',
      summary: 'Routing table, VRFs, BFD, VRRP and VARP',
      sections: [
        {
          title: 'Routing table',
          items: [
            c('show ip route', 'IPv4 routing table'),
            c('show ip route <ip>', 'Route lookup for a destination'),
            c('show ip route summary', 'Routes per protocol'),
            c('show ip route vrf <vrf>', 'Routing table of a VRF'),
            c('show ip route bgp', 'BGP routes'),
            c('show ip route connected', 'Connected routes'),
            c('show ip route static', 'Static routes'),
            c('show ip route host', 'Host routes'),
            c('show vrf', 'VRFs and their interfaces'),
          ],
        },
        {
          title: 'BFD, VRRP & VARP',
          items: [
            c('show bfd peers', 'BFD sessions'),
            c('show bfd peers detail', 'BFD sessions in detail'),
            c('show vrrp brief', 'VRRP groups, one line each'),
            c('show vrrp', 'VRRP groups in detail'),
            c('show ip virtual-router', 'VARP virtual router addresses'),
          ],
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
            c('show ip bgp summary', 'IPv4 BGP neighbors and their state'),
            c('show ipv6 bgp summary', 'IPv6 BGP neighbors and their state'),
            c('show ip bgp neighbors', 'Every BGP neighbor in detail'),
            c('show ip bgp neighbors <peer-ip>', 'One BGP neighbor in detail'),
          ],
        },
        {
          title: 'Routes of a peer',
          items: [
            c(
              'show ip bgp neighbors <peer-ip> received-routes',
              'Routes received from the peer (needs rib-in pre-policy retain)',
            ),
            c('show ip bgp neighbors <peer-ip> advertised-routes', 'Routes advertised to the peer'),
            c('show ip bgp neighbors <peer-ip> routes', 'Routes learned from the peer'),
            c('show ip bgp <prefix>', 'BGP entry for a prefix'),
          ],
        },
        {
          title: 'Actions',
          items: [c('clear ip bgp <peer-ip> soft', 'Soft reset of one neighbor', { action: true })],
        },
      ],
    },
    {
      id: 'ospf',
      name: 'OSPF',
      icon: 'ospf',
      color: '#6B53B8',
      summary: 'OSPFv2 process, neighbors, interfaces and LSDB',
      sections: [
        {
          title: 'OSPFv2',
          items: [
            c('show ip ospf', 'OSPF process'),
            c('show ip ospf neighbor', 'OSPF neighbors'),
            c('show ip ospf interface brief', 'Interfaces running OSPF'),
            c('show ip ospf interface', 'OSPF interfaces in detail'),
            c('show ip ospf database database-summary', 'LSA counts per type'),
            c('show ip ospf border-routers', 'ABRs and ASBRs'),
          ],
        },
      ],
    },
    {
      id: 'vxlan',
      name: 'VXLAN',
      icon: 'vpn',
      color: '#1F7A55',
      summary: 'VTEPs, VNIs and the VXLAN address table',
      sections: [
        {
          title: 'VXLAN',
          items: [
            c('show interfaces VXLAN 1', 'The VXLAN interface: source, VNIs and flood list'),
            c('show vxlan vtep', 'Remote VTEPs'),
            c('show vxlan vni', 'VNI to VLAN mapping'),
            c('show vxlan address-table', 'MAC addresses learned over VXLAN'),
          ],
        },
      ],
    },
    {
      id: 'security',
      name: 'Security',
      icon: 'security',
      color: '#A23B72',
      summary: 'Access lists, route maps and prefix lists',
      sections: [
        {
          title: 'Filters',
          items: [
            c('show ip access-lists', 'IPv4 access lists and their counters'),
            c('show ip access-lists summary', 'IPv4 access lists, summary'),
            c('show ipv6 access-lists', 'IPv6 access lists'),
            c('show route-map', 'Route maps'),
            c('show ip prefix-list', 'Prefix lists'),
          ],
        },
      ],
    },
  ],
}
