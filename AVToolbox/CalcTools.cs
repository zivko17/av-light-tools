// === AVToolbox — AV/IT calculators ===
using System.Net;

namespace AVToolbox;

public static class CalcTools
{
    public static void Subnet()
    {
        Ui.Header("SUBNET CALCULATOR");
        string input = Ui.Ask("CIDR (e.g. 192.168.1.0/24)");
        var parts = input.Split('/');
        if (parts.Length != 2
            || !IPAddress.TryParse(parts[0], out var ip)
            || !int.TryParse(parts[1], out int prefix)
            || prefix < 0 || prefix > 32)
        {
            Ui.Err("Invalid CIDR notation.");
            return;
        }

        uint addr = ToUInt(ip);
        uint mask = prefix == 0 ? 0 : uint.MaxValue << (32 - prefix);
        uint network = addr & mask;
        uint broadcast = network | ~mask;
        long hosts = prefix >= 31 ? (prefix == 31 ? 2 : 1) : (1L << (32 - prefix)) - 2;

        Ui.Label("Network", $"{FromUInt(network)}/{prefix}");
        Ui.Label("Mask", FromUInt(mask).ToString());
        Ui.Label("Broadcast", FromUInt(broadcast).ToString());
        if (prefix <= 30)
        {
            Ui.Label("First host", FromUInt(network + 1).ToString());
            Ui.Label("Last host", FromUInt(broadcast - 1).ToString());
        }
        Ui.Label("Usable hosts", hosts.ToString("N0"), ConsoleColor.Green);
    }

    public static void Bitrate()
    {
        Ui.Header("VIDEO BITRATE CALCULATOR");
        if (!int.TryParse(Ui.Ask("Width (px)"), out int w) ||
            !int.TryParse(Ui.Ask("Height (px)"), out int h) ||
            !double.TryParse(Ui.Ask("Frame rate (fps)"), out double fps))
        {
            Ui.Err("Invalid input.");
            return;
        }

        // RAW: 8-bit 4:2:2 (16 bpp) and 10-bit RGB (30 bpp)
        double raw422 = (double)w * h * fps * 16 / 1e6;
        double rawRgb10 = (double)w * h * fps * 30 / 1e6;
        // NDI full bandwidth ≈ 1.06 bits/pixel (1080p60 ≈ 132 Mb/s); NDI HX ≈ 1/8 of that
        double ndi = (double)w * h * fps * 1.06 / 1e6;
        double ndiHx = ndi / 8;

        Ui.Blank();
        Ui.Label("RAW 4:2:2 8-bit", $"{raw422:N0} Mb/s");
        Ui.Label("RAW RGB 10-bit", $"{rawRgb10:N0} Mb/s");
        Ui.Label("NDI (full)", $"{ndi:N0} Mb/s  ({ndi * 3600 / 8 / 1000:N0} GB/hour)", ConsoleColor.Green);
        Ui.Label("NDI HX (approx)", $"{ndiHx:N0} Mb/s  ({ndiHx * 3600 / 8 / 1000:N1} GB/hour)", ConsoleColor.Green);
        Ui.Blank();
        Ui.Hint("NDI values are estimates — actual bandwidth varies with content.");
    }

    private static uint ToUInt(IPAddress ip)
    {
        var b = ip.GetAddressBytes();
        return ((uint)b[0] << 24) | ((uint)b[1] << 16) | ((uint)b[2] << 8) | b[3];
    }

    private static IPAddress FromUInt(uint v) =>
        new(new[] { (byte)(v >> 24), (byte)(v >> 16), (byte)(v >> 8), (byte)v });
}
