// === AVToolbox — FFmpeg audio tools (outputs to the user's Music folder) ===
using System.Text.Json;

namespace AVToolbox;

public static class AudioTools
{
    // format -> (extension, codec args)
    private static readonly Dictionary<string, (string Ext, string[] Args)> Codecs = new()
    {
        ["mp3"]  = (".mp3",  new[] { "-c:a", "libmp3lame", "-b:a", "320k" }),
        ["aac"]  = (".m4a",  new[] { "-c:a", "aac", "-b:a", "256k" }),
        ["opus"] = (".opus", new[] { "-c:a", "libopus", "-b:a", "160k" }),
        ["flac"] = (".flac", new[] { "-c:a", "flac" }),
        ["wav"]  = (".wav",  new[] { "-c:a", "pcm_s16le" }),
    };

    private static string MusicFolder => Environment.GetFolderPath(Environment.SpecialFolder.MyMusic);

    private static string OutPath(string input, string suffix, string ext)
    {
        var name = Path.GetFileNameWithoutExtension(input) + suffix + ext;
        return Path.Combine(MusicFolder, name);
    }

    private static (string Ext, string[] Args)? AskFormat()
    {
        Ui.Hint($"Formats: {string.Join(" / ", Codecs.Keys)}");
        string fmt = Ui.Ask("Output format").ToLowerInvariant();
        if (!Codecs.TryGetValue(fmt, out var codec)) { Ui.Err("Unknown format."); return null; }
        return codec;
    }

    private static void RunFFmpeg(string ffmpeg, string input, string output, params string[] middleArgs)
    {
        var args = new List<string> { "-hide_banner", "-y", "-i", input };
        args.AddRange(middleArgs);
        args.Add(output);
        Ui.Blank();
        int code = Exec.Run(ffmpeg, args.ToArray());
        Ui.Blank();
        if (code == 0) Ui.Ok($"Done -> {output}");
        else Ui.Err($"FFmpeg exited with code {code}.");
    }

    public static void Convert()
    {
        var ffmpeg = Exec.Require("ffmpeg", "Gyan.FFmpeg");
        if (ffmpeg == null) return;
        var input = Ui.AskFile("Input file");
        if (input == null) return;
        var codec = AskFormat();
        if (codec == null) return;
        RunFFmpeg(ffmpeg, input, OutPath(input, "", codec.Value.Ext), codec.Value.Args);
    }

    public static void ExtractAudio()
    {
        var ffmpeg = Exec.Require("ffmpeg", "Gyan.FFmpeg");
        if (ffmpeg == null) return;
        var input = Ui.AskFile("Video file");
        if (input == null) return;
        var codec = AskFormat();
        if (codec == null) return;
        var args = new List<string> { "-vn" };
        args.AddRange(codec.Value.Args);
        RunFFmpeg(ffmpeg, input, OutPath(input, "", codec.Value.Ext), args.ToArray());
    }

    public static void Normalize()
    {
        var ffmpeg = Exec.Require("ffmpeg", "Gyan.FFmpeg");
        if (ffmpeg == null) return;
        var input = Ui.AskFile("Input file");
        if (input == null) return;

        // Keep the source format when we know it; otherwise fall back to WAV.
        string ext = Path.GetExtension(input).ToLowerInvariant();
        var codec = Codecs.Values.FirstOrDefault(c => c.Ext == ext);
        if (codec.Ext == null) codec = Codecs["wav"];

        Ui.Hint("Normalizing to EBU R128 (-23 LUFS)...");
        var args = new List<string> { "-af", "loudnorm=I=-23:LRA=7:TP=-2" };
        args.AddRange(codec.Args);
        RunFFmpeg(ffmpeg, input, OutPath(input, "_norm", codec.Ext), args.ToArray());
    }

    public static void Trim()
    {
        var ffmpeg = Exec.Require("ffmpeg", "Gyan.FFmpeg");
        if (ffmpeg == null) return;
        var input = Ui.AskFile("Input file");
        if (input == null) return;
        string start = Ui.Ask("Start (hh:mm:ss or seconds)");
        string end = Ui.Ask("End (hh:mm:ss or seconds)");
        if (start.Length == 0 || end.Length == 0) { Ui.Err("Start and end required."); return; }

        string ext = Path.GetExtension(input);
        RunFFmpeg(ffmpeg, input, OutPath(input, "_trim", ext), "-ss", start, "-to", end, "-c", "copy");
    }

    public static void Probe()
    {
        var ffprobe = Exec.Require("ffprobe", "Gyan.FFmpeg");
        if (ffprobe == null) return;
        var input = Ui.AskFile("Media file");
        if (input == null) return;

        var json = Exec.Capture(ffprobe, "-v", "quiet", "-print_format", "json",
                                "-show_format", "-show_streams", input);
        if (string.IsNullOrWhiteSpace(json)) { Ui.Err("ffprobe returned no data."); return; }

        try
        {
            using var doc = JsonDocument.Parse(json);
            var root = doc.RootElement;

            Ui.Header(Path.GetFileName(input));
            if (root.TryGetProperty("format", out var fmt))
            {
                Ui.Label("Container", Str(fmt, "format_long_name"));
                if (double.TryParse(Str(fmt, "duration"), out double dur))
                    Ui.Label("Duration", TimeSpan.FromSeconds(dur).ToString(@"hh\:mm\:ss"));
                if (double.TryParse(Str(fmt, "bit_rate"), out double br))
                    Ui.Label("Bitrate", $"{br / 1000:0} kb/s");
                if (double.TryParse(Str(fmt, "size"), out double size))
                    Ui.Label("Size", Ui.HumanSize(size));
            }

            if (root.TryGetProperty("streams", out var streams))
            {
                foreach (var s in streams.EnumerateArray())
                {
                    string type = Str(s, "codec_type");
                    Ui.Blank();
                    Ui.Write($"  [{type}] {Str(s, "codec_name")}", ConsoleColor.Yellow);
                    if (type == "video")
                    {
                        Ui.Label("Resolution", $"{Str(s, "width")}x{Str(s, "height")}");
                        Ui.Label("Frame rate", Str(s, "r_frame_rate"));
                        Ui.Label("Pixel format", Str(s, "pix_fmt"));
                    }
                    else if (type == "audio")
                    {
                        Ui.Label("Sample rate", $"{Str(s, "sample_rate")} Hz");
                        Ui.Label("Channels", Str(s, "channels"));
                        if (double.TryParse(Str(s, "bit_rate"), out double sbr))
                            Ui.Label("Bitrate", $"{sbr / 1000:0} kb/s");
                    }
                }
            }
        }
        catch (JsonException)
        {
            Ui.Err("Could not parse ffprobe output.");
        }
    }

    private static string Str(JsonElement el, string prop) =>
        el.TryGetProperty(prop, out var v) ? v.ToString() : "-";
}
