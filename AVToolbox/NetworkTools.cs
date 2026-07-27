// === AVToolbox — network tools (Ethernet-focused, like NetTools.ps1) ===
using System.Net;
using System.Net.NetworkInformation;
using System.Net.Sockets;
using System.Text.RegularExpressions;

namespace AVToolbox;

public static class NetworkTools
{
    /// <summary>Active non-loopback interfaces with an IPv4 address, Ethernet first.</summary>
    private static List<NetworkInterface> ActiveNics() =>
        NetworkInterface.GetAllNetworkInterfaces()
            .Where(n => n.OperationalStatus == OperationalStatus.Up
                     && n.NetworkInterfaceType != NetworkInterfaceType.Loopback
                     && n.GetIPProperties().UnicastAddresses
                         .Any(a => a.Address.AddressFamily == AddressFamily.InterNetwork))
            .OrderBy(n => n.NetworkInterfaceType == NetworkInterfaceType.Ethernet ? 0 : 1)
            .ToList();

    private static UnicastIPAddressInformation? Ipv4Of(NetworkInterface nic) =>
        nic.GetIPProperties().UnicastAddresses
           .FirstOrDefault(a => a.Address.AddressFamily == AddressFamily.InterNetwork);

    public static void NetInfo()
    {
        Ui.Header("NETWORK INFO");
        var nics = ActiveNics();
        if (nics.Count == 0) { Ui.Err("No active network interface found."); return; }

        foreach (var nic in nics)
        {
            var props = nic.GetIPProperties();
            var ip = Ipv4Of(nic);
            var gw = props.GatewayAddresses
                          .FirstOrDefault(g => g.Address.AddressFamily == AddressFamily.InterNetwork);
            var dns = props.DnsAddresses.Where(d => d.AddressFamily == AddressFamily.InterNetwork);
            bool dhcp = props.GetIPv4Properties()?.IsDhcpEnabled ?? false;

            Ui.Write($"  {nic.Name}", ConsoleColor.Cyan);
            Ui.Hint($"  {nic.Description}");
            Ui.Label("IP", $"{ip?.Address} / {ip?.IPv4Mask}");
            Ui.Label("Gateway", gw?.Address.ToString() ?? "-");
            Ui.Label("DNS", string.Join(", ", dns.Select(d => d.ToString())));
            Ui.Label("MAC", FormatMac(nic.GetPhysicalAddress()));
            Ui.Label("Link speed", nic.Speed > 0 ? $"{nic.Speed / 1_000_000} Mbps" : "-");
            Ui.Label("Mode", dhcp ? "DHCP" : "Static", dhcp ? ConsoleColor.Green : ConsoleColor.Yellow);
            Ui.Blank();
        }
    }

    public static void MyIp()
    {
        Ui.Header("MY IP");
        var nic = ActiveNics().FirstOrDefault();
        if (nic != null)
        {
            var props = nic.GetIPProperties();
            var gw = props.GatewayAddresses
                          .FirstOrDefault(g => g.Address.AddressFamily == AddressFamily.InterNetwork);
            Ui.Label("Interface", nic.Name);
            Ui.Label("Local IP", Ipv4Of(nic)?.Address.ToString() ?? "-");
            Ui.Label("Gateway", gw?.Address.ToString() ?? "-");
            Ui.Label("MAC", FormatMac(nic.GetPhysicalAddress()));
        }

        try
        {
            using var http = new HttpClient { Timeout = TimeSpan.FromSeconds(6) };
            var pub = http.GetStringAsync("https://api.ipify.org").GetAwaiter().GetResult().Trim();
            Ui.Label("Public IP", pub, ConsoleColor.Green);
        }
        catch
        {
            Ui.Label("Public IP", "unreachable (no internet?)", ConsoleColor.Red);
        }
    }

    public static void Scan()
    {
        var nic = ActiveNics().FirstOrDefault();
        var ip = nic != null ? Ipv4Of(nic) : null;
        if (ip == null) { Ui.Err("No active IPv4 interface."); return; }

        var octets = ip.Address.GetAddressBytes();
        string prefix = $"{octets[0]}.{octets[1]}.{octets[2]}";
        Ui.Header($"SCAN {prefix}.1-254");
        Ui.Hint("Pinging in parallel...");

        var alive = new List<string>();
        var tasks = Enumerable.Range(1, 254).Select(async i =>
        {
            string host = $"{prefix}.{i}";
            try
            {
                using var ping = new Ping();
                var reply = await ping.SendPingAsync(host, 350);
                if (reply.Status == IPStatus.Success)
                    lock (alive) alive.Add(host);
            }
            catch { /* host unreachable */ }
        });
        Task.WhenAll(tasks).GetAwaiter().GetResult();

        // MAC addresses from the ARP cache
        var arp = new Dictionary<string, string>();
        var arpOut = Exec.Capture("arp", "-a") ?? "";
        foreach (Match m in Regex.Matches(arpOut, @"(\d+\.\d+\.\d+\.\d+)\s+([0-9a-f-]{17})", RegexOptions.IgnoreCase))
            arp[m.Groups[1].Value] = m.Groups[2].Value.ToUpper();

        // Reverse DNS with a short timeout, in parallel
        var names = new Dictionary<string, string>();
        var dnsTasks = alive.Select(async host =>
        {
            try
            {
                var t = Dns.GetHostEntryAsync(host);
                if (await Task.WhenAny(t, Task.Delay(700)) == t)
                    lock (names) names[host] = t.Result.HostName;
            }
            catch { /* no PTR record */ }
        });
        Task.WhenAll(dnsTasks).GetAwaiter().GetResult();

        Ui.Blank();
        foreach (var host in alive.OrderBy(h => int.Parse(h.Split('.')[3])))
        {
            string self = host == ip.Address.ToString() ? "  (this PC)" : "";
            Ui.Write($"  {host,-16}", ConsoleColor.Green, newline: false);
            Ui.Write($"{arp.GetValueOrDefault(host, "-"),-19}", ConsoleColor.White, newline: false);
            Ui.Write(names.GetValueOrDefault(host, "") + self, ConsoleColor.DarkGray);
        }
        Ui.Blank();
        Ui.Ok($"{alive.Count} hosts up.");
    }

