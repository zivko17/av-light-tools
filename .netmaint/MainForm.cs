// === NetMaint — main window: left navigation + page host ===
using System.Diagnostics;

namespace NetMaint;

public class MainForm : Form
{
    private readonly Panel _content;
    private readonly Dictionary<string, UserControl> _pages = new();
    private readonly List<Button> _navButtons = new();

    private static readonly (string Title, Func<UserControl> Factory)[] Nav =
    {
        ("Network Info",  () => new Pages.NetworkInfoPage()),
        ("Subnet Scan",   () => new Pages.ScanPage()),
        ("Ports",         () => new Pages.PortsPage()),
        ("Ping Monitor",  () => new Pages.PingPage()),
        ("Traffic",       () => new Pages.TrafficPage()),
        ("Speedtest",     () => new Pages.SpeedtestPage()),
        ("IP Config",     () => new Pages.IpConfigPage()),
        ("Maintenance",   () => new Pages.MaintenancePage()),
    };

    public MainForm()
    {
        Text = "NetMaint — Network & Maintenance";
        StartPosition = FormStartPosition.CenterScreen;
        Size = new Size(1150, 760);
        MinimumSize = new Size(980, 620);
        BackColor = Theme.Bg;
        Font = Theme.Base;

        _content = new Panel { Dock = DockStyle.Fill, BackColor = Theme.Bg };

        var sidebar = new Panel { Dock = DockStyle.Left, Width = 190, BackColor = Theme.Panel };
        var title = new Label
        {
            Text = "NETMAINT",
            ForeColor = Theme.Accent,
            Font = new Font("Segoe UI Semibold", 14f),
            AutoSize = false,
            Height = 56,
            Dock = DockStyle.Top,
            TextAlign = ContentAlignment.MiddleCenter
        };
        sidebar.Controls.Add(title);

        var navFlow = new FlowLayoutPanel
        {
            Dock = DockStyle.Fill,
            FlowDirection = FlowDirection.TopDown,
            WrapContents = false,
            Padding = new Padding(10, 8, 10, 8)
        };

        foreach (var (navTitle, _) in Nav)
        {
            var btn = new Button
            {
                Text = "  " + navTitle,
                TextAlign = ContentAlignment.MiddleLeft,
                Size = new Size(166, 38),
                FlatStyle = FlatStyle.Flat,
                BackColor = Theme.Panel,
                ForeColor = Theme.Text,
                Font = Theme.Base,
                Margin = new Padding(0, 2, 0, 2),
                Cursor = Cursors.Hand,
                Tag = navTitle
            };
            btn.FlatAppearance.BorderSize = 0;
            btn.FlatAppearance.MouseOverBackColor = Theme.Card;
            btn.Click += (_, _) => ShowPage(navTitle);
            _navButtons.Add(btn);
            navFlow.Controls.Add(btn);
        }
        sidebar.Controls.Add(navFlow);
        navFlow.BringToFront();

        // Bottom of sidebar: admin state
        var adminPanel = new Panel { Dock = DockStyle.Bottom, Height = 64, BackColor = Theme.Panel };
        if (NetHelpers.IsAdmin())
        {
            var lbl = new Label
            {
                Text = "Administrator",
                ForeColor = Theme.Good,
                Dock = DockStyle.Fill,
                TextAlign = ContentAlignment.MiddleCenter,
                Font = Theme.Semibold
            };
            adminPanel.Controls.Add(lbl);
        }
        else
        {
            var elevate = Theme.Button("Run as admin", 150, 30);
            elevate.Location = new Point(20, 8);
            elevate.Click += (_, _) => RestartElevated();
            var note = new Label
            {
                Text = "needed for IP config",
                ForeColor = Theme.TextDim,
                AutoSize = true,
                Location = new Point(38, 42),
                Font = new Font("Segoe UI", 8f)
            };
            adminPanel.Controls.Add(elevate);
            adminPanel.Controls.Add(note);
        }
        sidebar.Controls.Add(adminPanel);

        Controls.Add(_content);
        Controls.Add(sidebar);

        ShowPage(Nav[0].Title);
    }

    private void ShowPage(string title)
    {
        if (!_pages.TryGetValue(title, out var page))
        {
            page = Nav.First(n => n.Title == title).Factory();
            page.Dock = DockStyle.Fill;
            _pages[title] = page;
            _content.Controls.Add(page);
        }
        page.BringToFront();
        foreach (var b in _navButtons)
        {
            bool active = (string)b.Tag! == title;
            b.BackColor = active ? Theme.Card : Theme.Panel;
            b.ForeColor = active ? Theme.Accent : Theme.Text;
        }
    }

    private static void RestartElevated()
    {
        try
        {
            Process.Start(new ProcessStartInfo(Application.ExecutablePath)
            {
                UseShellExecute = true,
                Verb = "runas"
            });
            Application.Exit();
        }
        catch { /* UAC cancelled */ }
    }
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
