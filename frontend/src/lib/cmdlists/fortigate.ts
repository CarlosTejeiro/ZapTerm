// FortiGate (FortiOS 7.6) command lists.
//
// Every command below was checked against Fortinet's FortiOS 7.6
// documentation — nothing was added from memory:
//  • CLI troubleshooting cheat sheet (7.6.0)
//    docs.fortinet.com/document/fortigate/7.6.0/cli-troubleshooting-cheat-sheet/420966
//  • Administration guide 7.6.6: "Debugging the packet flow" (54688),
//    "IPsec related diagnose commands" (44240), "Performing a sniffer trace
//    or packet capture" (680228: "To stop the sniffer, type CTRL+C"),
//    "Routing concepts" (139692), "Using a session table" (562859),
//    "How the FortiGate firmware license works" (7.6.4, 299518)
//  • CLI reference 7.6.6: diagnose vpn ike (309172113), diagnose sys
//    (235530229), diagnose ip router bgp (869760727) / ospf (929006518)
// Left out on purpose: commands that reset, format or reboot the device
// (factoryreset, formatlogdisk, execute reboot…), ones the docs warn can
// cause an outage (execute fsso refresh, diagnose sys ha reset-uptime), ones
// that put a password on the command line (diagnose test authserver), and
// ones whose 7.6 argument syntax the docs don't spell out.
import type { CommandItem, VendorLists } from './types'

/** A plain command. */
const c = (cmd: string, desc: string, extra: Partial<CommandItem> = {}): CommandItem => ({
  kind: 'command',
  cmd,
  desc,
  ...extra,
})

