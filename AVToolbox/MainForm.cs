// === AVToolbox — main window: sidebar of tools + coloured output pane ===
namespace AVToolbox;

public class MainForm : Form, IUiHost
{
    private static readonly Color BgDark    = Color.FromArgb(16, 16, 20);
    private static readonly Color BgPanel   = Color.FromArgb(30, 30, 36);
    private static readonly Color BgButton  = Color.FromArgb(42, 42, 50);
    private static readonly Color FgText    = Color.Gainsboro;
    private static readonly Color FgAccent  = Color.FromArgb(86, 214, 232);
    private static readonly Color FgSection = Color.FromArgb(229, 192, 80);

    private readonly RichTextBox _output;
    private readonly FlowLayoutPanel _sidebar;
    private readonly Label _status;

    public MainForm()
    {
        Text = "AV Toolbox";
        StartPosition = FormStartPosition.CenterScreen;
        Size = new Size(1100, 720);
        MinimumSize = new Size(860, 520);
        BackColor = BgPanel;
        Font = new Font("Segoe UI", 9.5f);

        _output = new RichTextBox
        {
            Dock = DockStyle.Fill,
            ReadOnly = true,
            BorderStyle = BorderStyle.None,
            BackColor = BgDark,
            ForeColor = FgText,
            Font = new Font("Consolas", 10f),
            DetectUrls = false,
            WordWrap = false,
            ScrollBars = RichTextBoxScrollBars.Both
        };

        var sidePanel = new Panel { Dock = DockStyle.Left, Width = 250, BackColor = BgPanel };
        _sidebar = new FlowLayoutPanel
        {
            Dock = DockStyle.Fill,
            FlowDirection = FlowDirection.TopDown,
            WrapContents = false,
            AutoScroll = true,
            Padding = new Padding(10, 6, 4, 10)
        };
        sidePanel.Controls.Add(_sidebar);

        var topBar = new Panel { Dock = DockStyle.Top, Height = 42, BackColor = BgPanel };
        var title = new Label
        {
            Text = "AV TOOLBOX",
            ForeColor = FgAccent,
            Font = new Font("Segoe UI Semibold", 12.5f),
            AutoSize = true,
            Location = new Point(14, 9)
        };
        _status = new Label
        {
            Text = "Ready",
            ForeColor = Color.FromArgb(130, 130, 140),
            AutoSize = true,
            Location = new Point(160, 13)
        };
        var clear = new Button
        {
            Text = "Clear",
            FlatStyle = FlatStyle.Flat,
            BackColor = BgButton,
            ForeColor = FgText,
            Size = new Size(70, 28),
            Anchor = AnchorStyles.Top | AnchorStyles.Right
        };
        clear.FlatAppearance.BorderColor = Color.FromArgb(70, 70, 80);
        clear.Location = new Point(topBar.Width - 84, 7);
        clear.Click += (_, _) => _output.Clear();
        topBar.Controls.Add(title);
        topBar.Controls.Add(_status);
        topBar.Controls.Add(clear);
        topBar.Resize += (_, _) => clear.Location = new Point(topBar.Width - 84, 7);

        BuildSidebar();

        Controls.Add(_output);
        Controls.Add(sidePanel);
        Controls.Add(topBar);

        Ui.Host = this;
        Ui.Hint("Select a tool on the left.");
    }

    private void BuildSidebar()
    {
        foreach (var section in ToolRegistry.Sections)
        {
            _sidebar.Controls.Add(new Label
            {
                Text = section.Title,
                ForeColor = FgSection,
                Font = new Font("Segoe UI Semibold", 9.5f),
                AutoSize = true,
                Margin = new Padding(2, 12, 0, 4)
            });

            foreach (var item in section.Items)
            {
                var btn = new Button
                {
                    Text = "  " + item.Name,
                    TextAlign = ContentAlignment.MiddleLeft,
                    FlatStyle = FlatStyle.Flat,
                    BackColor = BgButton,
                    ForeColor = FgText,
                    Size = new Size(215, 30),
                    Margin = new Padding(2, 2, 2, 2),
                    Tag = item
                };
                btn.FlatAppearance.BorderColor = Color.FromArgb(58, 58, 68);
                btn.FlatAppearance.MouseOverBackColor = Color.FromArgb(56, 56, 66);
                new ToolTip().SetToolTip(btn, item.Desc);
                btn.Click += (_, _) => RunTool(item);
                _sidebar.Controls.Add(btn);
            }
        }
    }

    private void RunTool(MenuItem item)
    {
        _sidebar.Enabled = false;
        _status.Text = $"Running {item.Name}...";
        _status.ForeColor = FgAccent;

        Task.Run(() =>
        {
            try { item.Action(); }
            catch (Exception ex) { Ui.Err($"Error running '{item.Name}': {ex.Message}"); }
            finally
            {
                BeginInvoke(() =>
                {
                    _sidebar.Enabled = true;
                    _status.Text = "Ready";
                    _status.ForeColor = Color.FromArgb(130, 130, 140);
                });
            }
        });
    }

    // ---- IUiHost (tools run on a worker thread; marshal to UI) ----

    public void Append(string text, ConsoleColor color, bool newline)
    {
        if (InvokeRequired) { BeginInvoke(() => Append(text, color, newline)); return; }
        _output.SelectionStart = _output.TextLength;
        _output.SelectionLength = 0;
        _output.SelectionColor = Map(color);
        _output.AppendText(newline ? text + Environment.NewLine : text);
        _output.SelectionStart = _output.TextLength;
        _output.ScrollToCaret();
    }

    public string? Prompt(string label)
    {
        if (InvokeRequired) return (string?)Invoke(() => Prompt(label));
        return InputDialog.Show(this, label);
    }

    public string? PickFile(string title)
    {
        if (InvokeRequired) return (string?)Invoke(() => PickFile(title));
        using var dlg = new OpenFileDialog { Title = title, CheckFileExists = true };
        return dlg.ShowDialog(this) == DialogResult.OK ? dlg.FileName : null;
    }

    private static Color Map(ConsoleColor c) => c switch
    {
        ConsoleColor.Cyan     => FgAccent,
        ConsoleColor.DarkCyan => Color.FromArgb(0, 160, 180),
        ConsoleColor.Yellow   => FgSection,
        ConsoleColor.Green    => Color.FromArgb(120, 210, 115),
        ConsoleColor.Red      => Color.FromArgb(240, 95, 95),
        ConsoleColor.DarkGray => Color.FromArgb(130, 130, 140),
        _                     => FgText
    };
}
