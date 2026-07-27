// === AVToolbox — output/prompt abstraction (colour scheme matches C:\Scripts) ===
namespace AVToolbox;

/// <summary>The GUI implements this; tools only talk to Ui.</summary>
public interface IUiHost
{
    void Append(string text, ConsoleColor color, bool newline);
    string? Prompt(string label);
    string? PickFile(string title);
}

public static class Ui
{
    // Cyan = headers, Yellow = labels/warnings, Green = good, Red = bad,
    // DarkGray = hints/secondary, White = primary values.

    public static IUiHost? Host;

    public static void Write(string text, ConsoleColor color, bool newline = true)
        => Host?.Append(text, color, newline);

    public static void Header(string text)
    {
        Blank();
        Write($"  === {text} ===", ConsoleColor.Cyan);
        Blank();
    }

    public static void Label(string label, string value, ConsoleColor valueColor = ConsoleColor.White)
    {
        Write($"  {label,-18}", ConsoleColor.Yellow, newline: false);
        Write(value, valueColor);
    }

    public static void Ok(string text)   => Write("  " + text, ConsoleColor.Green);
    public static void Err(string text)  => Write("  " + text, ConsoleColor.Red);
    public static void Warn(string text) => Write("  " + text, ConsoleColor.Yellow);
    public static void Hint(string text) => Write("  " + text, ConsoleColor.DarkGray);
    public static void Line(string text) => Write("  " + text, ConsoleColor.White);

    /// <summary>Continue the current line (no indent) and end it.</summary>
    public static void Plain(string text) => Write(text, ConsoleColor.White);

    public static void Blank() => Write("", ConsoleColor.White);

    /// <summary>Prompt for input via dialog; strips surrounding quotes and trims. Empty = cancelled.</summary>
    public static string Ask(string prompt)
    {
        var s = Host?.Prompt(prompt) ?? "";
        s = s.Trim();
        if (s.Length >= 2 && s.StartsWith('"') && s.EndsWith('"')) s = s[1..^1];
        return s.Trim();
    }

    /// <summary>Ask for an existing file via file picker; returns null if cancelled.</summary>
    public static string? AskFile(string prompt)
    {
        var p = Host?.PickFile(prompt);
        if (string.IsNullOrWhiteSpace(p)) return null;
        if (!File.Exists(p)) { Err($"File not found: {p}"); return null; }
        return p;
    }

    /// <summary>Simple usage bar, e.g. [#####-----] 50%</summary>
    public static string Bar(double fraction, int width = 20)
    {
        if (double.IsNaN(fraction)) fraction = 0;
        fraction = Math.Clamp(fraction, 0, 1);
        int filled = (int)Math.Round(fraction * width);
        return "[" + new string('#', filled) + new string('-', width - filled) + "]";
    }

    public static string HumanSize(double bytes)
    {
        string[] units = { "B", "KB", "MB", "GB", "TB" };
        int u = 0;
        while (bytes >= 1024 && u < units.Length - 1) { bytes /= 1024; u++; }
        return $"{bytes:0.#} {units[u]}";
    }
}
