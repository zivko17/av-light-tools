// === AVToolbox — AV / IT toolkit (WinForms). Tool list is data-driven. ===
namespace AVToolbox;

public record MenuItem(string Name, string Desc, Action Action);
public record Section(string Title, MenuItem[] Items);

public static class ToolRegistry
{
    // Add a tool by adding an entry here — the sidebar builds itself.
    public static readonly Section[] Sections =
    {
        new("Networking", new MenuItem[]
        {
            new("netinfo",     "IP, gateway, DNS, MAC, link speed per interface", NetworkTools.NetInfo),
            new("myip",        "Local + public IP",                               NetworkTools.MyIp),
            new("scan",        "Discover hosts in subnet (parallel ping + ARP)",  NetworkTools.Scan),
            new("portcheck",   "Test a single TCP port",                          NetworkTools.PortCheck),
            new("portscan",    "Scan common ports of a host",                     NetworkTools.PortScan),
            new("connections", "Active TCP connections by process",               NetworkTools.Connections),
            new("flushdns",    "Clear DNS cache",                                 NetworkTools.FlushDns),
        }),
        new("System", new MenuItem[]
        {
            new("sysinfo", "Model, CPU, RAM, GPU, serial, OS, uptime", SystemTools.SysInfo),
            new("top",     "Top processes by CPU / RAM",               SystemTools.Top),
            new("disks",   "Physical disks + volume usage",            SystemTools.Disks),
            new("battery", "Battery charge, status, time left",        SystemTools.Battery),
        }),
        new("Audio / Media (FFmpeg)", new MenuItem[]
        {
            new("convert",       "Convert audio (mp3/aac/opus/flac/wav) -> Music", AudioTools.Convert),
            new("extract-audio", "Extract audio track from a video -> Music",      AudioTools.ExtractAudio),
            new("normalize",     "Normalize loudness EBU R128 (-23 LUFS)",         AudioTools.Normalize),
            new("trim",          "Cut a section without re-encoding",              AudioTools.Trim),
            new("probe",         "Show technical info of a media file",            AudioTools.Probe),
        }),
        new("NDI", new MenuItem[]
        {
            new("ndi-check",    "Link speed, mDNS 5353, NDI ports 5960-5990", NdiTools.Check),
            new("ndi-services", "NDI services and running NDI processes",     NdiTools.Services),
            new("ndi-config",   "Show NDI Access Manager configuration",      NdiTools.Config),
        }),
        new("Calculators", new MenuItem[]
        {
            new("calc-subnet",  "Subnet math: CIDR -> range, mask, hosts",    CalcTools.Subnet),
            new("calc-bitrate", "Video bitrate: RAW + NDI estimate, GB/hour", CalcTools.Bitrate),
        }),
    };
}

public static class Program
{
    [STAThread]
    public static void Main()
    {
        Application.SetHighDpiMode(HighDpiMode.SystemAware);
        Application.EnableVisualStyles();
        Application.SetCompatibleTextRenderingDefault(false);
        Application.Run(new MainForm());
    }
}