export const fortigate: VendorLists = {
  label: 'FortiGate · FortiOS 7.6',
  stopDebug: ['diagnose debug disable', 'diagnose debug reset'],
  stopKeys: '\x03',
  lists: [
    {
      id: 'system',
      name: 'System',
      icon: 'system',
      color: '#5B4FC4',
      summary: 'Status, resources, hardware and logging',
      sections: [
        {
          title: 'Status',
          items: [
            c('get system status', 'Version, build, serial number, operation mode'),
            c(
              'get system performance status',
              'CPU and memory utilization, network usage, sessions',
            ),
            c('execute time', 'Current system time'),
            c('diagnose sys vd list', 'Virtual domains and their statistics'),
            c('diagnose sys cmdb info', 'Latest configuration change made by a daemon'),
          ],
        },
        {
          title: 'Resources',
          items: [
            c(
              'diagnose sys top',
              'Top processes (Shift+C sort by CPU, Shift+M by memory, q to quit)',
            ),
            c('diagnose sys mpstat <seconds>', 'CPU usage per core every n seconds'),
            c('diagnose hardware sysinfo memory', 'System memory information'),
            c('diagnose hardware sysinfo cpu', 'CPU information'),
            c(
              'diagnose sys process pidof <daemon>',
              'PID of a running daemon (names from diagnose sys top)',
            ),
            c('diagnose debug crashlog read', 'System and application crashes'),
            c('diagnose debug config-error-log read', 'Errors in the configuration file'),
          ],
        },
        {
          title: 'Hardware',
          items: [
            c('diagnose hardware deviceinfo disk', 'Disk information'),
            c('diagnose sys flash list', 'Flash partitions'),
            c('diagnose hardware sysinfo interrupts', 'Hardware interrupt statistics'),
          ],
        },
        {
          title: 'Licenses',
          items: [
            c(
              'diagnose test update info contract',
              'System contracts and their expiry dates; pending_registration shows FortiCare registration',
            ),
            c(
              'diagnose test update info contract | grep FMWR',
              'Firmware & General Updates (FMWR) contract expiry',
            ),
          ],
        },
        {
          title: 'Logging',
          items: [
            c(
              'execute log fortianalyzer test-connectivity',
              'Test the connection to FortiAnalyzer',
            ),
            c('execute log filter', 'Show the log filters that are set'),
          ],
        },
      ],
    },
    {
      id: 'interfaces',
      name: 'Interfaces',
      icon: 'interfaces',
      color: '#2F7A8C',
      summary: 'Interfaces, ARP, sessions and reachability',
      sections: [
        {
          title: 'Interfaces',
          items: [
            c('get system interface physical', 'Physical interfaces: status, speed, IP'),
            c('get system interface', 'Summary of every interface and its IP settings'),
            c('diagnose ip address list', 'IP addresses in use'),
            c('get hardware nic <interface>', 'Detailed NIC counters and errors'),
            c('get system interface transceiver', 'Connected transceivers'),
            c('diagnose netlink brctl list', 'Software switches on the FortiGate'),
          ],
        },
        {
          title: 'ARP',
          items: [
            c('get system arp', 'ARP table'),
            c('diagnose ip arp list', 'ARP entries in detail'),
          ],
        },
        {
          title: 'Sessions',
          items: [
            c('diagnose sys session stat', 'Session table statistics'),
            c('diagnose sys session filter dst <ip>', 'Filter the session table by destination IP'),
            c('diagnose sys session filter', 'Show the session filters that are set'),
            c('diagnose sys session list', 'Sessions matching the filter'),
            c('diagnose firewall iprope list', 'Internal firewall policy table'),
          ],
        },
        {
          title: 'Reachability',
          items: [
            c('execute ping <ip>', 'Ping an address'),
            c('execute traceroute <ip>', 'Traceroute to an address'),
          ],
        },
      ],
    },
    {
      id: 'routing',
      name: 'Routing',
      icon: 'routing',
      color: '#B26A1E',
      summary: 'Routing table, kernel routes and BFD',
      sections: [
        {
          title: 'Routing table',
          items: [
            c('get router info routing-table all', 'Routing table'),
            c(
              'get router info routing-table database',
              'Routing database, including inactive routes',
            ),
            c(
              'get router info routing-table details <ip>',
              'Route lookup for a destination: next hops and interface',
            ),
            c('get router info kernel', 'Kernel routing table'),
            c('get router info protocols', 'Routing protocols running'),
            c('get router info6 routing-table database', 'IPv6 routing database'),
          ],
        },
        {
          title: 'BFD',
          items: [c('get router info bfd neighbor', 'BFD neighbors')],
        },
      ],
    },
    {
      id: 'sdwan',
      name: 'SD-WAN',
      icon: 'sdwan',
      color: '#C2577A',
      summary: 'Members, health checks, rules and link monitor',
      sections: [
        {
          title: 'SD-WAN',
          items: [
            c('diagnose sys sdwan health-check status', 'Health check statistics per member'),
            c('diagnose sys sdwan member', 'SD-WAN members'),
            c('diagnose sys sdwan service4', 'SD-WAN rules in the control plane (IPv4)'),
            c('diagnose sys sdwan service6', 'SD-WAN rules in the control plane (IPv6)'),
            c('diagnose firewall proute list', 'SD-WAN rules and policy routes in the data plane'),
          ],
        },
        {
          title: 'Link monitor',
          items: [
            c('diagnose sys link-monitor status', 'Link monitor statistics'),
            c('diagnose sys link-monitor interface <interface>', 'Link monitor of one interface'),
            c('diagnose test application lnkmtd 1', 'Link monitor daemon statistics (1)'),
            c('diagnose test application lnkmtd 2', 'Link monitor daemon statistics (2)'),
            c('diagnose test application lnkmtd 3', 'Link monitor daemon statistics (3)'),
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
            c('get router info bgp summary', 'BGP summary (IPv4)'),
            c('get router info6 bgp summary', 'BGP summary (IPv6)'),
            c('get router info bgp neighbors', 'Every BGP peer in detail'),
          ],
        },
        {
          title: 'Routes of a peer',
          items: [
            c(
              'get router info bgp neighbors <peer-ip> received-routes',
              'Routes received from the peer',
            ),
            c(
              'get router info bgp neighbors <peer-ip> advertised-routes',
              'Routes advertised to the peer',
            ),
            c('get router info bgp neighbors <peer-ip> routes', 'Routes learned from the peer'),
            c(
              'get router info bgp network <prefix>',
              'BGP entry for a prefix and the peers it is advertised to',
            ),
          ],
        },
        {
          title: 'Actions',
          items: [
            c(
              'execute router clear bgp ip <peer-ip> soft',
              'Soft reset of one peer (routes re-sent both ways)',
              { action: true },
            ),
          ],
        },
      ],
    },
    {
      id: 'ospf',
      name: 'OSPF',
      icon: 'ospf',
      color: '#6B53B8',
      summary: 'Status, neighbors, interfaces and LSDB',
      sections: [
        {
          title: 'OSPF',
          items: [
            c('get router info ospf status', 'OSPF process status'),
            c('get router info ospf neighbor', 'OSPF neighbors'),
            c('get router info ospf neighbor all', 'OSPF neighbor information in detail'),
            c('get router info ospf interface', 'Interfaces running OSPF'),
            c('get router info ospf database brief', 'OSPF database in brief'),
            c('get router info6 ospf neighbor', 'OSPFv3 neighbors (IPv6)'),
          ],
        },
      ],
    },
    {
      id: 'vpn',
      name: 'IPsec',
      icon: 'vpn',
      color: '#1F7A55',
      summary: 'Phase 1, phase 2, IKE counters and filters',
      sections: [
        {
          title: 'Tunnels',
          items: [
            c('get vpn ipsec tunnel summary', 'Summary of every IPsec tunnel'),
            c('get vpn ipsec tunnel details', 'Detailed information of every tunnel'),
            c('diagnose vpn ike gateway list', 'Phase 1 (IKE gateways)'),
            c('diagnose vpn ike gateway list name <phase1-name>', 'Phase 1 of one tunnel'),
            c('diagnose vpn tunnel list', 'Phase 2 SAs and counters'),
          ],
        },
        {
          title: 'IKE',
          items: [
            c('diagnose vpn ipsec status', 'Encryption and decryption counters'),
            c('diagnose vpn ike status', 'IKE status'),
            c('diagnose vpn ike counts', 'IKE counts'),
            c('diagnose vpn ike errors', 'IKE errors'),
            c('diagnose vpn ike stats', 'IKE statistics'),
            c('diagnose vpn ike routes', 'Routes added by IKE'),
            c('diagnose vpn ike log filter list', 'IKE debug filter currently set'),
            c('diagnose vpn ike log filter clear', 'Clear the IKE debug filter'),
          ],
        },
        {
          title: 'Actions',
          items: [
            c(
              'diagnose vpn tunnel flush <phase2-name>',
              'Flush the phase 2 SAs of a tunnel (it renegotiates)',
              { action: true },
            ),
            c(
              'diagnose vpn ike gateway clear name <phase1-name>',
              'Clear a phase 1 (the tunnel goes down and renegotiates)',
              { action: true },
            ),
          ],
        },
      ],
    },
    {
      id: 'ha',
      name: 'HA',
      icon: 'ha',
      color: '#8A6D1F',
      summary: 'Cluster status, checksums and history',
      sections: [
        {
          title: 'Cluster',
          items: [
            c('get system ha status', 'HA status and members'),
            c('diagnose system ha status', 'HA status and information'),
            c('diagnose sys ha history read', 'HA event history'),
            c('execute ha manage <index> <username>', 'Log into a specific cluster member'),
          ],
        },
        {
          title: 'Synchronization',
          items: [
            c('diagnose sys ha checksum cluster', 'Configuration checksums of every member'),
            c('diagnose sys ha checksum show <vdom>', 'Detailed checksums of one VDOM'),
            c('diagnose sys ha checksum recalculate', 'Recalculate the HA checksums', {
              action: true,
            }),
            c('execute ha synchronize start', 'Start HA synchronization manually', {
              action: true,
            }),
          ],
        },
      ],
    },
    {
      id: 'security',
      name: 'Security',
      icon: 'security',
      color: '#A23B72',
      summary: 'IPS engine, web filter, DNS filter, signatures and proxy',
      sections: [
        {
          title: 'IPS engine',
          items: [
            c('diagnose test application ipsmonitor 1', 'IPS engine information'),
            c('diagnose ips session list', 'IPS sessions in each engine'),
            c('diagnose test application ipsmonitor 13', 'IPS sessions in each engine (monitor)'),
            c('diagnose test application ipsmonitor 99', 'Restart all IPS engines and monitor', {
              action: true,
            }),
          ],
        },
        {
          title: 'Web & DNS filter',
          items: [
            c('diagnose webfilter fortiguard statistics', 'Rating cache and daemon statistics'),
            c('diagnose debug rating', 'Web filter rating servers'),
            c(
              'diagnose test application urlfilter <option>',
              'Web filter test output for an option',
            ),
            c('diagnose test application dnsproxy <option>', 'DNS proxy test output for an option'),
          ],
        },
        {
          title: 'Signatures & FortiGuard',
          items: [
            c('diagnose autoupdate versions', 'Versions of engines and signature databases'),
            c('diagnose autoupdate status', 'FortiGuard update and license status'),
            c('execute update-now', 'Run a FortiGuard update now', { action: true }),
          ],
        },
        {
          title: 'Proxy (WAD)',
          items: [
            c('diagnose test application wad 1000', 'All WAD processes'),
            c('diagnose test application wad 2', 'Total WAD memory usage'),
          ],
        },
      ],
    },
    {
      id: 'users',
      name: 'Users/ZTNA',
      icon: 'users',
      color: '#2E7D9A',
      summary: 'Authenticated users, FSSO and EMS / ZTNA',
      sections: [
        {
          title: 'Authenticated users',
          items: [
            c('diagnose firewall auth list', 'Authenticated IPv4 users'),
            c('diagnose wad user list', 'Users authenticated by the proxy (WAD)'),
          ],
        },
        {
          title: 'FSSO',
          items: [
            c(
              'diagnose debug authd fsso server-status',
              'Connection between FortiGate and the collector agent',
            ),
            c('diagnose debug authd fsso list', 'FSSO logged-on users'),
            c('diagnose debug fsso-polling summary', 'Polls from FortiGate to the DC (summary)'),
            c('diagnose debug fsso-polling detail', 'Polls from FortiGate to the DC (detail)'),
            c('diagnose debug fsso-polling user', 'Users seen by DC polling'),
            c(
              'diagnose debug authd fsso refresh-logons',
              'Ask the collector agent to resend its logged-on users',
              { action: true },
            ),
          ],
        },
        {
          title: 'EMS / ZTNA',
          items: [
            c(
              'diagnose endpoint fctems test-connectivity <ems>',
              'Test connectivity to FortiClient EMS',
            ),
            c('execute fctems verify <ems>', "Verify the EMS's certificate"),
            c('diagnose test application fcnacd 2', 'EMS connectivity information'),
            c('diagnose firewall dynamic list', 'EMS posture tags and dynamic IP / MAC addresses'),
            c('diagnose wad worker policy list', 'Application gateway rule statistics'),
          ],
        },
      ],
    },
    {
      id: 'fortilink',
      name: 'Switch/AP',
      icon: 'fortilink',
      color: '#5C7A2E',
      summary: 'Managed FortiSwitches and FortiAPs',
      sections: [
        {
          title: 'FortiSwitch',
          items: [
            c(
              'execute switch-controller get-conn-status <fortiswitch-sn>',
              'Connection status of a FortiSwitch',
            ),
            c('diagnose switch-controller switch-info mac-table', 'MAC address table'),
            c('diagnose switch-controller switch-info port-stats', 'Port statistics'),
            c('diagnose switch-controller switch-info port-properties', 'Port properties'),
            c('diagnose switch-controller switch-info trunk status', 'Trunk information'),
            c('diagnose switch-controller switch-info stp', 'STP instance status'),
            c('diagnose switch-controller switch-info lldp', 'LLDP information'),
            c('diagnose switch-controller switch-info poe', 'PoE information'),
            c('diagnose switch-controller switch-info mclag', 'MCLAG information'),
            c('diagnose switch-controller switch-info modules', 'Transceiver modules'),
            c('diagnose switch-controller switch-info 802.1X', '802.1X status per port'),
          ],
        },
        {
          title: 'FortiAP',
          items: [
            c('diagnose wireless-controller wlac -c wtp', 'Managed FortiAPs'),
            c('diagnose wireless-controller wlac -c sta', 'Wireless clients'),
            c('diagnose wireless-controller wlac -c vap', 'Virtual APs: SSID, BSSID, interface'),
            c('diagnose wireless-controller wlac help', 'Wireless controller debug options'),
          ],
        },
      ],
    },
    {
      id: 'debug',
      name: 'Debug',
      icon: 'debug',
      color: '#B23B3B',
      summary: 'Traces and real-time debugs — they load the CPU, stop them when done',
      debug: true,
      sections: [
        {
          title: 'Traffic',
          items: [
            {
              kind: 'recipe',
              name: 'Debug flow of an IP',
              desc: 'Why traffic to or from an address is accepted, routed or dropped',
              fields: [
                { key: 'ip', label: 'IP address', placeholder: '10.0.0.25' },
                { key: 'count', label: 'Packets to trace', default: '100' },
              ],
              lines: [
                'diagnose debug reset',
                'diagnose debug flow filter clear',
                'diagnose debug flow filter addr {{ip}}',
                'diagnose debug flow show function-name enable',
                'diagnose debug flow show iprope enable',
                'diagnose debug console timestamp enable',
                'diagnose debug enable',
                'diagnose debug flow trace start {{count}}',
              ],
            },
            {
              kind: 'recipe',
              name: 'Packet sniffer',
              desc: 'Capture packets on an interface with a filter (Ctrl+C to stop)',
              fields: [
                { key: 'iface', label: 'Interface', default: 'any' },
                { key: 'filter', label: 'Filter', placeholder: 'host 10.0.0.25' },
                { key: 'verbose', label: 'Verbose (1–6)', default: '4' },
                { key: 'count', label: 'Packets', default: '100' },
              ],
              lines: ["diagnose sniffer packet {{iface}} '{{filter}}' {{verbose}} {{count}} l"],
            },
            c('Ctrl+C', 'Stop a running sniffer (or any command waiting on output)', {
              keys: '\x03',
            }),
            c('diagnose debug flow trace stop', 'Stop a running flow trace'),
            c('diagnose debug flow filter clear', 'Clear the IPv4 debug flow filters'),
          ],
        },
        {
          title: 'VPN & routing',
          items: [
            {
              kind: 'recipe',
              name: 'IKE debug of a peer',
              desc: 'Phase 1 and phase 2 negotiation with one remote gateway',
              fields: [{ key: 'peer', label: 'Remote gateway IP', placeholder: '203.0.113.2' }],
              lines: [
                'diagnose debug reset',
                'diagnose vpn ike log filter clear',
                'diagnose vpn ike log filter rem-addr4 {{peer}}',
                'diagnose debug console timestamp enable',
                'diagnose debug application ike -1',
                'diagnose debug enable',
              ],
            },
            {
              kind: 'recipe',
              name: 'BGP debug',
              desc: 'Real-time BGP debugging',
              fields: [],
              lines: [
                'diagnose ip router bgp all enable',
                'diagnose ip router bgp level info',
                'diagnose debug enable',
              ],
            },
            {
              kind: 'recipe',
              name: 'OSPF debug',
              desc: 'Real-time OSPF debugging',
              fields: [],
              lines: [
                'diagnose ip router ospf all enable',
                'diagnose ip router ospf level info',
                'diagnose debug enable',
              ],
            },
            {
              kind: 'recipe',
              name: 'Link monitor debug',
              desc: 'Real-time link monitor (SD-WAN health) debugging',
              fields: [],
              lines: ['diagnose debug application link-monitor -1', 'diagnose debug enable'],
            },
          ],
        },
        {
          title: 'Security',
          items: [
            {
              kind: 'recipe',
              name: 'IPS debug of a host and port',
              desc: 'IPS engine debugs for Application Control and IPS (CPU intensive)',
              fields: [
                { key: 'ip', label: 'Host IP', placeholder: '10.0.0.25' },
                { key: 'port', label: 'Port', placeholder: '443' },
              ],
              lines: [
                'diagnose ips filter set "host {{ip}} and port {{port}}"',
                'diagnose ips debug enable all',
                'diagnose debug enable',
              ],
            },
            {
              kind: 'recipe',
              name: 'Web filter debug',
              desc: 'Real-time web filter daemon debugging',
              fields: [],
              lines: ['diagnose debug application urlfilter -1', 'diagnose debug enable'],
            },
            {
              kind: 'recipe',
              name: 'DNS proxy debug',
              desc: 'DNS filter, translation and resolution',
              fields: [],
              lines: ['diagnose debug application dnsproxy -1', 'diagnose debug enable'],
            },
          ],
        },
        {
          title: 'Users',
          items: [
            {
              kind: 'recipe',
              name: 'Authentication debug',
              desc: 'Remote and local authentication (fnbamd, authd)',
              fields: [],
              lines: [
                'diagnose debug application fnbamd -1',
                'diagnose debug application authd -1',
                'diagnose debug enable',
              ],
            },
            {
              kind: 'recipe',
              name: 'FSSO polling debug',
              desc: 'FortiGate polling the domain controllers',
              fields: [],
              lines: [
                'diagnose debug application fssod -1',
                'diagnose debug application smbcd -1',
                'diagnose debug enable',
              ],
            },
            {
              kind: 'recipe',
              name: 'SAML debug',
              desc: 'Real-time SAML debugging',
              fields: [],
              lines: ['diagnose debug application samld -1', 'diagnose debug enable'],
            },
            {
              kind: 'recipe',
              name: 'FortiClient NAC debug',
              desc: 'FortiGate to EMS connectivity (fcnacd)',
              fields: [],
              lines: ['diagnose debug application fcnacd -1', 'diagnose debug enable'],
            },
          ],
        },
        {
          title: 'Platform',
          items: [
            {
              kind: 'recipe',
              name: 'HA debug',
              desc: 'HA daemons (hatalk, hasync, harelay)',
              fields: [],
              lines: [
                'diagnose debug application hatalk -1',
                'diagnose debug application hasync -1',
                'diagnose debug application harelay -1',
                'diagnose debug enable',
              ],
            },
            {
              kind: 'recipe',
              name: 'Wireless client debug',
              desc: 'A station connecting to a FortiAP',
              fields: [{ key: 'mac', label: 'Client MAC', placeholder: 'aa:bb:cc:dd:ee:ff' }],
              lines: [
                'diagnose wireless-controller wlac sta_filter clear',
                'diagnose wireless-controller wlac sta_filter {{mac}} 255',
                'diagnose debug enable',
              ],
            },
          ],
        },
      ],
    },
  ],
}
