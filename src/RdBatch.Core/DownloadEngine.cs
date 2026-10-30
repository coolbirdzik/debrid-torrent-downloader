using System.Net;

namespace RdBatch.Core;

public sealed class DownloadEngine(RealDebridClient api, HttpClient transfers)
{
    private const int MaxTorrentBytes = 20 * 1024 * 1024;
    private readonly FileDownloader _downloader = new(transfers);
    public event Action<DownloadJob>? Changed;
    internal void Publish(DownloadJob job) => Changed?.Invoke(job);

    public DownloadSession CreateSession(IEnumerable<DownloadJob> jobs, string outputDirectory, int concurrency, bool flattenFiles = true)
        => new(this, jobs, outputDirectory, concurrency, flattenFiles);

    public async Task RunAsync(IEnumerable<DownloadJob> jobs, string outputDirectory, int concurrency, CancellationToken ct, bool flattenFiles = true)
    {
        var batch = jobs.ToArray();
        using var session = CreateSession(batch, outputDirectory, concurrency, flattenFiles);
        await Task.WhenAll(batch.Select(job => session.EnqueueAsync(job, ct)));
    }

    internal async Task ProcessAsync(DownloadJob job, string root, bool flattenFiles, DownloadPathAllocator paths, CancellationToken ct)
    {
        Set(job, JobState.Adding, job.TorrentId is null ? "Đang thêm vào Real-Debrid…" : "Đang tiếp tục torrent đã thêm…");
        if (job.TorrentId is null)
        {
            job.TorrentId = job.Source.StartsWith("magnet:", StringComparison.OrdinalIgnoreCase)
                ? await api.AddMagnetAsync(job.Source, ct)
                : await api.AddTorrentAsync(await ReadTorrentAsync(job.Source, ct), ct);
            Changed?.Invoke(job); // Persist the ID before any subsequent network call.
        }

        TorrentInfo info;
        var selected = false;
        while (true)
        {
            ct.ThrowIfCancellationRequested();
            info = await api.GetInfoAsync(job.TorrentId, ct);
            job.Name = string.IsNullOrWhiteSpace(info.Filename) ? job.Name : info.Filename;
            job.RemoteProgress = info.Progress;
            job.TotalBytes = info.Bytes;
            if (info.Status is "magnet_error" or "error" or "virus" or "dead")
                throw new InvalidOperationException(Text.Get("Torrent không tải được trên Real-Debrid: {0}", info.Status));
            if (info.Status == "waiting_files_selection" && !selected)
            {
                await api.SelectAllAsync(job.TorrentId, ct);
                selected = true;
                Set(job, JobState.Waiting, "Đã chọn toàn bộ file · chờ Real-Debrid…");
            }
            else if (info.Status == "downloaded")
            {
                if (info.Files.Any(f => f.Selected != 1))
                    throw new InvalidOperationException(Text.Get("Torrent trên tài khoản chỉ chọn một phần file. Hãy thêm lại torrent trên Real-Debrid với toàn bộ file được chọn."));
                break;
            }
            else Set(job, JobState.Waiting, RemoteLabel(info.Status) + " · {0}%", info.Progress.ToString("0"));
            // Many torrents may be waiting. The API client also applies a global rate limit.
            await Task.Delay(TimeSpan.FromSeconds(5), ct);
        }
        if (info.Links.Length == 0) throw new InvalidOperationException(Text.Get("Real-Debrid chưa cung cấp link tải cho torrent này."));
        job.RemoteProgress = 100;
        if (job.Files.Count == 0)
        {
            job.FlattenFiles = flattenFiles;
            job.OutputDirectory = flattenFiles ? Path.GetFullPath(root)
                : Path.Combine(Path.GetFullPath(root), SafePaths.Segment(job.Name) + "_" + job.Id[..8]);
        }
        // Queues from the first release already have paths in a torrent folder. Keep those resumable.
        job.FlattenFiles ??= false;
        if (job.OutputDirectory is null) throw new InvalidDataException(Text.Get("Hàng đợi có file đã lên lịch nhưng thiếu thư mục lưu."));
        Directory.CreateDirectory(job.OutputDirectory);
        Set(job, JobState.Downloading, "Đang chuẩn bị tải toàn bộ file…");

        for (var index = 0; index < info.Links.Length; index++)
        {
            ct.ThrowIfCancellationRequested();
            var link = info.Links[index];
            var file = job.Files.FirstOrDefault(f => f.LinkIndex == index) ?? job.Files.FirstOrDefault(f => f.Link == link);
            // Restricted links may change after resume. Preserve the established file mapping by index.
            if (file is null && job.Files.Count == info.Links.Length) file = job.Files[index];
            if (file is not null) { file.LinkIndex = index; file.Link = link; }
            if (file?.Completed == true)
            {
                var existing = SafePaths.UnderRoot(job.OutputDirectory, file.RelativePath);
                if (File.Exists(existing) && (file.ExpectedBytes <= 0 || new FileInfo(existing).Length == file.ExpectedBytes)) continue;
                file.Completed = false;
                Changed?.Invoke(job);
            }
            for (var attempt = 0; ; attempt++)
            {
                try
                {
                    var resolved = await api.UnrestrictAsync(link, ct);
                    if (file is null)
                    {
                        // RD can return split archives instead of a one-link-per-file mapping.
                        // Keep torrent directories only when every selected file maps to one link.
                        var sourceFiles = info.Files.Where(f => f.Selected == 1).ToArray();
                        var relative = job.FlattenFiles != true && sourceFiles.Length == info.Links.Length &&
                            Path.GetFileName(sourceFiles[index].Path.Replace('\\', '/')).Equals(resolved.Filename, StringComparison.OrdinalIgnoreCase)
                            ? SafePaths.Relative(sourceFiles[index].Path)
                            : SafePaths.Segment(Path.GetFileName(resolved.Filename.Replace('\\', '/')));
                        relative = paths.Allocate(job.OutputDirectory, relative);
                        file = new() { LinkIndex = index, Link = link, RelativePath = relative, ExpectedBytes = resolved.Filesize };
                        job.Files.Add(file);
                        Changed?.Invoke(job);
                    }
                    else if (file.ExpectedBytes > 0 && resolved.Filesize > 0 && file.ExpectedBytes != resolved.Filesize)
                        throw new InvalidDataException(Text.Get("Kích thước file đã thay đổi; không ghép với dữ liệu cũ."));
                    else if (file.ExpectedBytes <= 0) file.ExpectedBytes = resolved.Filesize;
                    var destination = SafePaths.UnderRoot(job.OutputDirectory, file.RelativePath);
                    SafePaths.EnsureNoReparsePoints(job.OutputDirectory, destination);
                    var completedBytes = job.Files.Where(f => f.Completed).Sum(f => f.ExpectedBytes);
                    var expectedTotal = Math.Max(job.TotalBytes, job.Files.Sum(f => f.ExpectedBytes));
                    await _downloader.DownloadAsync(resolved.Download, destination, file.ExpectedBytes, p =>
                    {
                        job.LocalProgress = expectedTotal > 0 ? Math.Min(100, (completedBytes + p.Bytes) * 100.0 / expectedTotal) : index * 100.0 / info.Links.Length;
                        job.Detail = $"{index + 1}/{info.Links.Length} · {Path.GetFileName(file.RelativePath)} · {DownloadJob.FormatBytes(p.Bytes)} / {DownloadJob.FormatBytes(p.Total)} · {DownloadJob.FormatBytes(p.BytesPerSecond)}/s";
                    }, ct);
                    file.Completed = true;
                    Changed?.Invoke(job);
                    break;
                }
                catch (Exception ex) when (!ct.IsCancellationRequested && attempt < 2 &&
                    (ex is HttpRequestException or TaskCanceledException || ex is RdApiException a && (int)a.Status >= 500))
                {
                    job.SetDetail("Kết nối gián đoạn · thử lại {0}/2…", attempt + 1);
                    Changed?.Invoke(job);
                    await Task.Delay(TimeSpan.FromSeconds(3 * (attempt + 1)), ct);
                }
            }
        }
        job.LocalProgress = 100;
        Set(job, JobState.Completed, "Đã lưu {0} file · {1}", info.Links.Length, job.OutputDirectory);
    }

