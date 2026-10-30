using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Net.Http;
using System.Security.Cryptography;
using System.Text;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Threading;
using Microsoft.Win32;
using RdBatch.Core;

namespace RdBatch.App;

public partial class MainWindow : Window
{
    public ObservableCollection<DownloadJob> Jobs { get; } = [];
    private readonly SettingsStore _store;
    private readonly AppSettings _settings;
    private readonly HttpClient _apiHttp = new() { Timeout = TimeSpan.FromSeconds(60) };
    private readonly HttpClient _transferHttp = new() { Timeout = Timeout.InfiniteTimeSpan };
    private readonly DispatcherTimer _timer = new() { Interval = TimeSpan.FromSeconds(1) };
    private CancellationTokenSource? _runCancellation;
    private Task? _runTask;
    private DownloadSession? _session;
    private readonly HashSet<DownloadJob> _pendingStarts = [];
    private bool _initializing = true;
    private bool _closing;
    private bool _readyToClose;
    private bool _checking;
    private bool _clearing;
    private Task? _clearTask;
    private LocalizedMessage _statusMessage = LocalizedMessage.Create("Sẵn sàng · Token được mã hóa bằng tài khoản Windows khi lưu.");
    private LocalizedMessage _accountMessage = LocalizedMessage.Create("Chưa kết nối");
    private bool _storageErrorShown;
    private string _verifiedToken = "";
    private RdUser? _user;
    private bool Running => _runCancellation is not null;

    public MainWindow(SettingsStore store, AppSettings settings)
    {
        _store = store;
        _settings = settings;
        InitializeComponent();
        LanguageBox.SelectedIndex = Text.Language == "en" ? 1 : 0;
        var workArea = SystemParameters.WorkArea;
        MinHeight = Math.Min(MinHeight, workArea.Height - 24);
        MinWidth = Math.Min(MinWidth, workArea.Width - 24);
        Height = Math.Min(Height, workArea.Height - 24);
        Width = Math.Min(Width, workArea.Width - 24);
        DataContext = this;
        foreach (var job in _settings.Jobs) Jobs.Add(job);
        FolderBox.Text = _settings.OutputDirectory;
        FlattenFilesBox.IsChecked = _settings.FlattenFiles;
        ConcurrencyBox.SelectedIndex = _settings.Concurrency - 1;
        if (!string.IsNullOrEmpty(_settings.ProtectedToken))
        {
            try { TokenBox.Password = Encoding.UTF8.GetString(ProtectedData.Unprotect(Convert.FromBase64String(_settings.ProtectedToken), null, DataProtectionScope.CurrentUser)); }
            catch (Exception ex) when (ex is CryptographicException or FormatException)
            { SetStatus("Không giải mã được token cũ trên tài khoản Windows này. Hãy dán lại token."); }
        }
        _initializing = false;
        if (_store.LoadWarningMessage is not null) SetStatus(_store.LoadWarningMessage.Key, _store.LoadWarningMessage.Arguments.Cast<object?>().ToArray());
        _timer.Tick += (_, _) => RefreshSummary();
        _timer.Start();
        UpdateControls();
        RefreshSummary();
    }

