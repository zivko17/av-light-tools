// === NetMaint — Network Info page (netinfo + myip): adapter cards + public IP ===
using System.Net.NetworkInformation;
using System.Net.Sockets;

namespace NetMaint.Pages;

public class NetworkInfoPage : UserControl
{
    private readonly FlowLayoutPanel _cards;
    private readonly Label _publicIp;
    private readonly Button _refresh;

    public NetworkInfoPage()
    {
        BackColor = Theme.Bg;
        Padding = new Padding(18);

        var heading = Theme.Heading("Network Info");
        heading.Location = new Point(20, 16);

        _refresh = Theme.AccentButton("Refresh", 110);
        _refresh.Location = new Point(20, 52);
        _refresh.Click += async (_, _) => await RefreshInfoAsync();

        _publicIp = Theme.Label("Public IP: ...", dim: true);
        _publicIp.Location = new Point(150, 60);
        _publicIp.Font = Theme.Semibold;

        _cards = new FlowLayoutPanel
        {
            Location = new Point(20, 96),
            Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right | AnchorStyles.Bottom,
            AutoScroll = true,
            WrapContents = true,
            FlowDirection = FlowDirection.LeftToRight
        };

        Controls.Add(heading);
        Controls.Add(_refresh);
        Controls.Add(_publicIp);
        Controls.Add(_cards);
        Resize += (_, _) => _cards.Size = new Size(Width - 40, Height - 116);
        _cards.Size = new Size(Width - 40, Height - 116);

        Load += async (_, _) => await RefreshInfoAsync();
    }

    private async Task RefreshInfoAsync()
    {
        _refresh.Enabled = false;
        _cards.Controls.Clear();

        foreach (var nic in NetHelpers.ActiveAdapters())
            _cards.Controls.Add(BuildCard(nic));

        _publicIp.Text = "Public IP: resolving...";
        _publicIp.ForeColor = Theme.TextDim;
        string pub = await NetHelpers.PublicIpAsync();
        if (pub.Length > 0)
        {
            _publicIp.Text = $"Public IP: {pub}";
            _publicIp.ForeColor = Theme.Good;
        }
        else
        {
            _publicIp.Text = "Public IP: unreachable (no internet?)";
            _publicIp.ForeColor = Theme.Bad;
        }
        _refresh.Enabled = true;
    }

    private static Panel BuildCard(NetworkInterface nic)
    {
        var card = Theme.CardPanel();
        card.Size = new Size(420, 210);
        card.Margin = new Padding(0, 0, 14, 14);

        var props = nic.GetIPProperties();
        var ip = NetHelpers.Ipv4Of(nic);
        var gw = props.GatewayAddresses
                      .FirstOrDefault(g => g.Address.AddressFamily == AddressFamily.InterNetwork);
        var dns = props.DnsAddresses
                       .Where(d => d.AddressFamily == AddressFamily.InterNetwork)
                       .Select(d => d.ToString()).ToList();
        bool dhcp = false;
        try { dhcp = props.GetIPv4Properties()?.IsDhcpEnabled ?? false; } catch { }

        long mbps = nic.Speed > 0 ? nic.Speed / 1_000_000 : 0;
        Color speedColor = mbps >= 1000 ? Theme.Good : mbps >= 100 ? Theme.WarnCol : Theme.Bad;
        bool isDefault = NetHelpers.HasGateway(nic);

        var name = new Label
        {
            Text = nic.Name + (isDefault ? "   (default route)" : ""),
            ForeColor = Theme.Accent,
            Font = Theme.Semibold,
            AutoSize = true,
            Location = new Point(14, 10)
        };
        var desc = new Label
        {
            Text = nic.Description,
            ForeColor = Theme.TextDim,
            AutoSize = true,
            Location = new Point(14, 32),
            Font = new Font("Segoe UI", 8.5f)
        };
        card.Controls.Add(name);
        card.Controls.Add(desc);

        int y = 60;
        void Row(string label, string value, Color? color = null)
        {
            var l = new Label { Text = label, ForeColor = Theme.TextDim, Location = new Point(14, y), AutoSize = true };
            var v = new Label { Text = value, ForeColor = color ?? Theme.Text, Location = new Point(120, y), AutoSize = true, MaximumSize = new Size(290, 0) };
            card.Controls.Add(l);
            card.Controls.Add(v);
            y += 24;
        }

        Row("IP", ip != null ? $"{ip.Address} / {ip.IPv4Mask}" : "(no address)");
        Row("Gateway", gw?.Address.ToString() ?? "(none)");
        Row("DNS", dns.Count > 0 ? string.Join(", ", dns) : "(automatic)");
        Row("MAC", NetHelpers.FormatMac(nic.GetPhysicalAddress()));
        Row("Link speed", mbps > 0 ? $"{mbps} Mbps" : "-", speedColor);
        Row("Mode", dhcp ? "DHCP (automatic)" : "Manual (static)", dhcp ? Theme.Good : Theme.WarnCol);

        return card;
    }
}
