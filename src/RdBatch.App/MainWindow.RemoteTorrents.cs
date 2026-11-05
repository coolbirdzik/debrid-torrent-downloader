using System.Collections.ObjectModel;
using System.Net;
using System.Windows;
using System.Windows.Controls;
using RdBatch.Core;

namespace RdBatch.App;

public partial class MainWindow
{
    private const int RemoteTab = 2;
    public ObservableCollection<RemoteTorrentRow> RemoteTorrents { get; } = [];
    private RemoteTorrentRow[]? _remoteReviewTargets;
    private CancellationTokenSource? _remoteDeleteCancellation;
    private CancellationTokenSource? _remoteLoadCancellation;
    private bool _selectingRemoteTorrents;
    private bool _remoteLoaded;
    private string? _remoteAccount; // Account the visible list was loaded for; deletion must match it.
    private LocalizedMessage _remoteEmpty = LocalizedMessage.Create("Dán API token để xem torrent trong tài khoản Real-Debrid.");
    private bool RemoteLoading => _remoteLoadCancellation is not null;

    // Persist every remote ID the app creates. The tab itself lists the account, not this registry.
    private void TrackRemoteTorrents()
    {
        foreach (var draft in Library) Track(draft.TorrentId, draft.SourceKey, draft.Name, draft.RemoteAccount);
        foreach (var job in Jobs) Track(job.TorrentId, job.SourceKey, job.Name, job.RemoteAccount);

        void Track(string? id, string sourceKey, string name, string? account)
        {
            if (string.IsNullOrWhiteSpace(id)) return;
            var entry = _settings.RemoteTorrents.FirstOrDefault(item => item.Id == id);
            if (entry is null) _settings.RemoteTorrents.Add(new() { Id = id, SourceKey = sourceKey, Name = name, Account = account });
            else { entry.Name = name; entry.Account ??= account; }
        }
    }

    private void WorkspaceTabChanged(object sender, SelectionChangedEventArgs e)
    {
        // Selection events from grids and combo boxes inside the tabs bubble up here too.
        if (_initializing || !ReferenceEquals(e.OriginalSource, WorkspaceTabs)) return;
        if (WorkspaceTabs.SelectedIndex == RemoteTab && !_remoteLoaded && !RemoteLoading) _ = LoadRemoteTorrentsAsync();
    }

    private void RefreshRemoteTorrents(object sender, RoutedEventArgs e) => _ = LoadRemoteTorrentsAsync();

    private async Task LoadRemoteTorrentsAsync()
    {
        if (_closing || _clearing || _remoteReviewTargets is not null) return;
        var token = TokenBox.Password.Trim();
        if (token.Length == 0)
        {
            ResetRemoteList(LocalizedMessage.Create("Dán API token để xem torrent trong tài khoản Real-Debrid."));
            return;
        }
        _remoteLoadCancellation?.Cancel(); // A newer load wins; the older one must not overwrite it.
        using var cancellation = new CancellationTokenSource();
        _remoteLoadCancellation = cancellation;
        UpdateRemoteControls();
        try
        {
            var api = new RealDebridClient(_apiHttp, token);
            var user = await api.GetUserAsync(cancellation.Token);
            var torrents = await api.ListTorrentsAsync(cancellation.Token);
            if (cancellation.IsCancellationRequested || _closing) return;
            _verifiedToken = token; _user = user; SetAccount("{0} · {1}", user.Username, user.Type);
            SaveToken(); Save(); // A successful load validates the token just like Connect.
            ResetRemoteList(LocalizedMessage.Create("Tài khoản Real-Debrid chưa có torrent nào."));
            _remoteAccount = user.Username;
            foreach (var torrent in torrents)
            {
                var row = new RemoteTorrentRow(torrent, user.Username);
                row.PropertyChanged += (_, args) =>
                {
                    if (!_selectingRemoteTorrents && args.PropertyName == nameof(RemoteTorrentRow.IsSelected))
                    { _remoteSelection.Anchor = row; UpdateRemoteControls(); }
                };
                RemoteTorrents.Add(row);
            }
            _remoteLoaded = true;
        }
        catch (OperationCanceledException) when (cancellation.IsCancellationRequested) { }
        catch (Exception ex) when (!_closing)
        {
            ResetRemoteList(LocalizedMessage.Create(DownloadEngine.FriendlyErrorKey(ex)));
            SetStatus(DownloadEngine.FriendlyErrorKey(ex));
        }
        catch (Exception) { } // Closing disposes the HTTP client under an in-flight load.
        finally
        {
            if (ReferenceEquals(_remoteLoadCancellation, cancellation)) _remoteLoadCancellation = null;
            if (!_closing) UpdateRemoteControls();
        }
    }

    private void ResetRemoteList(LocalizedMessage empty)
    {
        _remoteReviewTargets = null;
        RemoteGrid.ItemsSource = RemoteTorrents;
        RemoteTorrents.Clear();
        _remoteSelection.Anchor = null;
        _remoteAccount = null; _remoteLoaded = false;
        _remoteEmpty = empty;
        UpdateRemoteControls();
    }

