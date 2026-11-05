using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Media;
using RdBatch.Core;

namespace RdBatch.App;

public partial class MainWindow
{
    public static RoutedUICommand RemoveFolderCommand { get; } = new("Remove folder", nameof(RemoveFolderCommand), typeof(MainWindow));
    public ObservableCollection<TorrentDraft> Library { get; } = [];
    public ObservableCollection<TorrentFolder> BrowserFolders { get; } = [];
    public ObservableCollection<BrowserFile> BrowserFiles { get; } = [];
    private readonly Stack<BrowserRemoval> _removedFiles = new();
    private CancellationTokenSource? _inspectCancellation;
    private Task? _inspectTask;
    private TorrentFolder? _currentFolder;
    private bool _refreshingBrowser;

    private async Task<bool> BrowseInputsAsync(string text)
    {
        if (_inspectCancellation is not null || _clearing || _closing || _checking) return false;
        var parsed = InputParser.Parse(text);
        if (parsed.Sources.Count == 0)
        {
            SetStatus("Dán magnet hoặc chọn file .torrent để xem danh sách file.");
            StatusText.ToolTip = string.Join(Environment.NewLine, parsed.Errors);
            return false;
        }
        var token = TokenBox.Password.Trim();
        if (token.Length == 0) { SetStatus("Dán API token trước khi kiểm tra."); TokenBox.Focus(); return false; }
        using var cancellation = new CancellationTokenSource();
        _inspectCancellation = cancellation;
        UpdateControls();
        WorkspaceTabs.SelectedIndex = 0;
        var failures = new List<string>(parsed.Errors);
        try
        {
            var api = new RealDebridClient(_apiHttp, token);
            var engine = new DownloadEngine(api, _transferHttp);
            var user = await api.GetUserAsync(cancellation.Token);
            _verifiedToken = token; _user = user;
            SetAccount("{0} · {1}", user.Username, user.Type);
            if (user.Type != "premium" || user.Premium <= 0)
                throw new InvalidOperationException(Text.Get("Cần tài khoản Real-Debrid Premium còn hạn để tải torrent."));
            SaveToken();
            foreach (var source in parsed.Sources)
            {
                cancellation.Token.ThrowIfCancellationRequested();
                var draft = Library.FirstOrDefault(item => item.SourceKey.Equals(source.Key, StringComparison.OrdinalIgnoreCase));
                if (draft is null)
                {
                    draft = new() { Source = source.Source, SourceKey = source.Key, Name = source.Name };
                    Library.Add(draft);
                }
                if (draft.IsHidden) { draft.IsHidden = false; draft.ExcludedFileIds.Clear(); }
                if (draft.Files.Count > 0) continue;
                SetStatus("Đang đọc file trong {0}…", draft.Name);
                try
                {
                    if (draft.TorrentId is null)
                    {
                        draft.TorrentId = await engine.AddSourceAsync(draft.Source, cancellation.Token);
                        draft.RemoteAccount = user.Username;
                    }
                    Save(); // Keep remote IDs even if metadata takes time or the app closes.
                    var info = await engine.InspectAsync(draft.TorrentId, cancellation.Token);
                    draft.RemoteAccount = user.Username;
                    draft.Name = string.IsNullOrWhiteSpace(info.Filename) ? draft.Name : info.Filename;
                    draft.Files = info.Files.ToList();
                    Save(); RebuildFolders();
                }
                catch (OperationCanceledException) when (cancellation.IsCancellationRequested) { throw; }
                catch (Exception ex) { failures.Add(draft.Name + ": " + DownloadEngine.FriendlyError(ex)); }
            }
            SetStatus("Đã đọc {0} torrent · {1} file · {2} lỗi. Chọn file để thêm vào hàng đợi.",
                Library.Count(item => item.Files.Count > 0), Library.Sum(item => item.Files.Count), failures.Count);
            StatusText.ToolTip = failures.Count == 0 ? null : string.Join(Environment.NewLine, failures);
            return failures.Count == 0;
        }
        catch (OperationCanceledException) when (cancellation.IsCancellationRequested) { SetStatus("Đã dừng đọc torrent. Bấm Thêm vào thư viện để tiếp tục."); return false; }
        catch (Exception ex) { SetStatus(DownloadEngine.FriendlyErrorKey(ex)); return false; }
        finally { _inspectCancellation = null; Save(); RebuildFolders(); UpdateControls(); }
    }

    private async Task InspectSourcesAsync(string text)
    {
        if (_inspectTask is not null) return;
        var task = BrowseInputsAsync(text);
        _inspectTask = task;
        try
        {
            if (await task)
            {
                var current = SourcesBox.Text;
                if (current == text) SourcesBox.Clear();
                else if (current.StartsWith(text, StringComparison.Ordinal) && current.Length > text.Length && current[text.Length] is '\r' or '\n')
                    SourcesBox.Text = current[text.Length..].TrimStart('\r', '\n');
            }
        }
        finally { _inspectTask = null; }
    }

