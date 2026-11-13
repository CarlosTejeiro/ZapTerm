// Cisco IOS / IOS-XE command lists.
//
// Every command below was checked against Cisco's documentation — nothing
// was added from memory:
//  • Command Reference, Cisco IOS XE 17.15.x (Catalyst 9300): Interface and
//    Hardware, IP Addressing Services, Layer 2/3, Network Management,
//    Security, Stack Manager and High Availability, System Management,
//    Unicast Routing and VLAN chapters
//  • Cisco IOS IP Routing: BGP Command Reference (show ip bgp … through Z)
//  • IP Routing Configuration Guide, Cisco IOS XE 17.x: BGP 4
//  • Cisco IOS Security Command Reference (A to C; S to Z: show crypto …)
//  • Cisco IOS First Hop Redundancy Protocols Command Reference
//  • Embedded Packet Capture: Command Reference, Configuration Guide IOS XE
//    17, Catalyst 9000 Management Configuration Guide
//  • Security and VPN Configuration Guide IOS XE 17.x: Crypto Conditional
//    Debug Support
//  • "Understand Important Information on Debug Commands" (undebug all,
//    terminal monitor)
//  • IP Addressing Services Configuration Guide 17.15 (DHCP snooping)
// Left out: commands that reload or erase the device, and ones not found in
// those documents (e.g. "show processes cpu sorted", "show vrf").
import type { CommandItem, VendorLists } from './types'

const c = (cmd: string, desc: string, extra: Partial<CommandItem> = {}): CommandItem => ({
  kind: 'command',
  cmd,
  desc,
  ...extra,
})

