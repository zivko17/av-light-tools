// === NetMaint — Win32 interop: ARP, recycle bin, DNS cache ===
using System.Runtime.InteropServices;

namespace NetMaint;

public static class Native
{
    // --- ARP: resolve MAC of an IPv4 host on the local subnet ---
    [DllImport("iphlpapi.dll", ExactSpelling = true)]
    private static extern int SendARP(uint destIP, uint srcIP, byte[] macAddr, ref uint macAddrLen);

    public static string? ArpMac(System.Net.IPAddress ip)
    {
        try
        {
            uint dest = BitConverter.ToUInt32(ip.GetAddressBytes(), 0);
            var mac = new byte[6];
            uint len = 6;
            if (SendARP(dest, 0, mac, ref len) != 0 || len == 0) return null;
            return string.Join(":", mac.Take((int)len).Select(b => b.ToString("X2")));
        }
        catch { return null; }
    }

    // --- Recycle bin ---
    [StructLayout(LayoutKind.Sequential, Pack = 4)]
    private struct SHQUERYRBINFO
    {
        public int cbSize;
        public long i64Size;
        public long i64NumItems;
    }

    [DllImport("shell32.dll", CharSet = CharSet.Unicode)]
    private static extern int SHQueryRecycleBin(string? pszRootPath, ref SHQUERYRBINFO info);

    [DllImport("shell32.dll", CharSet = CharSet.Unicode)]
    private static extern int SHEmptyRecycleBin(IntPtr hwnd, string? pszRootPath, uint flags);

    private const uint SHERB_NOCONFIRMATION = 0x1;
    private const uint SHERB_NOPROGRESSUI   = 0x2;
    private const uint SHERB_NOSOUND        = 0x4;

    public static (long Bytes, long Items) RecycleBinSize()
    {
        var info = new SHQUERYRBINFO { cbSize = Marshal.SizeOf<SHQUERYRBINFO>() };
        return SHQueryRecycleBin(null, ref info) == 0 ? (info.i64Size, info.i64NumItems) : (0, 0);
    }

    /// <summary>Returns true if the bin was emptied (or already empty).</summary>
    public static bool EmptyRecycleBin()
    {
        int hr = SHEmptyRecycleBin(IntPtr.Zero, null,
            SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND);
        return hr == 0 || hr == -2147418113 /* already empty */;
    }

    // --- DNS resolver cache ---
    [DllImport("dnsapi.dll", EntryPoint = "DnsFlushResolverCache")]
    private static extern uint DnsFlushResolverCache();

    public static bool FlushDns()
    {
        try { return DnsFlushResolverCache() != 0; }
        catch { return false; }
    }
}
