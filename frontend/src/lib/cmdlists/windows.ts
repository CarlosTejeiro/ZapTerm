// Windows (PowerShell) command lists. Also shown for local tabs on Windows,
// which run PowerShell (pwsh when installed, else Windows PowerShell).
//
// Every command below was checked against Microsoft's documentation on
// learn.microsoft.com — nothing was added from memory: the PowerShell module
// references (Microsoft.PowerShell.Management/Utility/Core/Diagnostics,
// Storage, CimCmdlets, NetTCPIP, NetAdapter, NetConnection, DnsClient,
// NetSecurity, Hyper-V), the Windows Commands reference (systeminfo,
// ipconfig, ping, tracert, pathping), "Basic commands for WSL" and "Creating
// Get-WinEvent queries with FilterHashtable" (Level 2 = Error).
// Get-Uptime needs PowerShell 6 or later. The Hyper-V cmdlets need the
// Hyper-V module and an elevated session; Measure-VM needs resource metering
// enabled on the VM.
import type { CommandItem, VendorLists } from './types'

const c = (cmd: string, desc: string, extra: Partial<CommandItem> = {}): CommandItem => ({
  kind: 'command',
  cmd,
  desc,
  ...extra,
})

export const windows: VendorLists = {
  label: 'Windows · PowerShell',
  stopDebug: [],
  lists: [
    {
      id: 'system',
      name: 'System',
      icon: 'system',
      color: '#5B4FC4',
      summary: 'OS, uptime, updates, disks, processes and services',
      sections: [
        {
          title: 'System',
          items: [
            c('systeminfo', 'OS, build, boot time, memory and hotfixes'),
            c('Get-ComputerInfo', 'Detailed system and OS properties'),
            c('Get-Uptime', 'Time since the last boot (PowerShell 6+)'),
            c('Get-HotFix', 'Installed updates'),
            c('Get-Volume', 'Volumes, file systems and free space'),
          ],
        },
        {
          title: 'Processes & services',
          items: [
            c(
              'Get-Process | Sort-Object -Property CPU -Descending | Select-Object -First 15',
              'Top 15 processes by CPU time',
            ),
            c('Get-Process -Name <name>', 'One process by name'),
            c('Get-Service | Where-Object Status -EQ Running', 'Running services'),
            c('Get-Service -Name <name>', 'One service and its status'),
            c('Restart-Service -Name <name>', 'Restart a service', { action: true }),
          ],
        },
        {
          title: 'Administrator',
          items: [
            c(
              'Start-Process -FilePath "powershell" -Verb RunAs',
              'Open an elevated PowerShell in a new window (asks for admin credentials)',
            ),
          ],
        },
      ],
    },
    {
      id: 'network',
      name: 'Network',
      icon: 'interfaces',
      color: '#2F7A8C',
      summary: 'Addresses, adapters, routes, neighbors, ports and DNS',
      sections: [
        {
          title: 'Addresses & adapters',
          items: [
            c('ipconfig /all', 'Full IP configuration of every adapter'),
            c('Get-NetIPConfiguration', 'IP, gateway and DNS per interface'),
            c('Get-NetIPAddress -AddressFamily IPv4', 'IPv4 addresses'),
            c('Get-NetAdapter', 'Adapters, status and link speed'),
            c('Get-NetAdapterStatistics', 'Bytes and packets per adapter'),
            c('Get-NetConnectionProfile', 'Network category (Public/Private/Domain)'),
          ],
        },
        {
          title: 'Routes & neighbors',
          items: [
            c('Get-NetRoute -AddressFamily IPv4', 'IPv4 routing table'),
            c('Find-NetRoute -RemoteIPAddress <ip>', 'Route and source address for a destination'),
            c('Get-NetNeighbor -AddressFamily IPv4', 'ARP table'),
          ],
        },
        {
          title: 'Ports',
          items: [
            c('Get-NetTCPConnection -State Listen', 'Listening TCP ports'),
            c('Get-NetTCPConnection -State Established', 'Established TCP connections'),
            c('Get-NetUDPEndpoint', 'UDP endpoints'),
          ],
        },
        {
          title: 'DNS',
          items: [
            c('Get-DnsClientServerAddress', 'DNS servers per interface'),
            c('Get-DnsClientCache', 'DNS client cache'),
            c('Resolve-DnsName -Name <name>', 'Resolve a name'),
            c('Clear-DnsClientCache', 'Flush the DNS client cache', { action: true }),
          ],
        },
      ],
    },
    {
      id: 'diagnostics',
      name: 'Diagnostics',
      icon: 'sdwan',
      color: '#3E6FC2',
      summary: 'Reachability, paths and TCP port tests',
      sections: [
        {
          title: 'Reachability',
          items: [
            c('ping /n 4 <host>', 'Four pings to a host'),
            c('tracert /d <host>', 'Path to a host, no name lookups'),
            c('pathping /n <host>', 'Path with per-hop loss, no name lookups'),
            c('Test-NetConnection -ComputerName <host> -Port <port>', 'Test a TCP port'),
            c('Test-NetConnection -ComputerName <host> -TraceRoute', 'Ping plus traceroute'),
          ],
        },
      ],
    },
    {
      id: 'wsl',
      name: 'WSL',
      icon: 'routing',
      color: '#B26A1E',
      summary: 'Distributions, state and updates',
      sections: [
        {
          title: 'WSL',
          items: [
            c('wsl --list --verbose', 'Installed distributions, state and WSL version'),
            c('wsl --status', 'Default distribution and WSL settings'),
            c('wsl --version', 'WSL, kernel and WSLg versions'),
            c('wsl --list --online', 'Distributions available to install'),
            c('wsl --update', 'Update WSL', { action: true }),
            c('wsl --terminate <distro>', 'Stop one distribution', { action: true }),
            c('wsl --shutdown', 'Stop every distribution and the WSL VM', { action: true }),
          ],
        },
      ],
    },
    {
      id: 'hyperv',
      name: 'Hyper-V',
      icon: 'ha',
      color: '#1F7A55',
      summary: 'Host, VMs, switches, adapters, checkpoints and replication',
      sections: [
        {
          title: 'Host & VMs',
          items: [
            c('Get-VMHost', 'Hyper-V host settings'),
            c('Get-VM', 'Virtual machines, state, CPU and memory'),
            c('Get-VM | Where-Object State -EQ Running', 'Running virtual machines'),
            c('Get-VM -Name <vm>', 'One virtual machine'),
            c('Measure-VM -Name <vm>', 'Resource use of a VM (needs resource metering)'),
          ],
        },
        {
          title: 'Network & storage',
          items: [
            c('Get-VMSwitch', 'Virtual switches'),
            c('Get-VMNetworkAdapter -All', 'Network adapters of every VM and the host'),
            c('Get-VMNetworkAdapter -VMName <vm>', 'Network adapters of one VM'),
            c('Get-VMHardDiskDrive -VMName <vm>', 'Virtual disks of one VM'),
          ],
        },
        {
          title: 'Checkpoints, replication & integration',
          items: [
            c('Get-VMSnapshot -VMName <vm>', 'Checkpoints of one VM'),
            c('Get-VMReplication', 'Replication state of the VMs'),
            c('Get-VMIntegrationService -VMName <vm>', 'Integration services of one VM'),
          ],
        },
        {
          title: 'Actions',
          items: [
            c('Start-VM -Name <vm>', 'Start a VM', { action: true }),
            c('Stop-VM -Name <vm>', 'Shut down a VM', { action: true }),
          ],
        },
      ],
    },
    {
      id: 'security',
      name: 'Firewall & events',
      icon: 'security',
      color: '#A23B72',
      summary: 'Firewall profiles and rules, recent event log entries',
      sections: [
        {
          title: 'Windows Firewall',
          items: [
            c('Get-NetFirewallProfile', 'Firewall profiles and whether they are on'),
            c('Get-NetFirewallRule -Enabled True -Direction Inbound', 'Enabled inbound rules'),
          ],
        },
        {
          title: 'Event logs',
          items: [
            c(
              "Get-WinEvent -FilterHashtable @{LogName='System'; Level=2} -MaxEvents 50",
              'Last 50 errors in the System log',
            ),
            c('Get-WinEvent -LogName System -MaxEvents 50', 'Last 50 System events'),
            c('Get-WinEvent -LogName Application -MaxEvents 50', 'Last 50 Application events'),
          ],
        },
      ],
    },
  ],
}