    private async Task<byte[]> ReadTorrentAsync(string source, CancellationToken ct)
    {
        if (!Uri.TryCreate(source, UriKind.Absolute, out var uri) || uri.Scheme is not ("http" or "https"))
        {
            if (new FileInfo(source).Length > MaxTorrentBytes) throw new InvalidDataException(Text.Get("File .torrent lớn hơn 20 MB."));
            return await File.ReadAllBytesAsync(source, ct);
        }
        using var timeout = CancellationTokenSource.CreateLinkedTokenSource(ct);
        timeout.CancelAfter(TimeSpan.FromSeconds(90));
        using var response = await transfers.GetAsync(uri, HttpCompletionOption.ResponseHeadersRead, timeout.Token);
        response.EnsureSuccessStatusCode();
        if (response.Content.Headers.ContentLength > MaxTorrentBytes) throw new InvalidDataException(Text.Get("File .torrent lớn hơn 20 MB."));
        await using var stream = await response.Content.ReadAsStreamAsync(timeout.Token);
        using var buffer = new MemoryStream();
        var bytes = new byte[65536];
        int count;
        while ((count = await stream.ReadAsync(bytes, timeout.Token)) > 0)
        {
            if (buffer.Length + count > MaxTorrentBytes) throw new InvalidDataException(Text.Get("File .torrent lớn hơn 20 MB."));
            await buffer.WriteAsync(bytes.AsMemory(0, count), timeout.Token);
        }
        return buffer.ToArray();
    }

    private void Set(DownloadJob job, JobState state, string detail, params object?[] arguments) { job.State = state; job.SetDetail(detail, arguments); Changed?.Invoke(job); }
    private static string RemoteLabel(string status) => status switch
    {
        "magnet_conversion" => "Đang lấy metadata", "queued" => "Đang xếp hàng", "downloading" => "Real-Debrid đang tải",
        "compressing" => "Đang đóng gói", "uploading" => "Đang chuẩn bị link", "waiting_files_selection" => "Đang chọn file", _ => status
    };
    public static string FriendlyError(Exception ex) => Text.Get(FriendlyErrorKey(ex));
    public static string FriendlyErrorKey(Exception ex) => ex switch
    {
        RdApiException { Status: HttpStatusCode.Unauthorized } => "Token không hợp lệ hoặc hết hạn. Cập nhật token rồi thử lại.",
        RdApiException { Status: HttpStatusCode.NotFound } => "Torrent không còn trên Real-Debrid. Xóa dòng khỏi hàng đợi rồi thêm lại.",
        RdApiException { ErrorCode: 21 or 33 } => "Đã đạt giới hạn torrent đang hoạt động. Chờ torrent khác xong rồi thử lại.",
        RdApiException { ErrorCode: 23 } => "Tài khoản đã hết lưu lượng tải.",
        HttpRequestException => "Lỗi kết nối hoặc link tải không khả dụng. Bấm Thử lại để tiếp tục.",
        OperationCanceledException => "Kết nối quá thời gian chờ. Bấm Thử lại để tiếp tục.",
        _ => ex.Message
    };
}