export const ciscoIos: VendorLists = {
  label: 'Cisco IOS / IOS-XE',
  stopDebug: ['undebug all', 'debug crypto condition reset'],
  lists: [
    {
      id: 'system',
      name: 'System',
      icon: 'system',
      color: '#5B4FC4',
      summary: 'Version, resources, environment, licenses and logs',
      sections: [
        {
          title: 'Status',
          items: [
            c('show version', 'Software version, uptime and hardware'),
            c('show clock', 'Current time'),
            c('show inventory', 'Hardware inventory and serial numbers'),
            c('show running-config', 'Running configuration'),
            c('show logging', 'Logging settings and buffered messages'),
            c('show users', 'Users logged in'),
          ],
        },
        {
          title: 'Resources',
          items: [
            c('show processes cpu platform sorted', 'CPU per process, sorted (platform)'),
            c('show processes memory sorted', 'Memory per process, sorted'),
            c('show platform resources', 'CPU, memory and resource health'),
            c('show environment all', 'Fans, temperature and power supplies'),
          ],
        },
        {
          title: 'Redundancy & stack',
          items: [
            c('show redundancy', 'Redundancy state of the supervisors'),
            c('show switch', 'Stack members and roles'),
          ],
        },
        {
          title: 'Licenses',
          items: [
            c('show license summary', 'Licenses in use and their status'),
            c('show license usage', 'License usage details'),
          ],
        },
      ],
    },
    {
      id: 'interfaces',
      name: 'Interfaces',
      icon: 'interfaces',
      color: '#2F7A8C',
      summary: 'Interfaces, neighbors, ARP and MAC table',
      sections: [
        {
          title: 'Interfaces',
          items: [
            c('show ip interface brief', 'Interfaces, IP and status in one line each'),
            c('show interfaces status', 'Port status, VLAN, duplex and speed'),
            c('show interfaces <interface>', 'Counters and details of one interface'),
            c('show interfaces transceiver', 'Optical transceiver readings'),
            c('show etherchannel summary', 'Port-channels and their members'),
          ],
        },
        {
          title: 'Neighbors',
          items: [
            c('show cdp neighbors', 'CDP neighbors'),
            c('show cdp neighbors detail', 'CDP neighbors in detail (IP, platform)'),
            c('show lldp neighbors', 'LLDP neighbors'),
            c('show lldp neighbors detail', 'LLDP neighbors in detail'),
          ],
        },
        {
          title: 'ARP & MAC',
          items: [
            c('show ip arp', 'ARP table'),
            c('show mac address-table', 'MAC address table'),
            c('show mac address-table address <mac>', 'Where a MAC address is learned'),
          ],
        },
      ],
    },
    {
      id: 'switching',
      name: 'Switching',
      icon: 'fortilink',
      color: '#5C7A2E',
      summary: 'VLANs, trunks and spanning tree',
      sections: [
        {
          title: 'Layer 2',
          items: [
            c('show vlan brief', 'VLANs and their ports'),
            c('show interfaces trunk', 'Active trunk ports and allowed VLANs'),
            c('show spanning-tree', 'Spanning tree per VLAN'),
            c('show spanning-tree summary', 'Spanning tree summary'),
          ],
        },
      ],
    },
    {
      id: 'routing',
      name: 'Routing',
      icon: 'routing',
      color: '#B26A1E',
      summary: 'Routing table, CEF, VRFs, FHRP and BFD',
      sections: [
        {
          title: 'Routing table',
          items: [
            c('show ip route', 'IPv4 routing table'),
            c('show ip route <ip>', 'Route lookup for a destination'),
            c('show ip route summary', 'Routes per protocol'),
            c('show ip route vrf <vrf>', 'Routing table of a VRF'),
            c('show ip cef <ip>', 'CEF forwarding entry for a destination'),
            c('show ip protocols', 'Routing protocols running'),
            c('show ipv6 route', 'IPv6 routing table'),
          ],
        },
        {
          title: 'Tracking, FHRP & BFD',
          items: [
            c('show track', 'Tracked objects'),
            c('show standby brief', 'HSRP groups, one line each'),
            c('show vrrp brief', 'VRRP groups, one line each'),
            c('show bfd neighbors', 'BFD sessions'),
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
            c('show ip bgp summary', 'BGP neighbors and their state'),
            c('show ip bgp neighbors', 'Every BGP neighbor in detail'),
          ],
        },
        {
          title: 'Routes of a peer',
          items: [
            c('show ip bgp neighbors <peer-ip> received-routes', 'Routes received from the peer'),
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
      summary: 'Neighbors, interfaces and LSDB',
      sections: [
        {
          title: 'OSPF',
          items: [
            c('show ip ospf neighbor', 'OSPF neighbors'),
            c('show ip ospf neighbor detail', 'OSPF neighbors in detail'),
            c('show ip ospf interface brief', 'Interfaces running OSPF'),
            c('show ip ospf database', 'OSPF link-state database'),
            c('show ip ospf border-routers', 'ABRs and ASBRs'),
            c('show ipv6 ospf neighbor', 'OSPFv3 neighbors (IPv6)'),
          ],
        },
      ],
    },
    {
      id: 'vpn',
      name: 'IPsec',
      icon: 'vpn',
      color: '#1F7A55',
      summary: 'Crypto sessions, IKE and IPsec SAs',
      sections: [
        {
          title: 'Sessions & SAs',
          items: [
            c('show crypto session', 'Crypto sessions, one per peer'),
            c('show crypto ikev2 sa', 'IKEv2 security associations'),
            c('show crypto ikev2 sa detail', 'IKEv2 SAs in detail'),
            c('show crypto isakmp sa', 'IKEv1 (ISAKMP) security associations'),
            c('show crypto ipsec sa', 'IPsec SAs and encaps/decaps counters'),
            c('show crypto ipsec sa peer <ip>', 'IPsec SAs of one peer'),
            c('show crypto map', 'Crypto maps'),
          ],
        },
        {
          title: 'Actions',
          items: [
            c('clear crypto session remote <ip>', 'Clear the IKE and IPsec SAs of one peer', {
              action: true,
            }),
            c('clear crypto sa peer <ip>', 'Clear the IPsec SAs of one peer', { action: true }),
          ],
        },
      ],
    },
    {
      id: 'security',
      name: 'Security',
      icon: 'security',
      color: '#A23B72',
      summary: 'ACLs, AAA, 802.1X, port security, DHCP snooping and NAT',
      sections: [
        {
          title: 'Access lists & NAT',
          items: [
            c('show access-lists', 'Access lists and their hit counters'),
            c('show ip access-lists', 'IPv4 access lists'),
            c('show ip nat translations', 'NAT translations'),
            c('show ip nat statistics', 'NAT statistics'),
          ],
        },
        {
          title: 'Access control',
          items: [
            c('show aaa servers', 'AAA servers and their state'),
            c('show authentication sessions', 'Authentication sessions per port'),
            c('show dot1x all', '802.1X state'),
            c('show port-security', 'Port security per interface'),
            c('show ip dhcp snooping binding', 'DHCP snooping binding table'),
          ],
        },
      ],
    },
    {
      id: 'debug',
      name: 'Debug',
      icon: 'debug',
      color: '#B23B3B',
      summary: 'Packet capture and debugs — stop them when done',
      debug: true,
      sections: [
        {
          title: 'Packet capture (EPC)',
          items: [
            {
              kind: 'recipe',
              name: 'Capture on an interface',
              desc: 'Embedded Packet Capture "ZAPTERM", both directions, all IPv4',
              fields: [{ key: 'iface', label: 'Interface', placeholder: 'GigabitEthernet1/0/1' }],
              lines: [
                'monitor capture ZAPTERM interface {{iface}} both',
                'monitor capture ZAPTERM match ipv4 any any',
                'monitor capture ZAPTERM start',
              ],
            },
            c('monitor capture ZAPTERM stop', 'Stop the capture'),
            c('show monitor capture ZAPTERM buffer dump', 'Show the captured packets'),
            c('monitor capture ZAPTERM export <location>', 'Export the capture as a PCAP file'),
            c('no monitor capture ZAPTERM', 'Delete the capture'),
          ],
        },
        {
          title: 'Debugs',
          items: [
            {
              kind: 'recipe',
              name: 'Crypto debug of a peer',
              desc: 'IKE and IPsec debugs limited to one peer (conditional debug)',
              fields: [{ key: 'peer', label: 'Peer IP', placeholder: '203.0.113.2' }],
              lines: [
                'terminal monitor',
                'debug crypto condition peer ipv4 {{peer}}',
                'debug crypto isakmp',
                'debug crypto ipsec',
              ],
            },
            {
              kind: 'recipe',
              name: 'BGP updates of a peer',
              desc: 'BGP update messages exchanged with one neighbor',
              fields: [{ key: 'peer', label: 'Neighbor IP', placeholder: '192.0.2.1' }],
              lines: ['terminal monitor', 'debug ip bgp {{peer}} updates'],
            },
            c('terminal monitor', 'Show debug output in this (SSH/Telnet) session'),
            c('terminal no monitor', 'Stop showing debug output here (debugs keep running)'),
          ],
        },
      ],
    },
  ],
}