    private void CancelInspection(object sender, RoutedEventArgs e) => _inspectCancellation?.Cancel();

    private void RebuildFolders()
    {
        var previous = _currentFolder;
        var root = new TorrentFolder { Name = Text.Get("Tất cả torrent"), IsExpanded = true };
        foreach (var draft in Library)
        {
            if (draft.IsHidden || draft.Files.Count > 0 && !draft.VisibleFiles.Any()) continue;
            var torrent = new TorrentFolder { Name = draft.Name, Torrent = draft, IsExpanded = true };
            root.Children.Add(torrent);
            foreach (var file in draft.VisibleFiles)
            {
                var parts = TorrentFileSearch.Normalize(file.Path).Trim('/').Split('/');
                var node = torrent;
                var path = "";
                foreach (var part in parts.SkipLast(1))
                {
                    path = path.Length == 0 ? part : path + "/" + part;
                    if (node == torrent && path.Equals(draft.Name, StringComparison.OrdinalIgnoreCase)) continue;
                    var child = node.Children.FirstOrDefault(folder => folder.Path == path);
                    if (child is null)
                    {
                        child = new() { Name = part, Path = path, Torrent = draft };
                        node.Children.Add(child);
                    }
                    node = child;
                }
            }
        }
        BrowserFolders.Clear(); BrowserFolders.Add(root);
        _currentFolder = FindFolder(root, previous?.Torrent, previous?.Path ?? "") ?? root;
        _currentFolder.IsSelected = true;
        RefreshBrowser(keepSelection: true);
    }

    private static TorrentFolder? FindFolder(TorrentFolder node, TorrentDraft? draft, string path)
        => node.Torrent == draft && node.Path == path ? node : node.Children.Select(child => FindFolder(child, draft, path)).FirstOrDefault(child => child is not null);

    private void BrowserFolderChanged(object sender, RoutedPropertyChangedEventArgs<object> e)
    {
        if (e.NewValue is not TorrentFolder folder) return;
        if (ReferenceEquals(_currentFolder, folder)) return;
        _currentFolder = folder; RefreshBrowser();
    }

    private void BrowserFilterChanged(object sender, RoutedEventArgs e)
    { if (!_initializing) RefreshBrowser(); }

    private void ShowAllFiles(object sender, RoutedEventArgs e)
    {
        _currentFolder = BrowserFolders.FirstOrDefault();
        if (_currentFolder is not null) _currentFolder.IsSelected = true;
        SearchFilesBox.Clear(); VideoOnlyBox.IsChecked = false; RefreshBrowser();
    }

    private void RefreshBrowser(bool keepSelection = false)
    {
        if (_initializing) return;
        _refreshingBrowser = true;
        var anchor = keepSelection ? _fileSelection.Anchor : null;
        var selected = keepSelection ? BrowserFiles.Where(file => file.IsSelected).Select(file => (file.Torrent, file.File.Id)).ToHashSet() : [];
        foreach (var file in BrowserFiles) file.PropertyChanged -= BrowserSelectionChanged;
        BrowserFiles.Clear();
        var drafts = _currentFolder?.Torrent is { } draft ? new[] { draft } : Library.ToArray();
        foreach (var torrent in drafts)
        {
            var jobs = Jobs.Where(job => job.SourceKey.Equals(torrent.SourceKey, StringComparison.OrdinalIgnoreCase)).ToArray();
            foreach (var file in TorrentFileSearch.Filter(torrent.VisibleFiles, SearchFilesBox.Text, VideoOnlyBox.IsChecked == true, _currentFolder?.Path ?? "").OrderBy(file => file.Path, StringComparer.OrdinalIgnoreCase))
            {
                var row = new BrowserFile(torrent, file, jobs.Any(job => job.SelectedFileIds is null || job.SelectedFileIds.Contains(file.Id)));
                row.IsSelected = selected.Contains((torrent, file.Id));
                row.PropertyChanged += BrowserSelectionChanged;
                BrowserFiles.Add(row);
            }
        }
        BrowserPath.Text = _currentFolder?.Torrent is { } current
            ? current.Name + (_currentFolder.Path.Length > 0 ? " / " + _currentFolder.Path : "") : Text.Get("Tất cả torrent");
        BrowserSummary.Text = Text.Get("{0} kết quả · {1}", BrowserFiles.Count, DownloadJob.FormatBytes(BrowserFiles.Sum(file => file.File.Bytes)));
        BrowserEmpty.Visibility = BrowserFiles.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
        BrowserEmpty.Text = Text.Get(Library.Count == 0 ? "Dán link torrent ở trên để duyệt thư mục và chọn video." : "Không tìm thấy file. Thử từ khóa khác hoặc bỏ lọc video.");
        _fileSelection.Anchor = anchor is null ? null : BrowserFiles.FirstOrDefault(file => file.Torrent == anchor.Torrent && file.File.Id == anchor.File.Id);
        _refreshingBrowser = false;
        UpdateBrowserSelection();
    }