    private void SetStatus(string key, params object?[] arguments)
    { _statusMessage = LocalizedMessage.Create(key, arguments); StatusText.Text = _statusMessage.Render(); }
    private void SetAccount(string key, params object?[] arguments)
    { _accountMessage = LocalizedMessage.Create(key, arguments); AccountStatus.Text = _accountMessage.Render(); }
    private void LanguageChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_initializing || LanguageBox.SelectedItem is not ComboBoxItem { Tag: string language }) return;
        Text.SetLanguage(language);
        _settings.Language = Text.Language;
        foreach (var job in Jobs) job.RefreshLanguage();
        StatusText.Text = _statusMessage.Render();
        AccountStatus.Text = _accountMessage.Render();
        StatusText.ToolTip = null;
        RefreshSummary();
        Save();
    }

    private void SourcesChanged(object sender, TextChangedEventArgs e)
    { if (SourcePlaceholder is not null) SourcePlaceholder.Visibility = string.IsNullOrEmpty(SourcesBox.Text) ? Visibility.Visible : Visibility.Collapsed; }

    private void PasteSources(object sender, RoutedEventArgs e)
    {
        try { if (Clipboard.ContainsText()) SourcesBox.Text += (string.IsNullOrWhiteSpace(SourcesBox.Text) ? "" : Environment.NewLine) + Clipboard.GetText(); }
        catch (System.Runtime.InteropServices.COMException) { SetStatus("Clipboard đang bận. Thử Ctrl+V vào ô nhập."); }
    }

    private void PickTorrents(object sender, RoutedEventArgs e)
    {
        var dialog = new OpenFileDialog { Filter = "Torrent files (*.torrent)|*.torrent", Multiselect = true, Title = Text.Get("Chọn các file torrent") };
        if (dialog.ShowDialog(this) == true) AddInputs(string.Join(Environment.NewLine, dialog.FileNames));
    }
    private void AddSources(object sender, RoutedEventArgs e)
    { if (AddInputs(SourcesBox.Text)) SourcesBox.Clear(); }

    private bool AddInputs(string text)
    {
        if (_clearing || _closing) return false;
        var parsed = InputParser.Parse(text);
        var added = 0;
        var duplicates = 0;
        foreach (var source in parsed.Sources)
        {
            if (Jobs.Any(j => j.SourceKey.Equals(source.Key, StringComparison.OrdinalIgnoreCase))) { duplicates++; continue; }
            Jobs.Add(new() { Source = source.Source, SourceKey = source.Key, Name = source.Name });
            added++;
        }
        SetStatus("Đã thêm {0} torrent · {1} trùng · {2} dòng không hợp lệ. Toàn bộ file sẽ được chọn tự động.", added, duplicates, parsed.Errors.Count);
        if (parsed.Errors.Count > 0) StatusText.ToolTip = string.Join(Environment.NewLine, parsed.Errors);
        else StatusText.ToolTip = null;
        Save(); RefreshSummary(); UpdateControls();
        return parsed.Errors.Count == 0;
    }

    private void OnDragOver(object sender, DragEventArgs e)
    { e.Effects = !Running && !_clearing && !_closing && e.Data.GetDataPresent(DataFormats.FileDrop) ? DragDropEffects.Copy : DragDropEffects.None; e.Handled = true; }
    private void OnDrop(object sender, DragEventArgs e)
    {
        if (Running || _clearing || _closing || e.Data.GetData(DataFormats.FileDrop) is not string[] paths) return;
        AddInputs(string.Join(Environment.NewLine, paths.Where(p => p.EndsWith(".torrent", StringComparison.OrdinalIgnoreCase))));
        e.Handled = true;
    }

    private void BrowseFolder(object sender, RoutedEventArgs e)
    {
        var dialog = new OpenFolderDialog { Title = Text.Get("Chọn nơi lưu toàn bộ torrent"), FolderName = Directory.Exists(FolderBox.Text) ? FolderBox.Text : Environment.GetFolderPath(Environment.SpecialFolder.UserProfile) };
        if (dialog.ShowDialog(this) == true) { FolderBox.Text = dialog.FolderName; Save(); }
    }
    private void FolderChanged(object sender, RoutedEventArgs e) { if (!_initializing) Save(); }
    private void OutputLayoutChanged(object sender, RoutedEventArgs e)
    {
        if (_initializing) return;
        Save();
        SetStatus(FlattenFilesBox.IsChecked == true
            ? "Lưu phẳng: file được tải thẳng vào thư mục đã chọn; trùng tên sẽ thêm hậu tố. Torrent tải dở giữ đường dẫn cũ."
            : "Mỗi torrent một thư mục riêng, giữ cấu trúc file khi có thể. Torrent tải dở giữ đường dẫn cũ.");
    }
    private void ConcurrencyChanged(object sender, SelectionChangedEventArgs e) { if (!_initializing) Save(); }
    private void RememberChanged(object sender, RoutedEventArgs e) { if (!_initializing) { SaveToken(); Save(); } }
    private void TokenChanged(object sender, RoutedEventArgs e)
    {
        if (_initializing) return;
        _verifiedToken = ""; _user = null;
        SetAccount("Chưa kiểm tra token mới");
        // Erase a previously saved token immediately when it is edited. Save the new one on validation/start.
        _settings.ProtectedToken = null;
        Save();
    }

    private static void OpenExternal(string path) => Process.Start(new ProcessStartInfo(path) { UseShellExecute = true });
    private void OpenToken(object sender, RoutedEventArgs e)
    {
        try { OpenExternal("https://real-debrid.com/apitoken"); }
        catch (Exception ex) { SetStatus("Không mở được trình duyệt: {0}", ex.Message); }
    }
    private void OpenOutput(object sender, RoutedEventArgs e)
    {
        try
        {
            var path = (QueueGrid.SelectedItem as DownloadJob)?.OutputDirectory ?? FolderBox.Text.Trim();
            if (string.IsNullOrWhiteSpace(path)) return;
            Directory.CreateDirectory(path);
            OpenExternal(Path.GetFullPath(path));
        }
        catch (Exception ex) { SetStatus("Không mở được thư mục: {0}", ex.Message); }
    }

    private async void Connect(object sender, RoutedEventArgs e)
    {
        if (_checking || Running || _clearing || _closing) return;
        _checking = true; UpdateControls();
        try
        {
            var token = TokenBox.Password.Trim();
            if (token.Length == 0) throw new InvalidOperationException(Text.Get("Dán API token trước khi kiểm tra."));
            SetAccount("Đang kiểm tra…");
            var user = await new RealDebridClient(_apiHttp, token).GetUserAsync(CancellationToken.None);
            _verifiedToken = token; _user = user;
            SetAccount("{0} · {1}", user.Username, user.Type);
            SaveToken(); Save();
            SetStatus(user.Type == "premium" && user.Premium > 0 ? "Đã kết nối. Bạn có thể bắt đầu tải." : "Tài khoản chưa có Premium còn hạn để tải torrent.");
        }
        catch (Exception ex) { SetAccount("Chưa kết nối"); SetStatus(DownloadEngine.FriendlyErrorKey(ex)); }
        finally { _checking = false; UpdateControls(); }
    }

    private async void Start(object sender, RoutedEventArgs e)
    {
        await StartJobsAsync(Jobs.Where(j => j.CanResume).ToArray());
    }

    private async Task StartJobsAsync(DownloadJob[] jobs)
    {
        if (_checking || _closing || _clearing) return;
        if (jobs.Length == 0) { SetStatus("Thêm torrent hoặc bấm Retry ở dòng bị lỗi."); return; }
        if (Running)
        {
            if (_session is null || _runCancellation!.IsCancellationRequested)
            { SetStatus("Đang kết nối hoặc dừng hàng đợi. Chờ hoàn tất rồi thử lại."); return; }
            foreach (var job in jobs.Where(j => !j.IsActive && !j.IsRemoving && Jobs.Contains(j)))
                _ = _session.EnqueueAsync(job, _runCancellation.Token);
            Save(); UpdateControls(); RefreshSummary();
            return;
        }
        var token = TokenBox.Password.Trim();
        if (token.Length == 0) { SetStatus("Dán API token Real-Debrid trước khi tải."); TokenBox.Focus(); return; }
        string root;
        try
        {
            if (string.IsNullOrWhiteSpace(FolderBox.Text) || !Path.IsPathFullyQualified(FolderBox.Text.Trim())) throw new IOException(Text.Get("Chọn một đường dẫn lưu đầy đủ."));
            root = Path.GetFullPath(FolderBox.Text.Trim());
            Directory.CreateDirectory(root);
            var probe = Path.Combine(root, ".rd-write-test-" + Guid.NewGuid().ToString("N"));
            using (File.Create(probe)) { }
            File.Delete(probe);
            SaveToken(); Save();
        }
        catch (Exception ex) { SetStatus("Không thể chuẩn bị nơi lưu: {0}", ex.Message); return; }
        _runCancellation = new();
        foreach (var job in jobs) _pendingStarts.Add(job);
        UpdateControls();
        SetStatus("Đang kết nối Real-Debrid…");
        _runTask = RunQueueAsync(root, token, _runCancellation.Token);
        try { await _runTask; }
        finally
        {
            _session = null; _pendingStarts.Clear();
            _runCancellation.Dispose(); _runCancellation = null; _runTask = null;
            Save(); UpdateControls(); RefreshSummary();
            if (!_closing && !_clearing) SetStatus("{0} hoàn tất · {1} lỗi · {2} đã dừng. Bấm Retry hoặc Resume trên từng dòng.", Jobs.Count(j => j.State == JobState.Completed), Jobs.Count(j => j.State == JobState.Failed), Jobs.Count(j => j.State is JobState.Paused or JobState.Stopped));
        }
    }

    private async Task RunQueueAsync(string root, string token, CancellationToken ct)
    {
        try
        {
            var client = new RealDebridClient(_apiHttp, token);
            if (_verifiedToken != token || _user is null)
            {
                _user = await client.GetUserAsync(ct);
                _verifiedToken = token;
            }
            SetAccount("{0} · {1}", _user.Username, _user.Type);
            if (_user.Type != "premium" || _user.Premium <= 0) throw new InvalidOperationException(Text.Get("Cần tài khoản Real-Debrid Premium còn hạn để tải torrent."));
            var engine = new DownloadEngine(client, _transferHttp);
            engine.Changed += _ => { Save(); RefreshSummary(); UpdateControls(); };
            ct.ThrowIfCancellationRequested();
            using var session = engine.CreateSession(Jobs.ToArray(), root, _settings.Concurrency, _settings.FlattenFiles);
            _session = session;
            foreach (var job in _pendingStarts.ToArray())
            {
                _pendingStarts.Remove(job);
                if (Jobs.Contains(job) && !job.IsRemoving) _ = session.EnqueueAsync(job, ct);
            }
            UpdateControls();
            SetStatus("Đang xử lý · Retry, Resume, Stop và Xóa có thể thao tác riêng từng torrent.");
            // Retry/resume can enqueue more work while other torrents are still running.
            while (session.ActiveCount > 0) await session.WhenIdleAsync();
        }
        catch (OperationCanceledException) when (ct.IsCancellationRequested) { SetStatus("Đã tạm dừng."); }
        catch (Exception ex)
        {
            SetStatus(DownloadEngine.FriendlyErrorKey(ex));
            MessageBox.Show(this, StatusText.Text, Text.Get("Không thể bắt đầu"), MessageBoxButton.OK, MessageBoxImage.Information);
        }
        finally { _session = null; }
    }

    private void Pause(object sender, RoutedEventArgs e)
    { _runCancellation?.Cancel(); UpdateControls(); SetStatus("Đang tạm dừng và lưu tiến trình…"); }

    private async void StopAll(object sender, RoutedEventArgs e)
    {
        var stopping = StopJobsAsync(Jobs.Where(j => j.CanStop).ToArray());
        _runCancellation?.Cancel();
        UpdateControls();
        await stopping;
    }

    private async Task StopJobsAsync(DownloadJob[] jobs)
    {
        foreach (var job in jobs) _pendingStarts.Remove(job);
        var session = _session;
        if (session is not null) await Task.WhenAll(jobs.Select(session.StopAsync));
        else
            foreach (var job in jobs.Where(j => j.State is JobState.Queued or JobState.Paused or JobState.Stopped))
            { job.State = JobState.Stopped; job.SetDetail("Đã dừng · bấm Resume để tải tiếp"); }
        Save(); UpdateControls(); RefreshSummary();
        SetStatus("Đã dừng tác vụ đã chọn. File tạm được giữ để Resume; torrent trên Real-Debrid tiếp tục xử lý.");
    }

    private async Task RemoveJobsAsync(DownloadJob[] jobs)
    {
        if (_closing || _clearing) return;
        jobs = jobs.Where(j => j.CanRemove).ToArray();
        foreach (var job in jobs) job.IsRemoving = true;
        UpdateControls();
        await StopJobsAsync(jobs);
        foreach (var job in jobs) Jobs.Remove(job);
        Save(); UpdateControls(); RefreshSummary();
        SetStatus("Đã xóa {0} dòng khỏi danh sách. File đã tải, file tạm và torrent trên Real-Debrid được giữ nguyên.", jobs.Length);
    }

    private async void ClearQueue(object sender, RoutedEventArgs e)
    {
        if (_closing || _clearing || Jobs.Count == 0) return;
        if (MessageBox.Show(this, Text.Get("Xóa toàn bộ {0} torrent khỏi hàng đợi? App sẽ dừng các tác vụ đang chạy. File đã tải, file tạm và torrent trên Real-Debrid vẫn được giữ.", Jobs.Count),
            Text.Get("Xóa toàn bộ hàng đợi"), MessageBoxButton.YesNo, MessageBoxImage.Question, MessageBoxResult.No) != MessageBoxResult.Yes) return;
        _clearing = true;
        UpdateControls();
        _clearTask = ClearQueueAsync();
        try { await _clearTask; }
        catch (Exception ex) { SetStatus("Không thể xóa toàn bộ hàng đợi: {0}", ex.Message); }
        finally { _clearing = false; _clearTask = null; UpdateControls(); }
    }

    private async Task ClearQueueAsync()
    {
        SetStatus("Đang dừng tác vụ và xóa toàn bộ hàng đợi…");
        _pendingStarts.Clear();
        var run = _runTask;
        var clearing = QueueOperations.ClearAsync(Jobs, _session);
        _runCancellation?.Cancel(); // Also cancel account validation before a session exists.
        await clearing;
        if (run is not null) await run;
        Save(); RefreshSummary();
        SetStatus("Đã xóa toàn bộ hàng đợi. File đã tải, file tạm và torrent trên Real-Debrid được giữ nguyên.");
    }

    private async void RetrySelected(object sender, RoutedEventArgs e)
        => await StartJobsAsync(QueueGrid.SelectedItems.Cast<DownloadJob>().Where(j => j.CanRetry).ToArray());
    private async void ResumeSelected(object sender, RoutedEventArgs e)
        => await StartJobsAsync(QueueGrid.SelectedItems.Cast<DownloadJob>().Where(j => j.CanResume).ToArray());
    private async void StopSelected(object sender, RoutedEventArgs e)
        => await StopJobsAsync(QueueGrid.SelectedItems.Cast<DownloadJob>().Where(j => j.CanStop).ToArray());
    private async void RemoveSelected(object sender, RoutedEventArgs e)
        => await RemoveJobsAsync(QueueGrid.SelectedItems.Cast<DownloadJob>().ToArray());
    private async void RetryRow(object sender, RoutedEventArgs e)
    {
        e.Handled = true;
        if (sender is FrameworkElement { DataContext: DownloadJob job } && job.CanRetry) await StartJobsAsync([job]);
    }
    private async void ResumeRow(object sender, RoutedEventArgs e)
    {
        e.Handled = true;
        if (sender is FrameworkElement { DataContext: DownloadJob job } && job.CanResume) await StartJobsAsync([job]);
    }
    private async void StopRow(object sender, RoutedEventArgs e)
    {
        e.Handled = true;
        if (sender is FrameworkElement { DataContext: DownloadJob job } && job.CanStop) await StopJobsAsync([job]);
    }
    private async void RemoveRow(object sender, RoutedEventArgs e)
    {
        e.Handled = true;
        if (sender is FrameworkElement { DataContext: DownloadJob job }) await RemoveJobsAsync([job]);
    }
    private void QueueSelectionChanged(object sender, SelectionChangedEventArgs e) { if (!_initializing) UpdateControls(); }
    private void UpdateControls()
    {
        var idle = !Running && !_checking && !_closing && !_clearing;
        var actionsAvailable = !_checking && !_closing && !_clearing && (!Running || _session is not null && _runCancellation?.IsCancellationRequested == false);
        StartButton.IsEnabled = actionsAvailable && Jobs.Any(j => j.CanResume);
        PauseButton.IsEnabled = !_clearing && !_closing && Running && _runCancellation?.IsCancellationRequested == false;
        StopAllButton.IsEnabled = !_closing && !_clearing && Jobs.Any(j => j.CanStop);
        ClearQueueButton.IsEnabled = !_closing && !_clearing && Jobs.Count > 0;
        QueueGrid.IsEnabled = !_clearing && !_closing;
        AccountPanel.IsEnabled = idle; FolderBox.IsEnabled = idle; BrowseButton.IsEnabled = idle; ConcurrencyBox.IsEnabled = idle;
        FlattenFilesBox.IsEnabled = idle;
        AddButton.IsEnabled = idle; AddFilesButton.IsEnabled = idle; SourcesBox.IsEnabled = idle; PasteButton.IsEnabled = idle;
        RemoveButton.IsEnabled = !_closing && !_clearing && QueueGrid.SelectedItems.Cast<DownloadJob>().Any(j => j.CanRemove);
        RetryButton.IsEnabled = actionsAvailable && QueueGrid.SelectedItems.Cast<DownloadJob>().Any(j => j.CanRetry);
        ResumeSelectedButton.IsEnabled = actionsAvailable && QueueGrid.SelectedItems.Cast<DownloadJob>().Any(j => j.CanResume);
        StopSelectedButton.IsEnabled = !_closing && !_clearing && QueueGrid.SelectedItems.Cast<DownloadJob>().Any(j => j.CanStop);
    }
    private void RefreshSummary()
    {
        QueueSummary.Text = Text.Get("{0} torrent · {1} xong · {2} đang xử lý · {3} dừng · {4} lỗi", Jobs.Count, Jobs.Count(j => j.State == JobState.Completed), Jobs.Count(j => j.IsActive), Jobs.Count(j => j.State is JobState.Paused or JobState.Stopped), Jobs.Count(j => j.State == JobState.Failed));
        EmptyState.Visibility = Jobs.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
    }
    private void SaveToken()
    {
        _settings.ProtectedToken = RememberToken.IsChecked == true && TokenBox.Password.Trim().Length > 0
            ? Convert.ToBase64String(ProtectedData.Protect(Encoding.UTF8.GetBytes(TokenBox.Password.Trim()), null, DataProtectionScope.CurrentUser)) : null;
    }
    private void Save()
    {
        if (_initializing) return;
        try
        {
            _settings.OutputDirectory = FolderBox.Text.Trim();
            _settings.FlattenFiles = FlattenFilesBox.IsChecked == true;
            _settings.Concurrency = ConcurrencyBox.SelectedIndex + 1;
            _settings.Jobs = Jobs.ToList();
            _store.Save(_settings);
            _storageErrorShown = false;
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            SetStatus("Không lưu được hàng đợi: {0}", ex.Message);
            if (!_storageErrorShown) { _storageErrorShown = true; MessageBox.Show(this, StatusText.Text, Text.Get("Lỗi lưu hàng đợi"), MessageBoxButton.OK, MessageBoxImage.Warning); }
        }
    }

    private async void OnClosing(object? sender, CancelEventArgs e)
    {
        if (_readyToClose) { DisposeClients(); return; }
        if (Running || _clearing)
        {
            e.Cancel = true;
            if (_closing) return;
            _closing = true; _runCancellation?.Cancel(); UpdateControls();
            if (_runTask is not null) await _runTask;
            if (_clearTask is not null) await _clearTask;
            Save(); _readyToClose = true; Close();
            return;
        }
        Save(); DisposeClients();
    }
    private void DisposeClients() { _timer.Stop(); _apiHttp.Dispose(); _transferHttp.Dispose(); }
}
