// === NetMaint — Ping Monitor page (pingmulti + latency): live grid + latency graph ===
using System.Net.NetworkInformation;

namespace NetMaint.Pages;

public class PingPage : UserControl
{
    private readonly TextBox _ips;
    private readonly Button _startStop;
    private readonly DataGridView _grid;
    private readonly GraphPanel _graph;
    private readonly Label _graphLabel;
    private readonly System.Windows.Forms.Timer _timer;
    private readonly Dictionary<string, int> _rowByIp = new();
    private string? _graphIp;
    private bool _running;
    private bool _busy;

    public PingPage()
    {
        BackColor = Theme.Bg;

        var heading = Theme.Heading("Ping Monitor");
        heading.Location = new Point(20, 16);

        var lbl = Theme.Label("IPs (comma-separated):", dim: true);
        lbl.Location = new Point(20, 62);
        _ips = Theme.TextBox(430);
        _ips.Location = new Point(170, 58);
        _ips.PlaceholderText = "192.168.1.1, 192.168.1.10, 8.8.8.8";

        _startStop = Theme.AccentButton("Start", 90);
        _startStop.Location = new Point(615, 55);
        _startStop.Click += (_, _) => Toggle();

        _grid = Theme.Grid();
        _grid.Location = new Point(20, 100);
        _grid.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
        _grid.Height = 220;
        _grid.Columns.Add("ip", "IP");
        _grid.Columns.Add("status", "Status");
        _grid.Columns.Add("latency", "Latency");
        _grid.Columns.Add("loss", "Failures");
        _grid.SelectionChanged += (_, _) => SelectGraphTarget();

        _graphLabel = Theme.SectionLabel("Latency graph — select a row");
        _graphLabel.Location = new Point(20, 336);

        _graph = new GraphPanel
        {
            Location = new Point(20, 362),
            Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right | AnchorStyles.Bottom,
            SeriesColors = new[] { Theme.Accent },
            SeriesNames = new[] { "ms" },
            FormatValue = v => $"{v:0} ms"
        };

        _timer = new System.Windows.Forms.Timer { Interval = 2000 };
        _timer.Tick += async (_, _) => await TickAsync();

        Controls.Add(heading);
        Controls.Add(lbl);
        Controls.Add(_ips);
        Controls.Add(_startStop);
        Controls.Add(_grid);
        Controls.Add(_graphLabel);
        Controls.Add(_graph);

        Resize += (_, _) => LayoutSelf();
        LayoutSelf();
    }

    private void LayoutSelf()
    {
        _grid.Width = Width - 40;
        _graph.Size = new Size(Width - 40, Math.Max(120, Height - 382));
    }

    private void Toggle()
    {
        if (_running)
        {
            _timer.Stop();
            _running = false;
            _startStop.Text = "Start";
            return;
        }

        var ips = _ips.Text.Split(',')
            .Select(s => s.Trim())
            .Where(s => s.Length > 0)
            .Distinct()
            .ToList();
        if (ips.Count == 0) return;

        _grid.Rows.Clear();
        _rowByIp.Clear();
        _graph.Clear();
        foreach (var ip in ips)
            _rowByIp[ip] = _grid.Rows.Add(ip, "...", "-", "0");

        _graphIp = ips[0];
        _graphLabel.Text = $"Latency graph — {_graphIp}";
        _running = true;
        _startStop.Text = "Stop";
        _timer.Start();
        _ = TickAsync();
    }

    private void SelectGraphTarget()
    {
        if (_grid.SelectedRows.Count == 0) return;
        string ip = _grid.SelectedRows[0].Cells[0].Value?.ToString() ?? "";
        if (ip.Length > 0 && ip != _graphIp)
        {
            _graphIp = ip;
            _graph.Clear();
            _graphLabel.Text = $"Latency graph — {ip}";
        }
    }

    private async Task TickAsync()
    {
        if (_busy || !_running) return;
        _busy = true;
        try
        {
            var tasks = _rowByIp.Keys.Select(async ip =>
            {
                try
                {
                    using var ping = new Ping();
                    var reply = await ping.SendPingAsync(ip, 1500);
                    return (Ip: ip, Ok: reply.Status == IPStatus.Success, Ms: reply.RoundtripTime);
                }
                catch { return (Ip: ip, Ok: false, Ms: 0L); }
            }).ToList();
            var results = await Task.WhenAll(tasks);

            foreach (var r in results)
            {
                if (!_rowByIp.TryGetValue(r.Ip, out int idx) || idx >= _grid.Rows.Count) continue;
                var row = _grid.Rows[idx];
                if (r.Ok)
                {
                    row.Cells[1].Value = "ONLINE";
                    row.Cells[1].Style.ForeColor = Theme.Good;
                    row.Cells[2].Value = $"{r.Ms} ms";
                    row.Cells[2].Style.ForeColor =
                        r.Ms < 10 ? Theme.Good : r.Ms < 30 ? Theme.WarnCol : Theme.Bad;
                }
                else
                {
                    row.Cells[1].Value = "OFFLINE";
                    row.Cells[1].Style.ForeColor = Theme.Bad;
                    row.Cells[2].Value = "-";
                    int fails = int.Parse(row.Cells[3].Value?.ToString() ?? "0") + 1;
                    row.Cells[3].Value = fails.ToString();
                    row.Cells[3].Style.ForeColor = Theme.WarnCol;
                }

                if (r.Ip == _graphIp)
                    _graph.AddSample(r.Ok ? r.Ms : 0);
            }
        }
        finally { _busy = false; }
    }
}
