// === NetMaint — Subnet Scan page (scan + whoison): sweep, MAC, hostname, vendor ===
namespace NetMaint.Pages;

public class ScanPage : UserControl
{
    private readonly TextBox _subnet;
    private readonly Button _scan;
    private readonly Button _vendors;
    private readonly ProgressBar _progress;
    private readonly Label _status;
    private readonly DataGridView _grid;
    private CancellationTokenSource? _cts;

    public ScanPage()
    {
        BackColor = Theme.Bg;

        var heading = Theme.Heading("Subnet Scan");
        heading.Location = new Point(20, 16);

        var subnetLbl = Theme.Label("Subnet:", dim: true);
        subnetLbl.Location = new Point(20, 62);
        _subnet = Theme.TextBox(140);
        _subnet.Location = new Point(80, 58);
        _subnet.PlaceholderText = "192.168.1";

        _scan = Theme.AccentButton("Scan", 100);
        _scan.Location = new Point(235, 55);
        _scan.Click += async (_, _) => await ScanAsync();

        _vendors = Theme.Button("Lookup vendors", 130);
        _vendors.Location = new Point(345, 55);
        _vendors.Enabled = false;
        _vendors.Click += async (_, _) => await LookupVendorsAsync();

        _progress = new ProgressBar
        {
            Location = new Point(490, 60),
            Size = new Size(220, 22),
            Maximum = 254,
            Visible = false
        };

        _status = Theme.Label("", dim: true);
        _status.Location = new Point(20, 94);

        _grid = Theme.Grid();
        _grid.Location = new Point(20, 120);
        _grid.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right | AnchorStyles.Bottom;
        _grid.Columns.Add("ip", "IP");
        _grid.Columns.Add("mac", "MAC");
        _grid.Columns.Add("host", "Hostname");
        _grid.Columns.Add("vendor", "Vendor");
        _grid.Columns["ip"]!.FillWeight = 18;
        _grid.Columns["mac"]!.FillWeight = 24;
        _grid.Columns["host"]!.FillWeight = 30;
        _grid.Columns["vendor"]!.FillWeight = 28;

        Controls.Add(heading);
        Controls.Add(subnetLbl);
        Controls.Add(_subnet);
        Controls.Add(_scan);
        Controls.Add(_vendors);
        Controls.Add(_progress);
        Controls.Add(_status);
        Controls.Add(_grid);

        Resize += (_, _) => _grid.Size = new Size(Width - 40, Height - 140);
        _grid.Size = new Size(Width - 40, Height - 140);

        Load += (_, _) => AutoFillSubnet();
    }

    private void AutoFillSubnet()
    {
        var nic = NetHelpers.DefaultAdapter();
        var ip = nic != null ? NetHelpers.Ipv4Of(nic) : null;
        if (ip != null)
        {
            var o = ip.Address.GetAddressBytes();
            _subnet.Text = $"{o[0]}.{o[1]}.{o[2]}";
        }
    }

    private async Task ScanAsync()
    {
        string prefix = _subnet.Text.Trim().TrimEnd('.');
        var parts = prefix.Split('.');
        if (parts.Length != 3 || parts.Any(p => !int.TryParse(p, out int v) || v < 0 || v > 255))
        {
            _status.Text = "Invalid subnet — use three octets, e.g. 192.168.1";
            _status.ForeColor = Theme.Bad;
            return;
        }

        _cts = new CancellationTokenSource();
        _scan.Enabled = false;
        _vendors.Enabled = false;
        _grid.Rows.Clear();
        _progress.Value = 0;
        _progress.Visible = true;
        _status.Text = $"Scanning {prefix}.1-254 ...";
        _status.ForeColor = Theme.TextDim;

        try
        {
            var progress = new Progress<int>(done => _progress.Value = Math.Min(done, 254));
            var myIp = NetHelpers.DefaultAdapter() is { } nic ? NetHelpers.Ipv4Of(nic)?.Address.ToString() : null;

            var hits = await NetHelpers.PingSweepAsync(prefix, progress, _cts.Token);

            foreach (var h in hits)
            {
                int row = _grid.Rows.Add(h.Ip, h.Mac, h.Hostname + (h.Ip == myIp ? "  (this PC)" : ""), "");
                _grid.Rows[row].Cells[0].Style.ForeColor = Theme.Good;
            }
            _status.Text = $"{hits.Count} hosts up.";
            _status.ForeColor = Theme.Good;
            _vendors.Enabled = hits.Count > 0;
        }
        catch (OperationCanceledException)
        {
            _status.Text = "Scan cancelled.";
            _status.ForeColor = Theme.WarnCol;
        }
        finally
        {
            _progress.Visible = false;
            _scan.Enabled = true;
        }
    }

    private async Task LookupVendorsAsync()
    {
        _vendors.Enabled = false;
        // macvendors.com rate-limits — go one by one, visibly filling the column.
        foreach (DataGridViewRow row in _grid.Rows)
        {
            string mac = row.Cells[1].Value?.ToString() ?? "-";
            if (mac == "-" || (row.Cells[3].Value?.ToString()?.Length ?? 0) > 0) continue;
            row.Cells[3].Value = "...";
            string vendor = await NetHelpers.VendorLookupAsync(mac);
            row.Cells[3].Value = vendor;
            row.Cells[3].Style.ForeColor = vendor.StartsWith('(') ? Theme.TextDim : Theme.Text;
            await Task.Delay(600); // stay under the free-tier rate limit
        }
        _vendors.Enabled = true;
    }
}
