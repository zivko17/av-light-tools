// === NetMaint — Ports page (portcheck + portscan) ===
namespace NetMaint.Pages;

public class PortsPage : UserControl
{
    // Same list as NetTools.ps1 portscan
    private static readonly (int Port, string Svc)[] CommonPorts =
    {
        (21, "FTP"), (22, "SSH"), (23, "Telnet"), (53, "DNS"), (80, "HTTP"),
        (123, "NTP"), (161, "SNMP"), (443, "HTTPS"), (445, "SMB"), (3389, "RDP"),
        (5353, "mDNS/Bonjour"), (8000, "HTTP-Alt"), (8080, "HTTP-Proxy"), (8443, "HTTPS-Alt")
    };

    private readonly TextBox _host;
    private readonly TextBox _port;
    private readonly Button _check;
    private readonly Button _scanAll;
    private readonly Label _checkResult;
    private readonly DataGridView _grid;

    public PortsPage()
    {
        BackColor = Theme.Bg;

        var heading = Theme.Heading("Ports");
        heading.Location = new Point(20, 16);

        var hostLbl = Theme.Label("Host / IP:", dim: true);
        hostLbl.Location = new Point(20, 62);
        _host = Theme.TextBox(170);
        _host.Location = new Point(90, 58);

        var single = Theme.SectionLabel("Single port check");
        single.Location = new Point(20, 104);

        var portLbl = Theme.Label("Port:", dim: true);
        portLbl.Location = new Point(20, 136);
        _port = Theme.TextBox(80);
        _port.Location = new Point(60, 132);

        _check = Theme.AccentButton("Check", 90);
        _check.Location = new Point(155, 129);
        _check.Click += async (_, _) => await CheckSingleAsync();

        _checkResult = Theme.Label("");
        _checkResult.Location = new Point(260, 136);
        _checkResult.Font = Theme.Semibold;

        var scanLbl = Theme.SectionLabel("Common ports scan");
        scanLbl.Location = new Point(20, 184);

        _scanAll = Theme.AccentButton("Scan common ports", 160);
        _scanAll.Location = new Point(20, 212);
        _scanAll.Click += async (_, _) => await ScanCommonAsync();

        _grid = Theme.Grid();
        _grid.Location = new Point(20, 256);
        _grid.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right | AnchorStyles.Bottom;
        _grid.Columns.Add("port", "Port");
        _grid.Columns.Add("status", "Status");
        _grid.Columns.Add("svc", "Service");
        _grid.Columns["port"]!.FillWeight = 15;
        _grid.Columns["status"]!.FillWeight = 20;
        _grid.Columns["svc"]!.FillWeight = 65;

        Controls.Add(heading);
        Controls.Add(hostLbl);
        Controls.Add(_host);
        Controls.Add(single);
        Controls.Add(portLbl);
        Controls.Add(_port);
        Controls.Add(_check);
        Controls.Add(_checkResult);
        Controls.Add(scanLbl);
        Controls.Add(_scanAll);
        Controls.Add(_grid);

        Resize += (_, _) => _grid.Size = new Size(Width - 40, Height - 276);
        _grid.Size = new Size(Width - 40, Height - 276);
    }

    private bool ValidateHost(out string host)
    {
        host = _host.Text.Trim();
        if (host.Length == 0)
        {
            _checkResult.Text = "Enter a host first";
            _checkResult.ForeColor = Theme.WarnCol;
            return false;
        }
        return true;
    }

    private async Task CheckSingleAsync()
    {
        if (!ValidateHost(out string host)) return;
        if (!int.TryParse(_port.Text.Trim(), out int port) || port < 1 || port > 65535)
        {
            _checkResult.Text = "Invalid port";
            _checkResult.ForeColor = Theme.WarnCol;
            return;
        }

        _check.Enabled = false;
        _checkResult.Text = $"Testing {host}:{port} ...";
        _checkResult.ForeColor = Theme.TextDim;

        bool open = await NetHelpers.PortOpenAsync(host, port, 2000);
        _checkResult.Text = open ? $"OPEN — {host}:{port} responds" : $"CLOSED / no response — {host}:{port}";
        _checkResult.ForeColor = open ? Theme.Good : Theme.Bad;
        _check.Enabled = true;
    }

    private async Task ScanCommonAsync()
    {
        if (!ValidateHost(out string host)) return;

        _scanAll.Enabled = false;
        _grid.Rows.Clear();

        var tasks = CommonPorts
            .Select(async p => (p.Port, p.Svc, Open: await NetHelpers.PortOpenAsync(host, p.Port, 1000)))
            .ToList();
        var results = await Task.WhenAll(tasks);

        foreach (var r in results.OrderBy(r => r.Port))
        {
            int row = _grid.Rows.Add(r.Port, r.Open ? "OPEN" : "closed", r.Svc);
            var color = r.Open ? Theme.Good : Theme.TextDim;
            foreach (DataGridViewCell cell in _grid.Rows[row].Cells) cell.Style.ForeColor = color;
        }
        _scanAll.Enabled = true;
    }
}