    // A different token may belong to another account: drop the list instead of letting it go stale.
    private void RemoteTokenChanged()
    {
        _remoteLoadCancellation?.Cancel();
        if (_remoteAccount is not null || RemoteTorrents.Count > 0 || _remoteLoaded)
            ResetRemoteList(LocalizedMessage.Create("Token đã đổi. Bấm Tải lại để xem torrent của tài khoản mới."));
    }

    private void ClearVirtualLibrary(object sender, RoutedEventArgs e)
    {
        if (_closing || _clearing || _checking || _inspectTask is not null) return;
        TrackRemoteTorrents(); // Keep preview IDs recorded after clearing the batch.
        Library.Clear(); _removedFiles.Clear(); _currentFolder = null;
        SourcesBox.Clear(); SearchFilesBox.Clear(); VideoOnlyBox.IsChecked = false;
        Save(); RebuildFolders(); UpdateControls();
        SetStatus("Đã xóa thư viện ảo để bắt đầu batch mới. Hàng đợi, file trên máy và danh sách torrent Real-Debrid được giữ.");
    }

    private void ReviewRemoteDeletion(object sender, RoutedEventArgs e)
    {
        if (_closing || _clearing || _checking || RemoteLoading || _inspectTask is not null || Running && _session is null) return;
        var targets = RemoteTorrents.Where(row => row.IsSelected && row.CanDelete).ToArray();
        if (targets.Length == 0) return;
        _remoteReviewTargets = targets;
        RemoteGrid.ItemsSource = targets;
        UpdateRemoteControls();
    }

    private void SelectRemoteTorrents(object sender, RoutedEventArgs e)
    {
        if (_closing || _clearing || _remoteReviewTargets is not null) return;
        var select = RemoteTorrents.Any(row => row.CanDelete && !row.IsSelected);
        _selectingRemoteTorrents = true;
        foreach (var row in RemoteTorrents) row.IsSelected = select;
        _remoteSelection.Anchor = null;
        _selectingRemoteTorrents = false;
        UpdateRemoteControls();
    }

    private void CancelRemoteDeletion(object sender, RoutedEventArgs e)
    {
        if (_remoteDeleteCancellation is not null) { _remoteDeleteCancellation.Cancel(); return; }
        _remoteReviewTargets = null;
        RemoteGrid.ItemsSource = RemoteTorrents;
        UpdateRemoteControls();
    }

    private async void ConfirmRemoteDeletion(object sender, RoutedEventArgs e)
    {
        if (_closing || _clearing || _checking || _inspectTask is not null || _remoteReviewTargets is null || Running && _session is null) return;
        var token = TokenBox.Password.Trim();
        if (token.Length == 0) { SetStatus("Dán API token trước khi kiểm tra."); return; }
        var targets = _remoteReviewTargets.ToArray(); // Exact reviewed IDs; never expand deletion from source/hash.
        _clearing = true;
        using var cancellation = new CancellationTokenSource();
        _remoteDeleteCancellation = cancellation;
        UpdateControls();
        _clearTask = DeleteRemoteTorrentsAsync(targets, token, cancellation.Token);
        try { await _clearTask; }
        finally
        {
            _clearTask = null; _clearing = false; _remoteDeleteCancellation = null; _remoteReviewTargets = null;
            _remoteSelection.Anchor = null;
            foreach (var row in targets.Where(row => row.IsDeleted)) RemoteTorrents.Remove(row);
            RemoteGrid.ItemsSource = RemoteTorrents;
            Save(); RefreshBrowser(); RefreshSummary(); UpdateControls();
        }
    }

