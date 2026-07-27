// === NetMaint — lightweight live line-graph control (1-2 series) ===
namespace NetMaint;

public class GraphPanel : Panel
{
    private readonly List<double[]> _samples = new();

    public int MaxSamples { get; set; } = 120;
    public Color[] SeriesColors { get; set; } = { Theme.Accent, Theme.Good };
    public string[] SeriesNames { get; set; } = Array.Empty<string>();
    public Func<double, string> FormatValue { get; set; } = v => v.ToString("0");

    public GraphPanel()
    {
        DoubleBuffered = true;
        BackColor = Color.FromArgb(14, 14, 18);
        ResizeRedraw = true;
    }

    public void AddSample(params double[] values)
    {
        _samples.Add(values);
        while (_samples.Count > MaxSamples) _samples.RemoveAt(0);
        Invalidate();
    }

    public void Clear()
    {
        _samples.Clear();
        Invalidate();
    }

    protected override void OnPaint(PaintEventArgs e)
    {
        base.OnPaint(e);
        var g = e.Graphics;
        g.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.AntiAlias;

        var area = new Rectangle(6, 6, Width - 12, Height - 12);
        if (area.Width <= 0 || area.Height <= 0) return;

        double max = 1;
        foreach (var s in _samples)
            foreach (var v in s)
                if (v > max) max = v;
        max *= 1.15; // headroom

        // Grid lines: 25/50/75/100 %
        using var gridPen = new Pen(Color.FromArgb(38, 38, 48));
        using var dimBrush = new SolidBrush(Theme.TextDim);
        using var smallFont = new Font("Segoe UI", 7.5f);
        for (int i = 1; i <= 3; i++)
        {
            int y = area.Bottom - area.Height * i / 4;
            g.DrawLine(gridPen, area.Left, y, area.Right, y);
            g.DrawString(FormatValue(max * i / 4), smallFont, dimBrush, area.Left + 2, y - 13);
        }

        if (_samples.Count >= 2)
        {
            int series = _samples[0].Length;
            float dx = (float)area.Width / (MaxSamples - 1);
            for (int s = 0; s < series; s++)
            {
                var pts = new List<PointF>();
                for (int i = 0; i < _samples.Count; i++)
                {
                    float x = area.Right - (_samples.Count - 1 - i) * dx;
                    float y = area.Bottom - (float)(Math.Min(_samples[i][s], max) / max * area.Height);
                    pts.Add(new PointF(x, y));
                }
                using var pen = new Pen(SeriesColors[s % SeriesColors.Length], 1.8f);
                if (pts.Count >= 2) g.DrawLines(pen, pts.ToArray());
            }
        }

        // Legend
        for (int s = 0; s < SeriesNames.Length; s++)
        {
            using var b = new SolidBrush(SeriesColors[s % SeriesColors.Length]);
            int lx = area.Right - 90, ly = area.Top + 4 + s * 16;
            g.FillRectangle(b, lx, ly + 4, 10, 3);
            g.DrawString(SeriesNames[s], smallFont, b, lx + 14, ly - 2);
        }
    }
}