    private void BrowserSelectionChanged(object? sender, PropertyChangedEventArgs e)
    {
        if (!_refreshingBrowser && e.PropertyName == nameof(BrowserFile.IsSelected))
        { _fileSelection.Anchor = sender as BrowserFile; UpdateBrowserSelection(); }
    }

    private void UpdateBrowserSelection()
    {
        var selected = BrowserFiles.Where(file => file.IsSelected).ToArray();
        SelectionSummary.Text = Text.Get("Đã chọn {0} file · {1}", selected.Length, DownloadJob.FormatBytes(selected.Sum(file => file.File.Bytes)));
        QueueFilesButton.IsEnabled = selected.Any(file => file.CanQueue) && !_closing && !_clearing;
        SelectResultsButton.IsEnabled = BrowserFiles.Count > 0 && !_closing && !_clearing;
        RemoveBrowserFilesButton.IsEnabled = selected.Length > 0 && !_closing && !_clearing;
        RemoveBrowserFolderButton.IsEnabled = _currentFolder?.CanRemove == true && !_closing && !_clearing;
        UndoBrowserRemovalButton.IsEnabled = _removedFiles.Count > 0 && !_closing && !_clearing;
        var remaining = Library.Sum(torrent => torrent.VisibleFiles.Count(file => !IsQueued(torrent, file.Id)));
        QueueRemainingButton.Content = Text.Get("Thêm tất cả còn lại ({0})", remaining);
        QueueRemainingButton.IsEnabled = !_closing && !_clearing && remaining > 0;
    }

    private void SelectResults(object sender, RoutedEventArgs e)
    {
        var select = BrowserFiles.Any(file => !file.IsSelected);
        _refreshingBrowser = true;
        foreach (var file in BrowserFiles) file.IsSelected = select;
        _fileSelection.Anchor = null;
        _refreshingBrowser = false;
        UpdateBrowserSelection();
    }

    private void QueueSelectedFiles(object sender, RoutedEventArgs e)
    {
        if (_clearing || _closing) return;
        AddFilesToQueue(BrowserFiles.Where(file => file.IsSelected && file.CanQueue).ToArray());
    }

    private bool IsQueued(TorrentDraft torrent, int fileId)
        => Jobs.Any(job => job.SourceKey.Equals(torrent.SourceKey, StringComparison.OrdinalIgnoreCase) &&
            (job.SelectedFileIds is null || job.SelectedFileIds.Contains(fileId)));

    private void QueueRemainingFiles(object sender, RoutedEventArgs e)
    {
        if (_clearing || _closing) return;
        AddFilesToQueue(Library.SelectMany(torrent => torrent.VisibleFiles.Where(file => !IsQueued(torrent, file.Id))
            .Select(file => new BrowserFile(torrent, file, false))).ToArray());
    }

    private void AddFilesToQueue(BrowserFile[] selected)
    {
        foreach (var group in selected.GroupBy(file => file.Torrent))
        {
            var draft = group.Key;
            var ids = group.Select(file => file.File.Id).Distinct().ToList();
            var job = new DownloadJob
            {
                Source = draft.Source, SourceKey = draft.SourceKey, Name = draft.Name,
                TorrentId = draft.TorrentId, RemoteAccount = draft.TorrentId is null ? null : draft.RemoteAccount,
                SelectedFileIds = ids, TotalBytes = group.Sum(file => file.File.Bytes)
            };
            job.SetDetail("Đã chọn {0} file · {1}", ids.Count, string.Join(", ", group.Select(file => file.Name)));
            Jobs.Add(job);
            // A later selection needs its own remote torrent; an active selection cannot be changed.
            draft.TorrentId = null;
        }
        Save(); RefreshBrowser(); RefreshSummary(); UpdateControls();
        SetStatus("Đã thêm {0} file đã chọn vào hàng đợi. Bấm Bắt đầu để tải.", selected.Length);
    }

