// === NetMaint — Traffic page (speed + connections): live bandwidth + TCP grid ===
using System.Diagnostics;
using System.Net.NetworkInformation;

namespace NetMaint.Pages;

public class TrafficPage : UserControl
{
    private readonly ComboBox _adapter;
    private readonly Button _startStop;
    private readonly Label _down;
    private readonly Label _up;
    private readonly Label _totals;
    private readonly GraphPanel _graph;
    private readonly DataGridView _grid;
    private readonly Button _refreshConn;
    private readonly System.Windows.Forms.Timer _timer;

    private NetworkInterface? _nic;
    private long _prevRx, _prevTx, _baseRx, _baseTx;
    private DateTime _started;
    private bool _running;

    public TrafficPage()
    {
        BackColor = Theme.Bg;

        var heading = Theme.Heading("Traffic");
        heading.Location = new Point(20, 16);

        var lbl = Theme.Label("Adapter:", dim: true);
        lbl.Location = new Point(20, 62);
        _adapter = new ComboBox
        {
            Location = new Point(85, 58),
            Width = 260,
            DropDownStyle = ComboBoxStyle.DropDownList,
            BackColor = Color.FromArgb(14, 14, 18),
            ForeColor = Theme.Text,
            FlatStyle = FlatStyle.Flat,
            Font = Theme.Base
        };

        _startStop = Theme.AccentButton("Start", 90);
        _startStop.Location = new Point(360, 55);
        _startStop.Click += (_, _) => Toggle();

        _down = new Label { Text = "↓ 0 Mbps", ForeColor = Theme.Accent, Font = Theme.Big, AutoSize = true, Location = new Point(20, 100) };
        _up   = new Label { Text = "↑ 0 Mbps", ForeColor = Theme.Good,  Font = Theme.Big, AutoSize = true, Location = new Point(320, 100) };
        _totals = Theme.Label("Session totals: —", dim: true);
        _totals.Location = new Point(22, 148);

        _graph = new GraphPanel
        {
            Location = new Point(20, 176),
            Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right,
            Height = 190,
            SeriesColors = new[] { Theme.Accent, Theme.Good },
            SeriesNames = new[] { "Down", "Up" },
            FormatValue = v => Theme.HumanBits(v)
        };

        var connLbl = Theme.SectionLabel("Active TCP connections by process");
        connLbl.Location = new Point(20, 382);

        _refreshConn = Theme.Button("Refresh", 90, 26);
        _refreshConn.Location = new Point(280, 377);
        _refreshConn.Click += (_, _) => RefreshConnections();

        _grid = Theme.Grid();
        _grid.Location = new Point(20, 410);
        _grid.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right | AnchorStyles.Bottom;
        _grid.Columns.Add("proc", "Process");
        _grid.Columns.Add("local", "Local");
        _grid.Columns.Add("remote", "Remote");
        _grid.Columns["proc"]!.FillWeight = 25;
        _grid.Columns["local"]!.FillWeight = 35;
        _grid.Columns["remote"]!.FillWeight = 40;

        _timer = new System.Windows.Forms.Timer { Interval = 1000 };
        _timer.Tick += (_, _) => Sample();

        Controls.Add(heading);
        Controls.Add(lbl);
        Controls.Add(_adapter);
        Controls.Add(_startStop);
        Controls.Add(_down);
        Controls.Add(_up);
        Controls.Add(_totals);
        Controls.Add(_graph);
        Controls.Add(connLbl);
        Controls.Add(_refreshConn);
        Controls.Add(_grid);

        Resize += (_, _) => LayoutSelf();
        LayoutSelf();

        Load += (_, _) =>
        {
            FillAdapters();
            RefreshConnections();
        };
    }

    private void LayoutSelf()
    {
        _graph.Width = Width - 40;
        _grid.Size = new Size(Width - 40, Math.Max(100, Height - 430));
    }

    private void FillAdapters()
    {
        _adapter.Items.Clear();
        foreach (var nic in NetHelpers.ActiveAdapters())
            _adapter.Items.Add(nic.Name);
        if (_adapter.Items.Count > 0) _adapter.SelectedIndex = 0;
    }

    private void Toggle()
    {
        if (_running)
        {
            _timer.Stop();
            _running = false;
            _startStop.Text = "Start";
            _adapter.Enabled = true;
            return;
        }

        string name = _adapter.SelectedItem?.ToString() ?? "";
        _nic = NetHelpers.ActiveAdapters().FirstOrDefault(n => n.Name == name);
        if (_nic == null) return;

        var stats = _nic.GetIPStatistics();
        _prevRx = _baseRx = stats.BytesReceived;
        _prevTx = _baseTx = stats.BytesSent;
        _started = DateTime.Now;
        _graph.Clear();

        _running = true;
        _startStop.Text = "Stop";
        _adapter.Enabled = false;
        _timer.Start();
    }

    private void Sample()
    {
        if (_nic == null) return;
        try
        {
            var stats = _nic.GetIPStatistics();
            long rx = stats.BytesReceived, tx = stats.BytesSent;
            double downBps = (rx - _prevRx) * 8.0;
            double upBps = (tx - _prevTx) * 8.0;
            _prevRx = rx;
            _prevTx = tx;

            _down.Text = "↓ " + Theme.HumanBits(downBps);
            _up.Text = "↑ " + Theme.HumanBits(upBps);
            var elapsed = DateTime.Now - _started;
            _totals.Text = $"Session totals:  down {Theme.HumanBytes(rx - _baseRx)}   up {Theme.HumanBytes(tx - _baseTx)}   ({elapsed:mm\\:ss})";
            _graph.AddSample(downBps, upBps);
        }
        catch
        {
            // Adapter disappeared (unplugged) — stop cleanly.
            Toggle();
        }
    }

    private void RefreshConnections()
    {
        _grid.Rows.Clear();
        var procs = Process.GetProcesses().ToDictionary(p => p.Id, p => p.ProcessName);

        // netstat -ano gives the PID mapping without needing admin
        var (_, output) = NetHelpers.RunHidden("netstat", "-ano");
        var rows = new List<(string Proc, string Local, string Remote)>();
        foreach (var line in output.Split('\n'))
        {
            var parts = line.Trim().Split(' ', StringSplitOptions.RemoveEmptyEntries);
            if (parts.Length == 5 && parts[0] == "TCP" && parts[3] == "ESTABLISHED"
                && int.TryParse(parts[4], out int pid))
            {
                rows.Add((procs.GetValueOrDefault(pid, $"pid {pid}"), parts[1], parts[2]));
            }
        }

        foreach (var r in rows.OrderBy(r => r.Proc).ThenBy(r => r.Remote))
        {
            int idx = _grid.Rows.Add(r.Proc, r.Local, r.Remote);
            _grid.Rows[idx].Cells[0].Style.ForeColor = Theme.WarnCol;
        }
    }
}
