// === NetMaint — network plumbing shared by the pages ===
using System.Diagnostics;
using System.Net;
using System.Net.NetworkInformation;
using System.Net.Sockets;
using System.Security.Principal;

namespace NetMaint;

public record HostHit(string Ip, string Mac, string Hostname)
{
    public string Vendor { get; set; } = "";
}

public static class NetHelpers
{
    public static bool IsAdmin()
    {
        using var identity = WindowsIdentity.GetCurrent();
        return new WindowsPrincipal(identity).IsInRole(WindowsBuiltInRole.Administrator);
    }

    /// <summary>Up, non-loopback interfaces that have an IPv4 address.</summary>
    public static List<NetworkInterface> ActiveAdapters() =>
        NetworkInterface.GetAllNetworkInterfaces()
            .Where(n => n.OperationalStatus == OperationalStatus.Up
                     && n.NetworkInterfaceType != NetworkInterfaceType.Loopback
                     && n.GetIPProperties().UnicastAddresses
                         .Any(a => a.Address.AddressFamily == AddressFamily.InterNetwork))
            .OrderBy(n => HasGateway(n) ? 0 : 1)
            .ThenBy(n => n.NetworkInterfaceType == NetworkInterfaceType.Ethernet ? 0 : 1)
            .ToList();

    /// <summary>The adapter that owns the default route (mirrors Resolve-NetIface).</summary>
    public static NetworkInterface? DefaultAdapter() =>
        ActiveAdapters().FirstOrDefault(HasGateway) ?? ActiveAdapters().FirstOrDefault();

    public static bool HasGateway(NetworkInterface n) =>
        n.GetIPProperties().GatewayAddresses
         .Any(g => g.Address.AddressFamily == AddressFamily.InterNetwork
                && !g.Address.Equals(IPAddress.Any));

    public static UnicastIPAddressInformation? Ipv4Of(NetworkInterface nic) =>
        nic.GetIPProperties().UnicastAddresses
           .FirstOrDefault(a => a.Address.AddressFamily == AddressFamily.InterNetwork);

    public static string FormatMac(PhysicalAddress mac)
    {
        var bytes = mac.GetAddressBytes();
        return bytes.Length == 0 ? "-" : string.Join(":", bytes.Select(b => b.ToString("X2")));
    }

    /// <summary>Ping x.y.z.1-254 in parallel; reports 0-254 progress; returns alive IPs with MAC + hostname.</summary>
    public static async Task<List<HostHit>> PingSweepAsync(
        string prefix, IProgress<int> progress, CancellationToken ct)
    {
        int done = 0;
        var alive = new List<string>();
        var tasks = Enumerable.Range(1, 254).Select(async i =>
        {
            string host = $"{prefix}.{i}";
            try
            {
                using var ping = new Ping();
                var reply = await ping.SendPingAsync(host, 900);
                if (reply.Status == IPStatus.Success)
                    lock (alive) alive.Add(host);
            }
            catch { /* unreachable */ }
            finally { progress.Report(Interlocked.Increment(ref done)); }
        });
        await Task.WhenAll(tasks);
        ct.ThrowIfCancellationRequested();

        var hits = new List<HostHit>();
        var resolve = alive.OrderBy(h => int.Parse(h.Split('.')[3])).Select(async host =>
        {
            string mac = Native.ArpMac(IPAddress.Parse(host)) ?? "-";
            string name = "";
            try
            {
                var t = Dns.GetHostEntryAsync(host);
                if (await Task.WhenAny(t, Task.Delay(700, ct)) == t) name = t.Result.HostName;
            }
            catch { /* no PTR record */ }
            lock (hits) hits.Add(new HostHit(host, mac, name));
        });
        await Task.WhenAll(resolve);
        return hits.OrderBy(h => int.Parse(h.Ip.Split('.')[3])).ToList();
    }

    private static readonly HttpClient Http = new() { Timeout = TimeSpan.FromSeconds(6) };

    /// <summary>Vendor from MAC OUI via macvendors.com (rate-limited: call sequentially).</summary>
    public static async Task<string> VendorLookupAsync(string mac)
    {
        var oui = new string(mac.Where(char.IsLetterOrDigit).ToArray());
        if (oui.Length < 6) return "";
        try
        {
            var resp = await Http.GetAsync($"https://api.macvendors.com/{oui[..6]}");
            if (resp.IsSuccessStatusCode) return (await resp.Content.ReadAsStringAsync()).Trim();
            if ((int)resp.StatusCode == 429)
            {
                await Task.Delay(1200);
                var retry = await Http.GetAsync($"https://api.macvendors.com/{oui[..6]}");
                if (retry.IsSuccessStatusCode) return (await retry.Content.ReadAsStringAsync()).Trim();
            }
            return "(unknown)";
        }
        catch { return "(no internet)"; }
    }

    public static async Task<string> PublicIpAsync()
    {
        try { return (await Http.GetStringAsync("https://api.ipify.org")).Trim(); }
        catch { return ""; }
    }

    public static async Task<bool> PortOpenAsync(string host, int port, int timeoutMs = 1200)
    {
        try
        {
            using var client = new TcpClient();
            var connect = client.ConnectAsync(host, port);
            if (await Task.WhenAny(connect, Task.Delay(timeoutMs)) != connect) return false;
            await connect; // surface refusal as exception
            return client.Connected;
        }
        catch { return false; }
    }

    /// <summary>Run a process hidden and capture output. Returns (exitCode, stdout+stderr).</summary>
    public static (int Code, string Output) RunHidden(string exe, params string[] args)
    {
        try
        {
            var psi = new ProcessStartInfo(exe)
            {
                UseShellExecute = false,
                CreateNoWindow = true,
                RedirectStandardOutput = true,
                RedirectStandardError = true
            };
            foreach (var a in args) psi.ArgumentList.Add(a);
            using var p = Process.Start(psi)!;
            string output = p.StandardOutput.ReadToEnd() + p.StandardError.ReadToEnd();
            p.WaitForExit();
            return (p.ExitCode, output);
        }
        catch (Exception ex)
        {
            return (-1, ex.Message);
        }
    }

    public static string? FindExe(string name)
    {
        var exe = name.EndsWith(".exe", StringComparison.OrdinalIgnoreCase) ? name : name + ".exe";
        foreach (var dir in (Environment.GetEnvironmentVariable("PATH") ?? "").Split(';'))
        {
            if (string.IsNullOrWhiteSpace(dir)) continue;
            try
            {
                var full = Path.Combine(dir.Trim(), exe);
                if (File.Exists(full)) return full;
            }
            catch { /* malformed PATH entry */ }
        }
        return null;
    }
}
