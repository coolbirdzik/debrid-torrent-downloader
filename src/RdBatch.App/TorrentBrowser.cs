using System.ComponentModel;
using System.IO;
using System.Runtime.CompilerServices;
using RdBatch.Core;

namespace RdBatch.App;

public sealed class TorrentFolder : INotifyPropertyChanged
{
    public string Name { get; init; } = "";
    public TorrentDraft? Torrent { get; init; }
    public string Path { get; init; } = "";
    public bool CanRemove => Torrent is not null;
    public List<TorrentFolder> Children { get; } = [];
    private bool _isSelected;
    public bool IsSelected { get => _isSelected; set { _isSelected = value; PropertyChanged?.Invoke(this, new(nameof(IsSelected))); } }
    public bool IsExpanded { get; set; }
    public event PropertyChangedEventHandler? PropertyChanged;
}

public sealed record BrowserRemoval((TorrentDraft Torrent, int Id)[] Files, TorrentDraft[] HiddenTorrents);

public sealed class BrowserFile(TorrentDraft torrent, TorrentFile file, bool queued) : INotifyPropertyChanged
{
    public TorrentDraft Torrent { get; } = torrent;
    public TorrentFile File { get; } = file;
    public string Name => System.IO.Path.GetFileName(File.Path.Replace('\\', '/'));
    public string Location => Torrent.Name + " / " + DisplayPath;
    private string DisplayPath
    {
        get
        {
            var path = File.Path.Replace('\\', '/').Trim('/');
            return path.StartsWith(Torrent.Name + "/", StringComparison.OrdinalIgnoreCase) ? path[(Torrent.Name.Length + 1)..] : path;
        }
    }
    public string SizeLabel => DownloadJob.FormatBytes(File.Bytes);
    public bool IsVideo => TorrentFileSearch.IsVideo(File.Path);
    public string Icon => IsVideo ? "\uE714" : "\uE8A5";
    public bool CanQueue => !queued;
    public string StatusLabel => Text.Get(queued ? "Đã có trong hàng đợi" : IsVideo ? "Video" : "File");
    private bool _isSelected;
    public bool IsSelected
    {
        get => _isSelected;
        set { if (_isSelected == value) return; _isSelected = value; Notify(); }
    }
    public event PropertyChangedEventHandler? PropertyChanged;
    private void Notify([CallerMemberName] string? name = null) => PropertyChanged?.Invoke(this, new(name));
}
