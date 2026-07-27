// === NetMaint — Maintenance page (Cleanup.ps1 + Backup.ps1, fully native) ===
namespace NetMaint.Pages;

public class MaintenancePage : UserControl
{
    // Same targets as Cleanup.ps1
    private static (string Name, string Path)[] CleanTargets => new[]
    {
        ("User Temp",      Path.GetTempPath()),
        ("Windows Temp",   @"C:\Windows\Temp"),
        ("Prefetch",       @"C:\Windows\Prefetch"),
        ("Chrome cache",   Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), @"Google\Chrome\User Data\Default\Cache")),
        ("Edge cache",     Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), @"Microsoft\Edge\User Data\Default\Cache")),
    };

    private readonly DataGridView _cleanGrid;
    private readonly Button _analyze;
    private readonly Button _clean;
    private readonly Label _cleanTotal;

    private readonly TextBox _src;
    private readonly TextBox _dst;
    private readonly Label _backupStatus;
    private readonly ProgressBar _backupProgress;
    private readonly Button _backupBtn;
    private readonly Button _cancelBtn;
    private CancellationTokenSource? _backupCts;

    public MaintenancePage()
    {
        BackColor = Theme.Bg;

        // ---------- Cleanup ----------
        var cleanHead = Theme.Heading("System Cleanup");
        cleanHead.Location = new Point(20, 16);

        _analyze = Theme.Button("Analyze sizes", 120);
        _analyze.Location = new Point(20, 54);
        _analyze.Click += async (_, _) => await AnalyzeAsync();

        _clean = Theme.AccentButton("Clean checked", 130);
        _clean.Location = new Point(150, 54);
        _clean.Click += async (_, _) => await CleanAsync();

        _cleanTotal = Theme.Label("");
        _cleanTotal.Location = new Point(300, 61);
        _cleanTotal.Font = Theme.Semibold;

        _cleanGrid = Theme.Grid();
        _cleanGrid.Location = new Point(20, 94);
        _cleanGrid.Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right;
        _cleanGrid.Height = 205;
        _cleanGrid.ReadOnly = false;
        var check = new DataGridViewCheckBoxColumn { HeaderText = "", Name = "sel", FillWeight = 8 };
        _cleanGrid.Columns.Add(check);
        _cleanGrid.Columns.Add("target", "Target");
        _cleanGrid.Columns.Add("size", "Size");
        _cleanGrid.Columns.Add("result", "Result");
        _cleanGrid.Columns["target"]!.ReadOnly = true;
        _cleanGrid.Columns["size"]!.ReadOnly = true;
        _cleanGrid.Columns["result"]!.ReadOnly = true;
        _cleanGrid.Columns["target"]!.FillWeight = 30;
        _cleanGrid.Columns["size"]!.FillWeight = 18;
        _cleanGrid.Columns["result"]!.FillWeight = 44;

        foreach (var t in CleanTargets)
            _cleanGrid.Rows.Add(true, t.Name, "-", "");
        _cleanGrid.Rows.Add(true, "Recycle Bin", "-", "");

        // ---------- Backup ----------
        var backupHead = Theme.Heading("Quick Backup");
        backupHead.Location = new Point(20, 320);

        var srcLbl = Theme.Label("Source:", dim: true);
        srcLbl.Location = new Point(20, 366);
        _src = Theme.TextBox(430);
        _src.Location = new Point(90, 362);
        var srcFile = Theme.Button("File...", 70);
        srcFile.Location = new Point(530, 359);
        srcFile.Click += (_, _) => PickSourceFile();
        var srcDir = Theme.Button("Folder...", 80);
        srcDir.Location = new Point(605, 359);
        srcDir.Click += (_, _) => PickFolder(_src);

        var dstLbl = Theme.Label("Destination:", dim: true);
        dstLbl.Location = new Point(20, 404);
        _dst = Theme.TextBox(430);
        _dst.Location = new Point(90, 400);
        var dstDir = Theme.Button("Folder...", 80);
        dstDir.Location = new Point(530, 397);
        dstDir.Click += (_, _) => PickFolder(_dst);

        _backupBtn = Theme.AccentButton("Backup now", 120);
        _backupBtn.Location = new Point(20, 442);
        _backupBtn.Click += async (_, _) => await BackupAsync();

        _cancelBtn = Theme.Button("Cancel", 80);
        _cancelBtn.Location = new Point(150, 442);
        _cancelBtn.Enabled = false;
        _cancelBtn.Click += (_, _) => _backupCts?.Cancel();

        _backupProgress = new ProgressBar
        {
            Location = new Point(20, 484),
            Size = new Size(665, 22),
            Anchor = AnchorStyles.Top | AnchorStyles.Left | AnchorStyles.Right
        };

        _backupStatus = Theme.Label("", dim: true);
        _backupStatus.Location = new Point(20, 514);
        _backupStatus.MaximumSize = new Size(900, 0);

        Controls.Add(cleanHead);
        Controls.Add(_analyze);
        Controls.Add(_clean);
        Controls.Add(_cleanTotal);
        Controls.Add(_cleanGrid);
        Controls.Add(backupHead);
        Controls.Add(srcLbl);
        Controls.Add(_src);
        Controls.Add(srcFile);
        Controls.Add(srcDir);
        Controls.Add(dstLbl);
        Controls.Add(_dst);
        Controls.Add(dstDir);
        Controls.Add(_backupBtn);
        Controls.Add(_cancelBtn);
        Controls.Add(_backupProgress);
        Controls.Add(_backupStatus);

        Resize += (_, _) => _cleanGrid.Width = Width - 40;
        _cleanGrid.Width = Width - 40;
    }

    // ================= Cleanup =================

    private static long DirSize(string path)
    {
        long total = 0;
        try
        {
            foreach (var f in Directory.EnumerateFiles(path, "*", new EnumerationOptions
                     { RecurseSubdirectories = true, IgnoreInaccessible = true, AttributesToSkip = 0 }))
            {
                try { total += new FileInfo(f).Length; } catch { }
            }
        }
        catch { }
        return total;
    }

    private async Task AnalyzeAsync()
    {
        _analyze.Enabled = false;
        for (int i = 0; i < CleanTargets.Length; i++)
        {
            string path = CleanTargets[i].Path;
            _cleanGrid.Rows[i].Cells[2].Value = "...";
            long size = await Task.Run(() => Directory.Exists(path) ? DirSize(path) : 0);
            _cleanGrid.Rows[i].Cells[2].Value = Theme.HumanBytes(size);
        }
        var (binBytes, binItems) = Native.RecycleBinSize();
        _cleanGrid.Rows[CleanTargets.Length].Cells[2].Value =
            $"{Theme.HumanBytes(binBytes)} ({binItems} items)";
        _analyze.Enabled = true;
    }

    private async Task CleanAsync()
    {
        var confirm = MessageBox.Show(
            "This deletes temp files, browser caches and empties the Recycle Bin.\n" +
            "Files currently in use are skipped automatically.\n\nContinue?",
            "System Cleanup", MessageBoxButtons.YesNo, MessageBoxIcon.Warning);
        if (confirm != DialogResult.Yes) return;

        _clean.Enabled = false;
        _analyze.Enabled = false;
        long totalFreed = 0;

        for (int i = 0; i < CleanTargets.Length; i++)
        {
            var row = _cleanGrid.Rows[i];
            if (!(row.Cells[0].Value as bool? ?? false)) { row.Cells[3].Value = "skipped"; continue; }

            string path = CleanTargets[i].Path;
            row.Cells[3].Value = "cleaning...";
            long freed = await Task.Run(() =>
            {
                if (!Directory.Exists(path)) return 0L;
                // Measure before, delete what we can, re-measure: freed = what actually went away.
                long before = DirSize(path);
                DeleteContents(path);
                long after = DirSize(path);
                return Math.Max(0, before - after);
            });
            totalFreed += freed;

            if (freed > 0)
            {
                row.Cells[3].Value = $"{Theme.HumanBytes(freed)} freed";
                row.Cells[3].Style.ForeColor = Theme.Good;
            }
            else
            {
                row.Cells[3].Value = Directory.Exists(path) && DirSize(path) > 0
                    ? "in use / locked (skipped)" : "already clean";
                row.Cells[3].Style.ForeColor = Theme.TextDim;
            }
        }

        // Recycle bin
        var binRow = _cleanGrid.Rows[CleanTargets.Length];
        if (binRow.Cells[0].Value as bool? ?? false)
        {
            var (binBytes, _) = Native.RecycleBinSize();
            bool ok = await Task.Run(Native.EmptyRecycleBin);
            if (ok && binBytes > 0)
            {
                totalFreed += binBytes;
                binRow.Cells[3].Value = $"emptied ({Theme.HumanBytes(binBytes)})";
                binRow.Cells[3].Style.ForeColor = Theme.Good;
            }
            else
            {
                binRow.Cells[3].Value = "already empty or skipped";
                binRow.Cells[3].Style.ForeColor = Theme.TextDim;
            }
        }

        _cleanTotal.Text = $"TOTAL FREED: {Theme.HumanBytes(totalFreed)}";
        _cleanTotal.ForeColor = Theme.Accent;
        _clean.Enabled = true;
        _analyze.Enabled = true;
    }

    private static void DeleteContents(string dir)
    {
        try
        {
            foreach (var f in Directory.EnumerateFiles(dir))
            {
                try { File.SetAttributes(f, FileAttributes.Normal); File.Delete(f); } catch { }
            }
            foreach (var d in Directory.EnumerateDirectories(dir))
            {
                try { Directory.Delete(d, recursive: true); } catch { }
            }
        }
        catch { }
    }

    // ================= Backup =================

    private void PickSourceFile()
    {
        using var dlg = new OpenFileDialog { Title = "Backup source file" };
        if (dlg.ShowDialog(this) == DialogResult.OK) _src.Text = dlg.FileName;
    }

    private void PickFolder(TextBox target)
    {
        using var dlg = new FolderBrowserDialog();
        if (dlg.ShowDialog(this) == DialogResult.OK) target.Text = dlg.SelectedPath;
    }

    private async Task BackupAsync()
    {
        string src = _src.Text.Trim().Trim('"');
        string dstRoot = _dst.Text.Trim().Trim('"');

        bool isFile = File.Exists(src);
        if (!isFile && !Directory.Exists(src))
        {
            _backupStatus.Text = $"Source does not exist: {src}";
            _backupStatus.ForeColor = Theme.Bad;
            return;
        }
        if (dstRoot.Length == 0)
        {
            _backupStatus.Text = "Choose a destination folder.";
            _backupStatus.ForeColor = Theme.WarnCol;
            return;
        }

        // Same naming convention as Backup.ps1: name_backup_yyyy-MM-dd_HH-mm
        string name = Path.GetFileName(src.TrimEnd('\\', '/'));
        string backupPath = Path.Combine(dstRoot, $"{name}_backup_{DateTime.Now:yyyy-MM-dd_HH-mm}");

        _backupBtn.Enabled = false;
        _cancelBtn.Enabled = true;
        _backupCts = new CancellationTokenSource();
        var started = DateTime.Now;

        try
        {
            var files = await Task.Run(() => isFile
                ? new List<(string Full, string Rel)> { (src, Path.GetFileName(src)) }
                : Directory.EnumerateFiles(src, "*", new EnumerationOptions
                      { RecurseSubdirectories = true, IgnoreInaccessible = true, AttributesToSkip = 0 })
                    .Select(f => (Full: f, Rel: Path.GetRelativePath(src, f)))
                    .ToList());

            long totalBytes = await Task.Run(() =>
            {
                long t = 0;
                foreach (var f in files) { try { t += new FileInfo(f.Full).Length; } catch { } }
                return t;
            });

            _backupStatus.Text = $"Copying {files.Count} files ({Theme.HumanBytes(totalBytes)}) to {backupPath} ...";
            _backupStatus.ForeColor = Theme.TextDim;

            long copied = 0;
            int failed = 0;
            var progress = new Progress<long>(bytes =>
            {
                int pct = totalBytes > 0 ? (int)(bytes * 100 / totalBytes) : 100;
                _backupProgress.Value = Math.Min(100, pct);
            });

            await Task.Run(() =>
            {
                var buffer = new byte[1024 * 1024];
                foreach (var f in files)
                {
                    _backupCts.Token.ThrowIfCancellationRequested();
                    try
                    {
                        string dest = Path.Combine(backupPath, f.Rel);
                        Directory.CreateDirectory(Path.GetDirectoryName(dest)!);
                        using var inStream = File.OpenRead(f.Full);
                        using var outStream = File.Create(dest);
                        int read;
                        while ((read = inStream.Read(buffer, 0, buffer.Length)) > 0)
                        {
                            _backupCts.Token.ThrowIfCancellationRequested();
                            outStream.Write(buffer, 0, read);
                            copied += read;
                            ((IProgress<long>)progress).Report(copied);
                        }
                        File.SetLastWriteTimeUtc(dest, File.GetLastWriteTimeUtc(f.Full));
                    }
                    catch (OperationCanceledException) { throw; }
                    catch { failed++; }
                }
            }, _backupCts.Token);

            var elapsed = DateTime.Now - started;
            _backupProgress.Value = 100;
            _backupStatus.Text = $"Done in {(int)elapsed.TotalMinutes}m {elapsed.Seconds}s — saved to {backupPath}"
                                 + (failed > 0 ? $"  ({failed} files skipped)" : "");
            _backupStatus.ForeColor = failed > 0 ? Theme.WarnCol : Theme.Good;
        }
        catch (OperationCanceledException)
        {
            _backupStatus.Text = $"Cancelled — partial backup left at {backupPath}";
            _backupStatus.ForeColor = Theme.WarnCol;
        }
        catch (Exception ex)
        {
            _backupStatus.Text = "Backup failed: " + ex.Message;
            _backupStatus.ForeColor = Theme.Bad;
        }
        finally
        {
            _backupBtn.Enabled = true;
            _cancelBtn.Enabled = false;
        }
    }
}
