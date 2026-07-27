// === AVToolbox — NDI diagnostics (ports, services, config) ===
using System.Diagnostics;
using System.Net.NetworkInformation;
using System.ServiceProcess;
using System.Text.Json;

namespace AVToolbox;

public static class NdiTools
{
    public static void Check()
    {
        Ui.Header("NDI NETWORK CHECK");

        // Link speed — NDI wants gigabit
        var nic = NetworkInterface.GetAllNetworkInterfaces()
            .FirstOrDefault(n => n.OperationalStatus == OperationalStatus.Up
                              && n.NetworkInterfaceType == NetworkInterfaceType.Ethernet);
        if (nic != null)
        {
            long mbps = nic.Speed / 1_000_000;
            Ui.Label("Ethernet link", $"{mbps} Mbps",
                mbps >= 1000 ? ConsoleColor.Green : ConsoleColor.Red);
            if (mbps < 1000) Ui.Warn("NDI needs gigabit — check cable/switch.");
        }
        else
        {
            Ui.Warn("No Ethernet interface up (WiFi is not recommended for NDI).");
        }
        Ui.Blank();

        // Listening ports: mDNS 5353 (discovery) and 5960-5990 (streams)
        var ipProps = IPGlobalProperties.GetIPGlobalProperties();
        var udp = ipProps.GetActiveUdpListeners();
        var tcp = ipProps.GetActiveTcpListeners();

        bool mdns = udp.Any(e => e.Port == 5353);
        Ui.Label("mDNS 5353/udp", mdns ? "listening (discovery OK)" : "NOT listening",
            mdns ? ConsoleColor.Green : ConsoleColor.Red);

        var ndiPorts = tcp.Where(e => e.Port >= 5960 && e.Port <= 5990)
                          .Select(e => e.Port).Distinct().OrderBy(p => p).ToList();
        Ui.Label("TCP 5960-5990", ndiPorts.Count > 0
                ? string.Join(", ", ndiPorts) + "  (NDI active)"
                : "none listening (no NDI source/receiver running)",
            ndiPorts.Count > 0 ? ConsoleColor.Green : ConsoleColor.DarkGray);

        Ui.Blank();
        Ui.Hint("If discovery fails across machines: check firewall rules for mDNS");
        Ui.Hint("and that all devices are in the same subnet / NDI groups.");
    }

    public static void Services()
    {
        Ui.Header("NDI SERVICES & PROCESSES");
        try
        {
            var services = ServiceController.GetServices()
                .Where(s => s.DisplayName.Contains("NDI", StringComparison.OrdinalIgnoreCase))
                .ToList();
            if (services.Count == 0) Ui.Hint("No NDI services installed.");
            foreach (var s in services)
            {
                bool running = s.Status == ServiceControllerStatus.Running;
                Ui.Write($"  {s.DisplayName,-40}", ConsoleColor.White, newline: false);
                Ui.Write(s.Status.ToString(), running ? ConsoleColor.Green : ConsoleColor.Red);
            }
        }
        catch (Exception ex)
        {
            Ui.Err($"Could not query services: {ex.Message}");
        }

        Ui.Blank();
        var procs = Process.GetProcesses()
            .Where(p => p.ProcessName.Contains("NDI", StringComparison.OrdinalIgnoreCase))
            .OrderBy(p => p.ProcessName)
            .ToList();
        if (procs.Count == 0) Ui.Hint("No NDI processes running.");
        foreach (var p in procs)
            Ui.Ok($"{p.ProcessName}  (pid {p.Id})");
    }

    public static void Config()
    {
        Ui.Header("NDI CONFIG");
        string path = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
            "NDI", "ndi-config.v1.json");

        if (!File.Exists(path))
        {
            Ui.Hint($"Config not found: {path}");
            Ui.Hint("Run NDI Access Manager once to create it.");
            return;
        }

        try
        {
            using var doc = JsonDocument.Parse(File.ReadAllText(path));
            Ui.Hint(path);
            Ui.Blank();
            Ui.Plain(JsonSerializer.Serialize(doc.RootElement,
                new JsonSerializerOptions { WriteIndented = true }));
        }
        catch (Exception ex)
        {
            Ui.Err($"Could not parse config: {ex.Message}");
        }
    }
}
