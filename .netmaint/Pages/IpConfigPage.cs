// === NetMaint — IP Config page (SetIP.ps1 profiles + custom, flushdns, reset-nic) ===
using System.Net;

namespace NetMaint.Pages;

public class IpConfigPage : UserControl
{
    // Same profiles as SetIP.ps1
    private static readonly (string Name, string Ip, string Mask, string Gw)[] Profiles =
    {
        ("Profile 0", "192.168.0.99", "255.255.255.0", "192.168.0.1"),
        ("Profile 1", "192.168.1.99", "255.255.255.0", "192.168.1.1"),
    };

    private readonly ComboBox _iface;
    private readonly TextBox _custom;
    private readonly TextBox _gateway;
    private readonly Label _status;
    private readonly Label _current;
    private readonly bool _isAdmin = NetHelpers.IsAdmin();

    public IpConfigPage()
    {
        BackColor = Theme.Bg;

        var heading = Theme.Heading("IP Configuration");
        heading.Location = new Point(20, 16);

        var ifLbl = Theme.Label("Interface:", dim: true);
        ifLbl.Location = new Point(20, 62);
        _iface = new ComboBox
        {
            Location = new Point(90, 58),
            Width = 240,
            DropDownStyle = ComboBoxStyle.DropDownList,
            BackColor = Color.FromArgb(14, 14, 18),
            ForeColor = Theme.Text,
            FlatStyle = FlatStyle.Flat,
            Font = Theme.Base
        };
        _iface.SelectedIndexChanged += (_, _) => ShowCurrent();

        var profLbl = Theme.SectionLabel("Profiles");
        profLbl.Location = new Point(20, 106);

        int x = 20;
        foreach (var p in Profiles)
        {
            var btn = Theme.Button($"{p.Name}  ({p.Ip})", 190);
            btn.Location = new Point(x, 134);
            var profile = p;
            btn.Click += (_, _) => Apply(() => SetStatic(profile.Ip, profile.Mask, profile.Gw),
                                        $"{profile.Name}: {profile.Ip}");
            Controls.Add(btn);
            x += 200;
        }
        var dhcpBtn = Theme.Button("DHCP (automatic)", 160);
        dhcpBtn.Location = new Point(x, 134);
        dhcpBtn.Click += (_, _) => Apply(SetDhcp, "DHCP");
        Controls.Add(dhcpBtn);

        var customLbl = Theme.SectionLabel("Custom static IP");
        customLbl.Location = new Point(20, 190);

        var ipLbl = Theme.Label("IP/CIDR:", dim: true);
        ipLbl.Location = new Point(20, 222);
        _custom = Theme.TextBox(170);
        _custom.Location = new Point(85, 218);
        _custom.PlaceholderText = "192.168.1.25/24";

        var gwLbl = Theme.Label("Gateway (optional):", dim: true);
        gwLbl.Location = new Point(275, 222);
        _gateway = Theme.TextBox(140);
        _gateway.Location = new Point(400, 218);

        var applyBtn = Theme.AccentButton("Apply", 90);
        applyBtn.Location = new Point(555, 215);
        applyBtn.Click += (_, _) => ApplyCustom();
        Controls.Add(applyBtn);

        var utilLbl = Theme.SectionLabel("Utilities");
        utilLbl.Location = new Point(20, 274);

        var flushBtn = Theme.Button("Flush DNS cache", 150);
        flushBtn.Location = new Point(20, 302);
        flushBtn.Click += (_, _) => FlushDns();
        Controls.Add(flushBtn);

        var resetBtn = Theme.Button("Restart adapter", 150);
        resetBtn.Location = new Point(180, 302);
        resetBtn.Click += async (_, _) => await ResetNicAsync();
        Controls.Add(resetBtn);

        _status = Theme.Label("");
        _status.Location = new Point(20, 350);
        _status.Font = Theme.Semibold;
        _status.MaximumSize = new Size(800, 0);

        var curCap = Theme.SectionLabel("Current configuration");
        curCap.Location = new Point(20, 396);
        _current = Theme.Label("", dim: true);
        _current.Location = new Point(20, 424);
        _current.Font = Theme.Mono;

        if (!_isAdmin)
        {
            var warn = Theme.Label("Changing the IP needs administrator rights — use the button in the sidebar.");
            warn.ForeColor = Theme.WarnCol;
            warn.Location = new Point(20, 90);
            Controls.Add(warn);
        }

        Controls.Add(heading);
        Controls.Add(ifLbl);
        Controls.Add(_iface);
        Controls.Add(profLbl);
        Controls.Add(customLbl);
        Controls.Add(ipLbl);
        Controls.Add(_custom);
        Controls.Add(gwLbl);
        Controls.Add(_gateway);
        Controls.Add(utilLbl);
        Controls.Add(_status);
        Controls.Add(curCap);
        Controls.Add(_current);

        Load += (_, _) => FillAdapters();
    }

    private void FillAdapters()
    {
        _iface.Items.Clear();
        var def = NetHelpers.DefaultAdapter();
        foreach (var nic in NetHelpers.ActiveAdapters())
            _iface.Items.Add(nic.Name);
        if (def != null) _iface.SelectedItem = def.Name;
        else if (_iface.Items.Count > 0) _iface.SelectedIndex = 0;
    }

