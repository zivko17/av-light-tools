// === AVToolbox — external process helpers ===
using System.Diagnostics;

namespace AVToolbox;

public static class Exec
{
    /// <summary>Find an executable on PATH (like Get-Command). Returns full path or null.</summary>
    public static string? FindExe(string name)
    {
        var exe = name.EndsWith(".exe", StringComparison.OrdinalIgnoreCase) ? name : name + ".exe";
        foreach (var dir in (Environment.GetEnvironmentVariable("PATH") ?? "").Split(';'))
        {
            if (string.IsNullOrWhiteSpace(dir)) continue;
            try
            {
                var full = Path.Combine(dir.Trim(), exe);
                if (File.Exists(full)) return full;
            }
            catch { /* malformed PATH entry */ }
        }
        return null;
    }

    /// <summary>Require an exe; if missing, print a winget install hint and return null.</summary>
    public static string? Require(string name, string wingetId)
    {
        var path = FindExe(name);
        if (path == null)
        {
            Ui.Err($"'{name}' not found on PATH.");
            Ui.Hint($"Install with:  winget install {wingetId}");
        }
        return path;
    }

    /// <summary>
    /// Run a process with ArgumentList (no quoting issues), no window;
    /// stdout/stderr are streamed into the UI output. Returns the exit code.
    /// </summary>
    public static int Run(string exe, params string[] args)
    {
        var psi = new ProcessStartInfo(exe)
        {
            UseShellExecute = false,
            CreateNoWindow = true,
            RedirectStandardOutput = true,
            RedirectStandardError = true
        };
        foreach (var a in args) psi.ArgumentList.Add(a);
        using var p = new Process { StartInfo = psi };
        p.OutputDataReceived += (_, e) => { if (e.Data != null) Ui.Hint(e.Data); };
        p.ErrorDataReceived += (_, e) => { if (e.Data != null) Ui.Hint(e.Data); };
        p.Start();
        p.BeginOutputReadLine();
        p.BeginErrorReadLine();
        p.WaitForExit();
        return p.ExitCode;
    }

    /// <summary>Run a process and capture stdout (stderr discarded). Returns null on failure to start.</summary>
    public static string? Capture(string exe, params string[] args)
    {
        try
        {
            var psi = new ProcessStartInfo(exe)
            {
                UseShellExecute = false,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                StandardOutputEncoding = System.Text.Encoding.UTF8
            };
            foreach (var a in args) psi.ArgumentList.Add(a);
            using var p = Process.Start(psi)!;
            string output = p.StandardOutput.ReadToEnd();
            p.StandardError.ReadToEnd();
            p.WaitForExit();
            return output;
        }
        catch { return null; }
    }
}
