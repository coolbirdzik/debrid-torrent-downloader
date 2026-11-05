using System.ComponentModel;
using System.Runtime.CompilerServices;
using System.Text.Json.Serialization;

namespace RdBatch.Core;

public enum JobState { Queued, Adding, Waiting, Downloading, Paused, Completed, Failed, Stopped }

public sealed class DownloadJob : INotifyPropertyChanged
{
    public string Id { get; set; } = Guid.NewGuid().ToString("N");
    public string Source { get; set; } = "";
    public string SourceKey { get; set; } = "";
    public string? TorrentId { get; set; }
    public string? RemoteAccount { get; set; }
    // Null preserves the all-files behavior of queues saved by earlier versions.
    public List<int>? SelectedFileIds { get; set; }
    public string? OutputDirectory { get; set; }
    public bool? FlattenFiles { get; set; }
    public List<DownloadFile> Files { get; set; } = [];
    private string _name = "Torrent mới";
    public string Name { get => _name; set { _name = value; Notify(); } }
    private JobState _state;
    public JobState State { get => _state; set { _state = value; Notify(); NotifyActions(); } }
    private bool _isActive;
    [JsonIgnore] public bool IsActive { get => _isActive; internal set { _isActive = value; Notify(); NotifyActions(); } }
    private bool _isStopping;
    [JsonIgnore] public bool IsStopping { get => _isStopping; internal set { _isStopping = value; Notify(); NotifyActions(); } }
    private bool _isRemoving;
    [JsonIgnore] public bool IsRemoving { get => _isRemoving; set { _isRemoving = value; Notify(); NotifyActions(); } }
    [JsonIgnore] public bool CanRetry => !IsRemoving && !IsActive && !IsStopping && State == JobState.Failed;
    [JsonIgnore] public bool CanResume => !IsRemoving && !IsActive && !IsStopping && State is JobState.Queued or JobState.Paused or JobState.Stopped;
    [JsonIgnore] public bool CanStop => !IsRemoving && !IsStopping && (IsActive || State is JobState.Queued or JobState.Paused);
    [JsonIgnore] public bool CanRemove => !IsRemoving && !IsStopping;
    private string _detail = "Sẵn sàng";
    public string Detail { get => DetailMessage?.Render() ?? Text.Get(_detail); set { _detail = value; DetailMessage = null; Notify(); } }
    public LocalizedMessage? DetailMessage { get; set; }
    public void SetDetail(string key, params object?[] arguments)
    { DetailMessage = LocalizedMessage.Create(key, arguments); Notify(nameof(Detail)); }
    public void RefreshLanguage() { Notify(nameof(Detail)); Notify(nameof(StateLabel)); }
    private double _remoteProgress;
    public double RemoteProgress { get => _remoteProgress; set { _remoteProgress = value; Notify(); } }
    private double _localProgress;
    public double LocalProgress { get => _localProgress; set { _localProgress = value; Notify(); } }
    private long _totalBytes;
    public long TotalBytes { get => _totalBytes; set { _totalBytes = value; Notify(); Notify(nameof(SizeLabel)); } }
    [JsonIgnore] public string SizeLabel => FormatBytes(TotalBytes);
    [JsonIgnore] public string StateLabel => Text.Get(IsStopping ? "Đang dừng…" : State switch
    {
        JobState.Queued => "Chờ xử lý", JobState.Adding => "Đang thêm", JobState.Waiting => "Real-Debrid",
        JobState.Downloading => "Đang tải về", JobState.Paused => "Tạm dừng", JobState.Stopped => "Đã dừng", JobState.Completed => "Hoàn tất", _ => "Có lỗi"
    });
    public event PropertyChangedEventHandler? PropertyChanged;
    private void Notify([CallerMemberName] string? name = null) => PropertyChanged?.Invoke(this, new(name));
    private void NotifyActions()
    {
        Notify(nameof(StateLabel)); Notify(nameof(CanRetry)); Notify(nameof(CanResume)); Notify(nameof(CanStop)); Notify(nameof(CanRemove));
    }
    public static string FormatBytes(double bytes)
    {
        string[] units = ["B", "KB", "MB", "GB", "TB"];
        var unit = 0;
        while (bytes >= 1024 && unit < units.Length - 1) { bytes /= 1024; unit++; }
        return $"{bytes:0.##} {units[unit]}";
    }
}

public sealed class DownloadFile
{
    public int? TorrentFileId { get; set; }
    public int LinkIndex { get; set; } = -1;
    public string Link { get; set; } = "";
    public string RelativePath { get; set; } = "";
    public long ExpectedBytes { get; set; }
    public bool Completed { get; set; }
}

public sealed class AppSettings
{
    public string Language { get; set; } = "en";
    public string Appearance { get; set; } = "system";
    public string BrowserView { get; set; } = "list";
    public string OutputDirectory { get; set; } = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), "Downloads", "Real-Debrid");
    public bool FlattenFiles { get; set; } = true;
    public int Concurrency { get; set; } = 3;
    public string? ProtectedToken { get; set; }
    public List<DownloadJob> Jobs { get; set; } = [];
    public List<TorrentDraft> Library { get; set; } = [];
    public List<RemoteTorrentReference> RemoteTorrents { get; set; } = [];
}

public sealed class TorrentDraft
{
    public string Source { get; set; } = "";
    public string SourceKey { get; set; } = "";
    public string Name { get; set; } = "";
    public string? TorrentId { get; set; }
    public string? RemoteAccount { get; set; }
    public List<TorrentFile> Files { get; set; } = [];
    public HashSet<int> ExcludedFileIds { get; set; } = [];
    public bool IsHidden { get; set; }
    [JsonIgnore] public IEnumerable<TorrentFile> VisibleFiles => IsHidden ? [] : Files.Where(file => !ExcludedFileIds.Contains(file.Id));
}

public sealed class RemoteTorrentReference
{
    public string Id { get; set; } = "";
    public string SourceKey { get; set; } = "";
    public string Name { get; set; } = "";
    public string? Account { get; set; }
}

public sealed record RdUser(string Username, string Type, int Premium);
public sealed record TorrentFile(int Id, string Path, long Bytes, int Selected);
public sealed record TorrentInfo(string Id, string Filename, string Status, double Progress, long Bytes, TorrentFile[] Files, string[] Links, int? Seeders = null, long? Speed = null);
public sealed record RdTorrent(string Id, string Filename, long Bytes, double Progress, string Status, DateTimeOffset? Added, long? Speed = null, int? Seeders = null);
public sealed record UnrestrictedLink(string Filename, long Filesize, string Download);
