using System.Diagnostics;
using System.Net;
using System.Net.Http.Headers;
using System.Text.Json;

namespace RdBatch.Core;

public sealed record TransferProgress(long Bytes, long Total, double BytesPerSecond);

public sealed class FileDownloader(HttpClient http)
{
    public async Task DownloadAsync(string url, string destination, long expectedBytes, Action<TransferProgress> progress, CancellationToken ct)
    {
        if (!Uri.TryCreate(url, UriKind.Absolute, out var uri) || uri.Scheme is not ("http" or "https"))
            throw new InvalidDataException(Text.Get("Link tải không hợp lệ."));
        if (File.Exists(destination)) throw new IOException(Text.Get("File đích đã tồn tại; app không ghi đè: {0}", Path.GetFileName(destination)));
        Directory.CreateDirectory(Path.GetDirectoryName(destination)!);
        var partial = destination + ".part";
        var metadata = partial + ".json";
        SafePaths.EnsureNoReparsePoints(Path.GetDirectoryName(destination)!, partial);
        SafePaths.EnsureNoReparsePoints(Path.GetDirectoryName(destination)!, metadata);
        PartialMetadata? saved = null;
        try { if (File.Exists(metadata)) saved = JsonSerializer.Deserialize<PartialMetadata>(await File.ReadAllTextAsync(metadata, ct)); }
        catch (JsonException) { }
        var offset = File.Exists(partial) && saved is not null && saved.ExpectedBytes == expectedBytes ? new FileInfo(partial).Length : 0;
        if (expectedBytes > 0 && offset > expectedBytes) offset = 0;

        using var request = new HttpRequestMessage(HttpMethod.Get, uri);
        request.Headers.AcceptEncoding.Add(new StringWithQualityHeaderValue("identity"));
        if (offset > 0)
        {
            request.Headers.Range = new RangeHeaderValue(offset, null);
            if (!string.IsNullOrEmpty(saved?.Validator)) request.Headers.TryAddWithoutValidation("If-Range", saved.Validator);
        }
        using var headerTimeout = CancellationTokenSource.CreateLinkedTokenSource(ct);
        headerTimeout.CancelAfter(TimeSpan.FromSeconds(90));
        using var response = await http.SendAsync(request, HttpCompletionOption.ResponseHeadersRead, headerTimeout.Token);
        if (response.StatusCode == HttpStatusCode.RequestedRangeNotSatisfiable && offset > 0 &&
            response.Content.Headers.ContentRange?.Length == offset && (expectedBytes <= 0 || offset == expectedBytes))
        {
            File.Move(partial, destination);
            File.Delete(metadata);
            progress(new(offset, offset, 0));
            return;
        }
        response.EnsureSuccessStatusCode();
        long total;
        if (response.StatusCode == HttpStatusCode.PartialContent)
        {
            var range = response.Content.Headers.ContentRange;
            if (range?.From != offset || range.Length is null || range.To != range.Length - 1)
                throw new InvalidDataException(Text.Get("Máy chủ trả về khoảng dữ liệu không hợp lệ; chưa ghép file."));
            total = range.Length.Value;
        }
        else
        {
            offset = 0; // Server ignored Range or If-Range detected a changed file: start cleanly.
            total = response.Content.Headers.ContentLength ?? expectedBytes;
        }
        if (expectedBytes > 0 && total > 0 && total != expectedBytes)
            throw new InvalidDataException(Text.Get("Kích thước link tải khác thông tin Real-Debrid; chưa lưu file hoàn tất."));
        if (total <= 0) total = expectedBytes;
        var validator = response.Headers.ETag is { IsWeak: false } tag ? tag.ToString() : response.Content.Headers.LastModified?.ToString("R");
        await File.WriteAllTextAsync(metadata, JsonSerializer.Serialize(new PartialMetadata(expectedBytes, validator)), ct);
        await using var input = await response.Content.ReadAsStreamAsync(ct);
        await using (var output = new FileStream(partial, offset > 0 ? FileMode.Open : FileMode.Create, FileAccess.Write, FileShare.Read, 128 * 1024, true))
        {
            output.Position = offset;
            var buffer = new byte[128 * 1024];
            var downloaded = offset;
            var watch = Stopwatch.StartNew();
            var lastReport = TimeSpan.Zero;
            using var readTimeout = CancellationTokenSource.CreateLinkedTokenSource(ct);
            progress(new(downloaded, total, 0));
            while (true)
            {
                readTimeout.CancelAfter(TimeSpan.FromSeconds(90));
                var read = await input.ReadAsync(buffer, readTimeout.Token);
                if (read == 0) break;
                await output.WriteAsync(buffer.AsMemory(0, read), ct);
                downloaded += read;
                if (watch.Elapsed - lastReport > TimeSpan.FromMilliseconds(250))
                {
                    progress(new(downloaded, total, (downloaded - offset) / Math.Max(.001, watch.Elapsed.TotalSeconds)));
                    lastReport = watch.Elapsed;
                }
            }
            await output.FlushAsync(ct);
            if (total > 0 && downloaded != total) throw new IOException(Text.Get("Kết nối ngắt trước khi tải đủ file. Có thể bấm Thử lại để tiếp tục."));
            ct.ThrowIfCancellationRequested();
            progress(new(downloaded, downloaded, 0));
        }
        File.Move(partial, destination); // Never overwrite an existing completed file.
        File.Delete(metadata);
    }
    private sealed record PartialMetadata(long ExpectedBytes, string? Validator);
}
