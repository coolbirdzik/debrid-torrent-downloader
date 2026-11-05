using System.ComponentModel;
using System.Globalization;
using System.Runtime.CompilerServices;
using RdBatch.Core;

namespace RdBatch.App;

/// <summary>One torrent from the account list; the account is the one the list was loaded for.</summary>
public sealed class RemoteTorrentRow(RdTorrent torrent, string account) : INotifyPropertyChanged
{
    public RdTorrent Torrent { get; } = torrent;
    public string Id => Torrent.Id;
    public string Name => string.IsNullOrWhiteSpace(Torrent.Filename) ? Torrent.Id : Torrent.Filename;
    public string Account { get; } = account;
    public string SizeLabel => Torrent.Bytes > 0 ? DownloadJob.FormatBytes(Torrent.Bytes) : "—";
    public string AddedLabel => Torrent.Added?.ToLocalTime().ToString("yyyy-MM-dd HH:mm", CultureInfo.InvariantCulture) ?? "—";
    // success / error / active / idle, used only to color the status pill.
    public string Tone => _failed ? "error" : _status is not null ? "idle" : Torrent.Status switch
    {
        "downloaded" => "success",
        "magnet_error" or "error" or "virus" or "dead" => "error",
        "downloading" or "compressing" or "uploading" or "magnet_conversion" or "queued" => "active",
        _ => "idle"
    };
    private bool _queued;
    public bool IsQueued { get => _queued; set { if (_queued == value) return; _queued = value; Notify(); } }
    private bool _selected;
    public bool IsSelected { get => _selected; set { var next = value && CanDelete; if (_selected == next) return; _selected = next; Notify(); } }
    private bool _deleted;
    public bool IsDeleted
    {
        get => _deleted;
        set { _deleted = value; if (value) IsSelected = false; Notify(); Notify(nameof(CanDelete)); }
    }
    public bool CanDelete => !IsDeleted;
    // Remote status until a delete attempt reports its own result.
    private LocalizedMessage? _status;
    public string Status => (_status ?? RemoteStatus()).Render();
    private bool _failed;
    public void SetStatus(string key, params object?[] arguments) => Report(false, key, arguments);
    public void Fail(string key, params object?[] arguments) => Report(true, key, arguments);
    private void Report(bool failed, string key, object?[] arguments)
    { _failed = failed; _status = LocalizedMessage.Create(key, arguments); Notify(nameof(Status)); Notify(nameof(Tone)); }
    public void Refresh() { Notify(nameof(Name)); Notify(nameof(Status)); }
    private LocalizedMessage RemoteStatus()
    {
        var progress = Math.Round(Torrent.Progress);
        return Torrent.Status switch
        {
            "downloaded" => LocalizedMessage.Create("Đã tải xong"),
            "downloading" => LocalizedMessage.Create("Đang tải {0}%", progress),
            "queued" => LocalizedMessage.Create("Đang xếp hàng"),
            "magnet_conversion" => LocalizedMessage.Create("Đang đọc magnet"),
            "waiting_files_selection" => LocalizedMessage.Create("Chờ chọn file"),
            "compressing" => LocalizedMessage.Create("Đang nén {0}%", progress),
            "uploading" => LocalizedMessage.Create("Đang tải lên {0}%", progress),
            "magnet_error" => LocalizedMessage.Create("Lỗi magnet"),
            "error" => LocalizedMessage.Create("Lỗi trên Real-Debrid"),
            "virus" => LocalizedMessage.Create("Có virus"),
            "dead" => LocalizedMessage.Create("Torrent chết"),
            var other => LocalizedMessage.Create(other ?? "")
        };
    }
    public event PropertyChangedEventHandler? PropertyChanged;
    private void Notify([CallerMemberName] string? name = null) => PropertyChanged?.Invoke(this, new(name));
}
