// === NetMaint — dark theme + control factory helpers ===
namespace NetMaint;

public static class Theme
{
    public static readonly Color Bg        = Color.FromArgb(18, 18, 23);
    public static readonly Color Panel     = Color.FromArgb(28, 28, 35);
    public static readonly Color Card      = Color.FromArgb(36, 36, 45);
    public static readonly Color ButtonBg  = Color.FromArgb(46, 46, 57);
    public static readonly Color Border    = Color.FromArgb(62, 62, 75);
    public static readonly Color Text      = Color.Gainsboro;
    public static readonly Color TextDim   = Color.FromArgb(140, 140, 152);
    public static readonly Color Accent    = Color.FromArgb(86, 214, 232);
    public static readonly Color AccentDim = Color.FromArgb(0, 150, 170);
    public static readonly Color Good      = Color.FromArgb(120, 210, 115);
    public static readonly Color WarnCol   = Color.FromArgb(229, 192, 80);
    public static readonly Color Bad       = Color.FromArgb(240, 95, 95);

    public static Font Base      => new("Segoe UI", 9.5f);
    public static Font Semibold  => new("Segoe UI Semibold", 9.5f);
    public static Font Title     => new("Segoe UI Semibold", 13f);
    public static Font Big       => new("Segoe UI Semibold", 22f);
    public static Font Mono      => new("Consolas", 10f);

    public static Button Button(string text, int width = 130, int height = 32)
    {
        var b = new Button
        {
            Text = text,
            Size = new Size(width, height),
            FlatStyle = FlatStyle.Flat,
            BackColor = ButtonBg,
            ForeColor = Text,
            Font = Base,
            Cursor = Cursors.Hand
        };
        b.FlatAppearance.BorderColor = Border;
        b.FlatAppearance.MouseOverBackColor = Color.FromArgb(58, 58, 70);
        return b;
    }

    public static Button AccentButton(string text, int width = 130, int height = 32)
    {
        var b = Button(text, width, height);
        b.BackColor = Color.FromArgb(20, 90, 100);
        b.ForeColor = Color.White;
        b.FlatAppearance.BorderColor = AccentDim;
        b.FlatAppearance.MouseOverBackColor = Color.FromArgb(0, 115, 130);
        return b;
    }

    public static Label Label(string text, bool dim = false)
        => new() { Text = text, AutoSize = true, ForeColor = dim ? TextDim : Text, Font = Base, BackColor = Color.Transparent };

    public static Label Heading(string text)
        => new() { Text = text, AutoSize = true, ForeColor = Accent, Font = Title, BackColor = Color.Transparent };

    public static Label SectionLabel(string text)
        => new() { Text = text, AutoSize = true, ForeColor = WarnCol, Font = Semibold, BackColor = Color.Transparent };

    public static TextBox TextBox(int width = 180)
        => new()
        {
            Width = width,
            BackColor = Color.FromArgb(14, 14, 18),
            ForeColor = Text,
            BorderStyle = BorderStyle.FixedSingle,
            Font = Base
        };

    public static DataGridView Grid()
    {
        var g = new DataGridView
        {
            BackgroundColor = Panel,
            BorderStyle = BorderStyle.None,
            EnableHeadersVisualStyles = false,
            ColumnHeadersHeightSizeMode = DataGridViewColumnHeadersHeightSizeMode.AutoSize,
            RowHeadersVisible = false,
            AllowUserToAddRows = false,
            AllowUserToDeleteRows = false,
            AllowUserToResizeRows = false,
            ReadOnly = true,
            SelectionMode = DataGridViewSelectionMode.FullRowSelect,
            MultiSelect = false,
            AutoSizeColumnsMode = DataGridViewAutoSizeColumnsMode.Fill,
            GridColor = Color.FromArgb(45, 45, 56),
            Font = Base
        };
        g.ColumnHeadersDefaultCellStyle.BackColor = Card;
        g.ColumnHeadersDefaultCellStyle.ForeColor = TextDim;
        g.ColumnHeadersDefaultCellStyle.SelectionBackColor = Card;
        g.ColumnHeadersDefaultCellStyle.Font = Semibold;
        g.DefaultCellStyle.BackColor = Panel;
        g.DefaultCellStyle.ForeColor = Text;
        g.DefaultCellStyle.SelectionBackColor = Color.FromArgb(30, 75, 85);
        g.DefaultCellStyle.SelectionForeColor = Color.White;
        g.DefaultCellStyle.Padding = new Padding(4, 2, 4, 2);
        return g;
    }

    public static Panel CardPanel()
        => new() { BackColor = Card, Padding = new Padding(14) };

    public static string HumanBytes(double bytes)
    {
        string[] units = { "B", "KB", "MB", "GB", "TB" };
        int u = 0;
        while (bytes >= 1024 && u < units.Length - 1) { bytes /= 1024; u++; }
        return $"{bytes:0.#} {units[u]}";
    }

    public static string HumanBits(double bitsPerSec)
    {
        if (bitsPerSec >= 1e9) return $"{bitsPerSec / 1e9:0.00} Gbps";
        if (bitsPerSec >= 1e6) return $"{bitsPerSec / 1e6:0.00} Mbps";
        if (bitsPerSec >= 1e3) return $"{bitsPerSec / 1e3:0.0} Kbps";
        return $"{bitsPerSec:0} bps";
    }
}