    public static void PortCheck()
    {
        string host = Ui.Ask("Host/IP");
        if (host.Length == 0) return;
        string portStr = Ui.Ask("Port");
        if (!int.TryParse(portStr, out int port)) { Ui.Err("Invalid port."); return; }
        Ui.Blank();
        TestPort(host, port, verbose: true);
    }

    public static void PortScan()
    {
        string host = Ui.Ask("Host/IP");
        if (host.Length == 0) return;

        (int Port, string Name)[] common =
        {
            (21, "FTP"), (22, "SSH"), (23, "Telnet"), (80, "HTTP"), (443, "HTTPS"),
            (445, "SMB"), (3389, "RDP"), (5353, "mDNS"), (5900, "VNC"),
            (5960, "NDI discovery"), (5961, "NDI stream"), (6789, "ArtNet alt"),
            (7000, "AirPlay"), (8000, "HTTP alt"), (8080, "HTTP proxy"), (9000, "QSC/Dante")
        };

        Ui.Header($"PORT SCAN {host}");
        int open = 0;
        Parallel.ForEach(common, c =>
        {
            if (TestPort(host, c.Port, verbose: false))
            {
                lock (common)
                {
                    Ui.Write($"  {c.Port,5}  ", ConsoleColor.Green, newline: false);
                    Ui.Write("OPEN   ", ConsoleColor.Green, newline: false);
                    Ui.Write(c.Name, ConsoleColor.White);
                    open++;
                }
            }
        });
        if (open == 0) Ui.Hint("No common ports open (or host is filtering).");
    }

    private static bool TestPort(string host, int port, bool verbose)
    {
        try
        {
            using var client = new TcpClient();
            bool ok = client.ConnectAsync(host, port).Wait(1500) && client.Connected;
            if (verbose)
            {
                if (ok) Ui.Ok($"{host}:{port} OPEN");
                else Ui.Err($"{host}:{port} closed / timeout");
            }
            return ok;
        }
        catch
        {
            if (verbose) Ui.Err($"{host}:{port} closed / unreachable");
            return false;
        }
    }

    public static void Connections()
    {
        Ui.Header("ACTIVE TCP CONNECTIONS");
        var output = Exec.Capture("netstat", "-ano") ?? "";
        var procs = System.Diagnostics.Process.GetProcesses()
            .ToDictionary(p => p.Id, p => p.ProcessName);

        var rows = new List<(string Proc, string Local, string Remote, string State)>();
        foreach (var line in output.Split('\n'))
        {
            var parts = line.Trim().Split(' ', StringSplitOptions.RemoveEmptyEntries);
            if (parts.Length == 5 && parts[0] == "TCP" && parts[3] == "ESTABLISHED"
                && int.TryParse(parts[4], out int pid))
            {
                rows.Add((procs.GetValueOrDefault(pid, $"pid {pid}"), parts[1], parts[2], parts[3]));
            }
        }

        foreach (var group in rows.GroupBy(r => r.Proc).OrderBy(g => g.Key))
        {
            Ui.Write($"  {group.Key}", ConsoleColor.Yellow);
            foreach (var r in group.Take(12))
                Ui.Hint($"  {r.Local,-24} -> {r.Remote}");
            if (group.Count() > 12) Ui.Hint($"  ... and {group.Count() - 12} more");
        }
        Ui.Blank();
        Ui.Ok($"{rows.Count} established connections.");
    }

    public static void FlushDns()
    {
        Ui.Header("FLUSH DNS");
        if (Exec.Run("ipconfig", "/flushdns") == 0) Ui.Ok("DNS cache cleared.");
        else Ui.Err("Failed to flush DNS.");
    }

    private static string FormatMac(PhysicalAddress mac) =>
        string.Join(":", mac.GetAddressBytes().Select(b => b.ToString("X2")));
}