    private async Task DeleteRemoteTorrentsAsync(RemoteTorrentRow[] targets, string token, CancellationToken ct)
    {
        var deleted = 0;
        var failures = 0;
        var errors = new List<string>();
        try
        {
            var api = new RealDebridClient(_apiHttp, token);
            var user = await api.GetUserAsync(ct);
            _verifiedToken = token; _user = user; SetAccount("{0} · {1}", user.Username, user.Type);
            // The list was loaded for one account; never send a DELETE with another account's token.
            if (_remoteAccount is null || !_remoteAccount.Equals(user.Username, StringComparison.OrdinalIgnoreCase))
                throw new InvalidOperationException(Text.Get("Danh sách thuộc tài khoản {0}; token hiện tại là {1}. Hãy tải lại danh sách.", _remoteAccount ?? "?", user.Username));
            foreach (var row in targets)
            {
                ct.ThrowIfCancellationRequested();
                if (!row.CanDelete) continue;
                try
                {
                    row.SetStatus("Đang xóa trên Real-Debrid…");
                    var linkedJobs = Jobs.Where(job => job.TorrentId == row.Id).ToArray();
                    foreach (var job in linkedJobs) _pendingStarts.Remove(job);
                    if (_session is { } session) await Task.WhenAll(linkedJobs.Select(session.StopAsync));
                    else foreach (var job in linkedJobs.Where(job => job.State is not (JobState.Completed or JobState.Failed)))
                        { job.State = JobState.Stopped; job.SetDetail("Đã dừng · bấm Resume để tải tiếp"); }
                    ct.ThrowIfCancellationRequested();
                    try { await api.DeleteTorrentAsync(row.Id, ct); }
                    catch (RdApiException ex) when (ex.Status == HttpStatusCode.NotFound) { /* Already gone on the verified account. */ }
                    foreach (var job in linkedJobs) Jobs.Remove(job);
                    foreach (var draft in Library.Where(draft => draft.TorrentId == row.Id)) { draft.TorrentId = null; draft.RemoteAccount = null; }
                    _settings.RemoteTorrents.RemoveAll(item => item.Id == row.Id);
                    row.SetStatus("Đã xóa trên Real-Debrid"); row.IsDeleted = true;
                    deleted++; Save();
                }
                catch (OperationCanceledException) when (ct.IsCancellationRequested)
                { row.SetStatus("Đã hủy; kiểm tra lại trước khi thử tiếp."); throw; }
                catch (Exception ex)
                {
                    failures++; errors.Add(row.Name + " [" + row.Id + "]: " + DownloadEngine.FriendlyError(ex));
                    row.Fail(DownloadEngine.FriendlyErrorKey(ex));
                }
            }
        }
        catch (OperationCanceledException) when (ct.IsCancellationRequested) { }
        catch (Exception ex)
        {
            errors.Add(DownloadEngine.FriendlyError(ex));
            foreach (var row in targets.Where(row => !row.IsDeleted)) row.Fail(DownloadEngine.FriendlyErrorKey(ex));
            failures = targets.Count(row => !row.IsDeleted);
        }
        SetStatus(ct.IsCancellationRequested ? "Đã hủy xóa từ xa · {0} torrent đã xóa · {1} lỗi." : "Real-Debrid: đã xóa {0} torrent · {1} lỗi. File trên máy được giữ.", deleted, failures);
        StatusText.ToolTip = errors.Count == 0 ? null : string.Join(Environment.NewLine, errors);
    }

    private void UpdateRemoteControls()
    {
        if (_initializing) return;
        var reviewing = _remoteReviewTargets is not null;
        var loading = RemoteLoading;
        var available = !_closing && !_clearing && !_checking && _inspectCancellation is null && (!Running || _session is not null);
        // Keep the grid enabled so reviewed rows stay readable; only block pointer input while frozen.
        RemoteGrid.IsHitTestVisible = available && !reviewing && !loading;
        var queued = Jobs.Select(job => job.TorrentId).OfType<string>().ToHashSet();
        foreach (var row in RemoteTorrents) row.IsQueued = queued.Contains(row.Id);
        RemoteRefreshButton.IsEnabled = !_closing && !_clearing && !reviewing && !loading;
        RemoteLoadProgress.Visibility = loading ? Visibility.Visible : Visibility.Collapsed;
        RemoteSelectAllButton.Visibility = reviewing ? Visibility.Collapsed : Visibility.Visible;
        RemoteSelectAllButton.IsEnabled = available && !loading && RemoteTorrents.Any(row => row.CanDelete);
        RemoteReviewButton.Visibility = reviewing ? Visibility.Collapsed : Visibility.Visible;
        RemoteReviewButton.IsEnabled = available && !loading && RemoteTorrents.Any(row => row.IsSelected && row.CanDelete);
        RemoteConfirmButton.Visibility = reviewing ? Visibility.Visible : Visibility.Collapsed;
        RemoteConfirmButton.IsEnabled = available;
        RemoteCancelButton.Visibility = reviewing ? Visibility.Visible : Visibility.Collapsed;
        RemoteCancelButton.IsEnabled = !_closing && (_remoteDeleteCancellation is not null || available);
        RemoteReviewNotice.Visibility = reviewing ? Visibility.Visible : Visibility.Collapsed;
        RemoteReviewNotice.Text = Text.Get("Xóa {0} torrent có ID trong bảng dưới khỏi tài khoản Real-Debrid. Các tác vụ gắn với ID này sẽ dừng và bỏ khỏi queue; giữ file trên máy. Thao tác xóa từ xa không thể Hoàn tác.", _remoteReviewTargets?.Length ?? 0);
        RemoteSummary.Text = _remoteAccount is null ? Text.Get("Chưa tải danh sách")
            : Text.Get("{0} torrent trên {1} · {2} đã chọn", RemoteTorrents.Count(row => row.CanDelete), _remoteAccount, RemoteTorrents.Count(row => row.IsSelected && row.CanDelete));
        RemoteEmpty.Visibility = !reviewing && RemoteTorrents.Count == 0 ? Visibility.Visible : Visibility.Collapsed;
        RemoteEmptyText.Text = loading ? Text.Get("Đang tải danh sách từ Real-Debrid…") : _remoteEmpty.Render();
        NewBatchButton.IsEnabled = !_closing && !_clearing && !_checking && _inspectCancellation is null && (Library.Count > 0 || !string.IsNullOrWhiteSpace(SourcesBox.Text));
    }
}
