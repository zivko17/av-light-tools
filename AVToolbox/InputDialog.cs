// === AVToolbox — small dark input dialog used by Ui.Ask ===
namespace AVToolbox;

public static class InputDialog
{
    /// <summary>Returns the entered text, or null if cancelled.</summary>
    public static string? Show(IWin32Window owner, string label)
    {
        using var form = new Form
        {
            Text = "AV Toolbox",
            FormBorderStyle = FormBorderStyle.FixedDialog,
            StartPosition = FormStartPosition.CenterParent,
            ClientSize = new Size(420, 120),
            MaximizeBox = false,
            MinimizeBox = false,
            ShowInTaskbar = false,
            BackColor = Color.FromArgb(30, 30, 36),
            Font = new Font("Segoe UI", 9.5f)
        };

        var lbl = new Label
        {
            Text = label,
            AutoSize = true,
            Location = new Point(14, 12),
            ForeColor = Color.Gainsboro
        };
        var txt = new TextBox
        {
            Location = new Point(14, 40),
            Width = 392,
            BackColor = Color.FromArgb(16, 16, 20),
            ForeColor = Color.Gainsboro,
            BorderStyle = BorderStyle.FixedSingle
        };

        var ok = MakeButton("OK", DialogResult.OK, new Point(246, 78));
        var cancel = MakeButton("Cancel", DialogResult.Cancel, new Point(330, 78));

        form.Controls.AddRange(new Control[] { lbl, txt, ok, cancel });
        form.AcceptButton = ok;
        form.CancelButton = cancel;

        return form.ShowDialog(owner) == DialogResult.OK ? txt.Text : null;
    }

    private static Button MakeButton(string text, DialogResult result, Point location)
    {
        var b = new Button
        {
            Text = text,
            DialogResult = result,
            Location = location,
            Size = new Size(76, 28),
            FlatStyle = FlatStyle.Flat,
            BackColor = Color.FromArgb(42, 42, 50),
            ForeColor = Color.Gainsboro
        };
        b.FlatAppearance.BorderColor = Color.FromArgb(70, 70, 80);
        return b;
    }
}
