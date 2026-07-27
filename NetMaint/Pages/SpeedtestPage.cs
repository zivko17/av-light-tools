// === NetMaint — Speedtest page (Ookla CLI, run hidden, results as big numbers) ===
using System.Text.Json;

namespace NetMaint.Pages;

public class SpeedtestPage : UserControl
{
    private readonly Button _run;
    private readonly Label _status;
    private readonly ProgressBar _spinner;
    private readonly Label _ping;
    private readonly Label _download;
    private readonly Label _upload;
    private readonly Label _details;

    public SpeedtestPage()
    {
        BackColor = Theme.Bg;

        var heading = Theme.Heading("Internet Speedtest");
        heading.Location = new Point(20, 16);

        _run = Theme.AccentButton("Run speedtest", 140);
        _run.Location = new Point(20, 56);
        _run.Click += async (_, _) => await RunAsync();

        _spinner = new ProgressBar
        {
            Location = new Point(175, 60),
            Size = new Size(200, 22),
            Style = ProgressBarStyle.Marquee,
            MarqueeAnimationSpeed = 30,
            Visible = false
        };

        _status = Theme.Label("", dim: true);
        _status.Location = new Point(20, 96);

        Label BigStat(string text, Color color, int x) => new()
        {
            Text = text,
            ForeColor = color,
            Font = new Font("Segoe UI Semibold", 26f),
            AutoSize = true,
            Location = new Point(x, 150)
        };

        var pingCap = Theme.Label("PING", dim: true);       pingCap.Location = new Point(24, 130);
        var downCap = Theme.Label("DOWNLOAD", dim: true);   downCap.Location = new Point(264, 130);
        var upCap = Theme.Label("UPLOAD", dim: true);       upCap.Location = new Point(564, 130);

        _ping = BigStat("—", Theme.WarnCol, 20);
        _download = BigStat("—", Theme.Accent, 260);
        _upload = BigStat("—", Theme.Good, 560);

        _details = Theme.Label("", dim: true);
        _details.Location = new Point(22, 220);

        Controls.Add(heading);
        Controls.Add(_run);
        Controls.Add(_spinner);
        Controls.Add(_status);
        Controls.Add(pingCap);
        Controls.Add(downCap);
        Controls.Add(upCap);
        Controls.Add(_ping);
        Controls.Add(_download);
        Controls.Add(_upload);
        Controls.Add(_details);
    }

    private async Task RunAsync()
    {
        var cli = NetHelpers.FindExe("speedtest");
        if (cli == null)
        {
            _status.Text = "Ookla Speedtest CLI not installed — run:  winget install Ookla.Speedtest.CLI";
            _status.ForeColor = Theme.WarnCol;
            return;
        }

        _run.Enabled = false;
        _spinner.Visible = true;
        _status.Text = "Running speed test... approx 30 seconds";
        _status.ForeColor = Theme.TextDim;

        var (code, output) = await Task.Run(() =>
            NetHelpers.RunHidden(cli, "--accept-license", "--accept-gdpr", "--format=json"));

        _spinner.Visible = false;
        _run.Enabled = true;

        if (code != 0)
        {
            _status.Text = "Speedtest failed: " + output.Split('\n').FirstOrDefault(l => l.Trim().Length > 0)?.Trim();
            _status.ForeColor = Theme.Bad;
            return;
        }

        try
        {
            // The JSON result is the last non-empty line
            var jsonLine = output.Split('\n').Last(l => l.TrimStart().StartsWith('{'));
            using var doc = JsonDocument.Parse(jsonLine);
            var root = doc.RootElement;

            double ping = root.GetProperty("ping").GetProperty("latency").GetDouble();
            double down = root.GetProperty("download").GetProperty("bandwidth").GetDouble() * 8 / 1e6;
            double up = root.GetProperty("upload").GetProperty("bandwidth").GetDouble() * 8 / 1e6;

            _ping.Text = $"{ping:0.0} ms";
            _download.Text = $"{down:0.0} Mbps";
            _upload.Text = $"{up:0.0} Mbps";

            string isp = root.TryGetProperty("isp", out var i) ? i.GetString() ?? "" : "";
            string server = "";
            if (root.TryGetProperty("server", out var s))
                server = $"{s.GetProperty("name").GetString()} ({s.GetProperty("location").GetString()})";
            _details.Text = $"ISP: {isp}    Server: {server}";
            _status.Text = "Done.";
            _status.ForeColor = Theme.Good;
        }
        catch (Exception)
        {
            _status.Text = "Could not parse speedtest output.";
            _status.ForeColor = Theme.Bad;
        }
    }
}
