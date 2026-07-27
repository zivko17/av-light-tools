// === AVToolbox — system info tools (WMI-based) ===
using System.Diagnostics;
using System.Management;

namespace AVToolbox;

public static class SystemTools
{
    private static IEnumerable<ManagementObject> Wmi(string className, string props = "*")
    {
        using var searcher = new ManagementObjectSearcher($"SELECT {props} FROM {className}");
        foreach (ManagementObject mo in searcher.Get()) yield return mo;
    }

    private static string S(ManagementObject mo, string prop) =>
        mo[prop]?.ToString()?.Trim() ?? "-";

    public static void SysInfo()
    {
        Ui.Header("SYSTEM INFO");
        try
        {
            foreach (var cs in Wmi("Win32_ComputerSystem"))
            {
                Ui.Label("Machine", Environment.MachineName);
                Ui.Label("Model", $"{S(cs, "Manufacturer")} {S(cs, "Model")}");
                double ramGb = Convert.ToDouble(cs["TotalPhysicalMemory"] ?? 0) / (1024.0 * 1024 * 1024);
                Ui.Label("RAM", $"{ramGb:0.#} GB");
            }
            foreach (var bios in Wmi("Win32_BIOS"))
                Ui.Label("Serial", S(bios, "SerialNumber"));
            foreach (var cpu in Wmi("Win32_Processor"))
                Ui.Label("CPU", $"{S(cpu, "Name")}  ({S(cpu, "NumberOfCores")}C/{S(cpu, "NumberOfLogicalProcessors")}T)");
            foreach (var gpu in Wmi("Win32_VideoController"))
                Ui.Label("GPU", S(gpu, "Name"));
            foreach (var os in Wmi("Win32_OperatingSystem"))
            {
                Ui.Label("OS", $"{S(os, "Caption")}  ({S(os, "Version")})");
                var boot = ManagementDateTimeConverter.ToDateTime(S(os, "LastBootUpTime"));
                var up = DateTime.Now - boot;
                Ui.Label("Uptime", $"{(int)up.TotalDays}d {up.Hours}h {up.Minutes}m", ConsoleColor.Green);
            }
        }
        catch (Exception ex)
        {
            Ui.Err($"WMI query failed: {ex.Message}");
        }
    }

    public static void Top()
    {
        Ui.Header("TOP PROCESSES");
        Ui.Hint("Sampling CPU for 1.5 s...");

        var first = new Dictionary<int, TimeSpan>();
        foreach (var p in Process.GetProcesses())
        {
            try { first[p.Id] = p.TotalProcessorTime; } catch { /* access denied */ }
        }
        Thread.Sleep(1500);

        var rows = new List<(string Name, double Cpu, double MemMb)>();
        foreach (var p in Process.GetProcesses())
        {
            try
            {
                double cpu = 0;
                if (first.TryGetValue(p.Id, out var t0))
                    cpu = (p.TotalProcessorTime - t0).TotalMilliseconds / 1500.0
                          / Environment.ProcessorCount * 100.0;
                rows.Add((p.ProcessName, cpu, p.WorkingSet64 / (1024.0 * 1024)));
            }
            catch { /* process exited */ }
        }

        Ui.Blank();
        Ui.Write($"  {"PROCESS",-30}{"CPU %",8}{"RAM MB",10}", ConsoleColor.Yellow);
        foreach (var r in rows.OrderByDescending(r => r.Cpu).Take(15))
        {
            var color = r.Cpu > 50 ? ConsoleColor.Red : r.Cpu > 15 ? ConsoleColor.Yellow : ConsoleColor.White;
            Ui.Write($"  {r.Name,-30}{r.Cpu,8:0.0}{r.MemMb,10:0}", color);
        }
    }

    public static void Disks()
    {
        Ui.Header("DISKS");
        try
        {
            foreach (var d in Wmi("Win32_DiskDrive"))
            {
                double sizeGb = Convert.ToDouble(d["Size"] ?? 0) / 1e9;
                Ui.Write($"  {S(d, "Model")}", ConsoleColor.Cyan, newline: false);
                Ui.Hint($"  {sizeGb:0} GB  {S(d, "InterfaceType")}");
            }
        }
        catch { /* WMI unavailable */ }

        Ui.Blank();
        foreach (var v in DriveInfo.GetDrives().Where(v => v.IsReady && v.DriveType == DriveType.Fixed))
        {
            double used = v.TotalSize - v.TotalFreeSpace;
            double frac = used / v.TotalSize;
            var color = frac > 0.9 ? ConsoleColor.Red : frac > 0.75 ? ConsoleColor.Yellow : ConsoleColor.Green;
            Ui.Write($"  {v.Name,-4}{Ui.Bar(frac)} ", color, newline: false);
            Ui.Plain($"{frac * 100:0}%  {Ui.HumanSize(used)} / {Ui.HumanSize(v.TotalSize)}  ({v.VolumeLabel})");
        }
    }

    public static void Battery()
    {
        Ui.Header("BATTERY");
        try
        {
            bool found = false;
            foreach (var b in Wmi("Win32_Battery"))
            {
                found = true;
                int charge = Convert.ToInt32(b["EstimatedChargeRemaining"] ?? 0);
                var color = charge < 20 ? ConsoleColor.Red : charge < 50 ? ConsoleColor.Yellow : ConsoleColor.Green;
                Ui.Write($"  {Ui.Bar(charge / 100.0)} ", color, newline: false);
                Ui.Write($"{charge}%", color);

                int status = Convert.ToInt32(b["BatteryStatus"] ?? 0);
                string statusText = status switch
                {
                    1 => "Discharging",
                    2 => "On AC power",
                    3 => "Fully charged",
                    4 => "Low",
                    5 => "Critical",
                    6 or 7 or 8 or 9 => "Charging",
                    _ => $"Status {status}"
                };
                Ui.Label("Status", statusText);

                int runtime = Convert.ToInt32(b["EstimatedRunTime"] ?? 0);
                if (status == 1 && runtime > 0 && runtime < 10000)
                    Ui.Label("Time left", $"{runtime / 60}h {runtime % 60}m");
            }
            if (!found) Ui.Hint("No battery detected (desktop PC).");
        }
        catch (Exception ex)
        {
            Ui.Err($"WMI query failed: {ex.Message}");
        }
    }
}