    private string Iface => _iface.SelectedItem?.ToString() ?? "Ethernet";

    private void ShowCurrent()
    {
        var nic = NetHelpers.ActiveAdapters().FirstOrDefault(n => n.Name == Iface);
        if (nic == null) { _current.Text = "(adapter not found)"; return; }
        var ip = NetHelpers.Ipv4Of(nic);
        var props = nic.GetIPProperties();
        var gw = props.GatewayAddresses.FirstOrDefault(
            g => g.Address.AddressFamily == System.Net.Sockets.AddressFamily.InterNetwork);
        bool dhcp = false;
        try { dhcp = props.GetIPv4Properties()?.IsDhcpEnabled ?? false; } catch { }
        _current.Text =
            $"IP:      {(ip != null ? $"{ip.Address} / {ip.IPv4Mask}" : "(none)")}\n" +
            $"Gateway: {gw?.Address.ToString() ?? "(none)"}\n" +
            $"Mode:    {(dhcp ? "DHCP (automatic)" : "Manual (static)")}";
    }

    private void SetStatus(string text, Color color)
    {
        _status.Text = text;
        _status.ForeColor = color;
    }

    private bool RequireAdmin()
    {
        if (_isAdmin) return true;
        SetStatus("Administrator rights required — restart via the sidebar button.", Theme.WarnCol);
        return false;
    }

    private void Apply(Func<(bool Ok, string Detail)> action, string what)
    {
        if (!RequireAdmin()) return;
        SetStatus($"Applying {what} on '{Iface}' ...", Theme.TextDim);
        var (ok, detail) = action();
        SetStatus(ok ? $"OK — {what}" : $"ERROR — {detail}", ok ? Theme.Good : Theme.Bad);
        ShowCurrent();
    }

    private (bool, string) SetStatic(string ip, string mask, string? gw)
    {
        var args = new List<string> { "interface", "ip", "set", "address", $"name={Iface}", "static", ip, mask };
        if (!string.IsNullOrEmpty(gw)) args.Add(gw);
        var (code, output) = NetHelpers.RunHidden("netsh", args.ToArray());
        NetHelpers.RunHidden("netsh", "interface", "ip", "set", "dns", $"name={Iface}", "source=dhcp");
        return (code == 0, output.Trim());
    }

    private (bool, string) SetDhcp()
    {
        var (code, output) = NetHelpers.RunHidden("netsh",
            "interface", "ip", "set", "address", $"name={Iface}", "source=dhcp");
        NetHelpers.RunHidden("netsh", "interface", "ip", "set", "dns", $"name={Iface}", "source=dhcp");
        // "DHCP is already enabled" also comes back as code 0 on some builds; treat both as fine
        return (code == 0 || output.Contains("already enabled", StringComparison.OrdinalIgnoreCase),
                output.Trim());
    }

    private void ApplyCustom()
    {
        string entry = _custom.Text.Trim();
        var m = System.Text.RegularExpressions.Regex.Match(entry, @"^(\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3})/(\d{1,2})$");
        if (!m.Success || !IPAddress.TryParse(m.Groups[1].Value, out _)
            || !int.TryParse(m.Groups[2].Value, out int cidr) || cidr > 32)
        {
            SetStatus("Invalid IP/CIDR — e.g. 192.168.1.25/24", Theme.WarnCol);
            return;
        }
        string gw = _gateway.Text.Trim();
        if (gw.Length > 0 && !IPAddress.TryParse(gw, out _))
        {
            SetStatus("Invalid gateway address.", Theme.WarnCol);
            return;
        }

        string mask = CidrToMask(cidr);
        Apply(() => SetStatic(m.Groups[1].Value, mask, gw.Length > 0 ? gw : null),
              $"{entry}" + (gw.Length > 0 ? $" GW {gw}" : ""));
    }

    private static string CidrToMask(int prefix)
    {
        var octets = new int[4];
        for (int i = 0; i < 4; i++)
        {
            int bits = Math.Min(8, Math.Max(0, prefix - 8 * i));
            octets[i] = 256 - (1 << (8 - bits));
        }
        return string.Join(".", octets);
    }

    private void FlushDns()
    {
        bool ok = Native.FlushDns();
        if (!ok) ok = NetHelpers.RunHidden("ipconfig", "/flushdns").Code == 0;
        SetStatus(ok ? "OK — DNS cache cleared" : "Failed to flush DNS cache", ok ? Theme.Good : Theme.Bad);
    }

    private async Task ResetNicAsync()
    {
        if (!RequireAdmin()) return;
        string iface = Iface;
        SetStatus($"Restarting adapter '{iface}' ...", Theme.TextDim);
        var result = await Task.Run(() =>
        {
            var (c1, o1) = NetHelpers.RunHidden("netsh", "interface", "set", "interface", iface, "admin=disabled");
            if (c1 != 0) return (false, o1);
            Thread.Sleep(2000);
            var (c2, o2) = NetHelpers.RunHidden("netsh", "interface", "set", "interface", iface, "admin=enabled");
            return (c2 == 0, o2);
        });
        SetStatus(result.Item1 ? $"OK — adapter '{iface}' restarted" : $"ERROR — {result.Item2.Trim()}",
                  result.Item1 ? Theme.Good : Theme.Bad);
        ShowCurrent();
    }
}