    private void RemoveBrowserFiles(object sender, RoutedEventArgs e)
    {
        if (_closing || _clearing) return;
        var removed = BrowserFiles.Where(file => file.IsSelected).Select(file => (file.Torrent, file.File.Id)).ToArray();
        if (removed.Length == 0) return;
        foreach (var (torrent, id) in removed) torrent.ExcludedFileIds.Add(id);
        _removedFiles.Push(new(removed, []));
        Save(); RebuildFolders();
        SetStatus("Đã bỏ {0} file khỏi thư viện ảo. Có thể Hoàn tác; hàng đợi đã tạo giữ nguyên.", removed.Length);
    }

    private void UndoBrowserRemoval(object sender, RoutedEventArgs e)
    {
        if (_closing || _clearing || !_removedFiles.TryPop(out var removed)) return;
        foreach (var (torrent, id) in removed.Files) torrent.ExcludedFileIds.Remove(id);
        foreach (var torrent in removed.HiddenTorrents) torrent.IsHidden = false;
        Save(); RebuildFolders();
        SetStatus("Đã hoàn tác thao tác xóa trong thư viện ảo.");
    }

    private void RemoveSelectedBrowserFolder(object sender, RoutedEventArgs e)
    { if (_currentFolder is { } folder) RemoveFolder(folder); }

    private void RemoveFolder(TorrentFolder folder)
    {
        if (_closing || _clearing || folder.Torrent is not { } torrent || !Library.Contains(torrent) || torrent.IsHidden) return;
        var files = TorrentFileSearch.Filter(torrent.VisibleFiles, "", false, folder.Path).ToArray();
        if (folder.Path.Length == 0)
        {
            // Hide the torrent root even if metadata is empty; keep its ID for remote management.
            torrent.IsHidden = true;
            _removedFiles.Push(new([], [torrent]));
        }
        else
        {
            if (files.Length == 0) return;
            foreach (var file in files) torrent.ExcludedFileIds.Add(file.Id);
            _removedFiles.Push(new(files.Select(file => (torrent, file.Id)).ToArray(), []));
        }
        Save(); RebuildFolders(); FolderTree.Focus();
        SetStatus("Đã bỏ thư mục {0} và {1} file khỏi thư viện ảo. Có thể Hoàn tác.", folder.Name, files.Length);
    }

    private void FolderRightClick(object sender, MouseButtonEventArgs e)
    {
        var element = e.OriginalSource as DependencyObject;
        while (element is not null && element is not TreeViewItem)
            element = element is FrameworkContentElement content ? content.Parent : VisualTreeHelper.GetParent(element);
        if (element is TreeViewItem item) { item.IsSelected = true; item.Focus(); }
    }

    private void RemoveFolderExecuted(object sender, ExecutedRoutedEventArgs e)
    { if (e.Parameter is TorrentFolder folder) RemoveFolder(folder); e.Handled = true; }

    private void CanRemoveFolder(object sender, CanExecuteRoutedEventArgs e)
    { e.CanExecute = !_closing && !_clearing && e.Parameter is TorrentFolder { CanRemove: true }; e.Handled = true; }

    private void BrowserFoldersKeyDown(object sender, KeyEventArgs e)
    {
        if (e.Key == Key.Delete) { RemoveSelectedBrowserFolder(sender, e); e.Handled = true; }
        else if (Keyboard.Modifiers == ModifierKeys.Control && e.Key == Key.Z)
        { UndoBrowserRemoval(sender, e); e.Handled = true; }
    }

    private void AppendSources(string text)
    {
        if (string.IsNullOrWhiteSpace(text)) return;
        SourcesBox.Text += (string.IsNullOrWhiteSpace(SourcesBox.Text) ? "" : Environment.NewLine) + text;
    }

    private void BrowserViewChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_initializing) return;
        _settings.BrowserView = BrowserViewBox.SelectedIndex == 1 ? "grid" : "list";
        ApplyBrowserView(); Save();
    }

    private void ApplyBrowserView()
    {
        var grid = BrowserViewBox.SelectedIndex == 1;
        BrowserGrid.Visibility = grid ? Visibility.Collapsed : Visibility.Visible;
        BrowserTilesScroll.Visibility = grid ? Visibility.Visible : Visibility.Collapsed;
    }

    private void BrowserFilesKeyDown(object sender, KeyEventArgs e)
    {
        if (e.Key == Key.Delete)
        {
            RemoveBrowserFiles(sender, e); e.Handled = true;
        }
        else if (Keyboard.Modifiers == ModifierKeys.Control && e.Key == Key.A)
        {
            _refreshingBrowser = true;
            foreach (var file in BrowserFiles) file.IsSelected = true;
            _fileSelection.Anchor = null;
            _refreshingBrowser = false; UpdateBrowserSelection(); e.Handled = true;
        }
        else if (Keyboard.Modifiers == ModifierKeys.Control && e.Key == Key.Z)
        {
            UndoBrowserRemoval(sender, e); e.Handled = true;
        }
    }
}
