using System.Net;
using System.Net.Http.Headers;
using System.Text.Json;

namespace RdBatch.Core;

public sealed class RdApiException(HttpStatusCode status, string message, int? errorCode = null) : Exception(message)
{
    public HttpStatusCode Status { get; } = status;
    public int? ErrorCode { get; } = errorCode;
}

public sealed class RealDebridClient(HttpClient http, string token)
{
    private static readonly JsonSerializerOptions JsonOptions = new() { PropertyNameCaseInsensitive = true };
    private readonly SemaphoreSlim _gate = new(1, 1);
    private DateTimeOffset _nextRequest;
    private static readonly Uri BaseUri = new("https://api.real-debrid.com/rest/1.0/");

    public Task<RdUser> GetUserAsync(CancellationToken ct) => RequestAsync<RdUser>(HttpMethod.Get, "user", null, ct);
    public Task<TorrentInfo> GetInfoAsync(string id, CancellationToken ct) => RequestAsync<TorrentInfo>(HttpMethod.Get, "torrents/info/" + Uri.EscapeDataString(id), null, ct);
    public async Task<string> AddMagnetAsync(string magnet, CancellationToken ct)
        => (await RequestAsync<CreatedTorrent>(HttpMethod.Post, "torrents/addMagnet", () => Form("magnet", magnet), ct)).Id;
    public async Task<string> AddTorrentAsync(byte[] bytes, CancellationToken ct)
        => (await RequestAsync<CreatedTorrent>(HttpMethod.Put, "torrents/addTorrent", () => new ByteArrayContent(bytes), ct)).Id;
    public async Task SelectAllAsync(string id, CancellationToken ct)
        => await RequestAsync<JsonElement>(HttpMethod.Post, "torrents/selectFiles/" + Uri.EscapeDataString(id), () => Form("files", "all"), ct);
    public Task<UnrestrictedLink> UnrestrictAsync(string link, CancellationToken ct)
        => RequestAsync<UnrestrictedLink>(HttpMethod.Post, "unrestrict/link", () => Form("link", link), ct);
    private static FormUrlEncodedContent Form(string key, string value) => new([new(key, value)]);

    private async Task<T> RequestAsync<T>(HttpMethod method, string path, Func<HttpContent>? content, CancellationToken ct)
    {
        for (var attempt = 0; ; attempt++)
        {
            // One shared limiter per run, including account checks. Well below RD's 250 requests/minute.
            await _gate.WaitAsync(ct);
            try
            {
                var delay = _nextRequest - DateTimeOffset.UtcNow;
                if (delay > TimeSpan.Zero) await Task.Delay(delay, ct);
                _nextRequest = DateTimeOffset.UtcNow.AddMilliseconds(400);
            }
            finally { _gate.Release(); }
            using var request = new HttpRequestMessage(method, new Uri(BaseUri, path));
            request.Headers.Authorization = new AuthenticationHeaderValue("Bearer", token);
            request.Content = content?.Invoke();
            using var response = await http.SendAsync(request, ct);
            var body = await response.Content.ReadAsStringAsync(ct);
            // Only retry safe reads and explicit rate-limit responses. Never replay an ambiguous torrent upload.
            if (attempt < 3 && (response.StatusCode == HttpStatusCode.TooManyRequests || (method == HttpMethod.Get && (int)response.StatusCode >= 500)))
            {
                var retry = response.Headers.RetryAfter?.Delta
                    ?? (response.Headers.RetryAfter?.Date - DateTimeOffset.UtcNow)
                    ?? TimeSpan.FromSeconds(5 * Math.Pow(2, attempt));
                await _gate.WaitAsync(ct);
                try { _nextRequest = DateTimeOffset.UtcNow + (retry > TimeSpan.Zero ? retry : TimeSpan.FromSeconds(5)); }
                finally { _gate.Release(); }
                continue;
            }
            if (!response.IsSuccessStatusCode)
            {
                string error = Text.Get("Yêu cầu thất bại");
                int? code = null;
                try
                {
                    using var doc = JsonDocument.Parse(body);
                    if (doc.RootElement.TryGetProperty("error", out var e)) error = e.GetString() ?? error;
                    if (doc.RootElement.TryGetProperty("error_code", out var c)) code = c.GetInt32();
                }
                catch (JsonException) { }
                throw new RdApiException(response.StatusCode, Text.Get("Real-Debrid {0}: {1}{2}", (int)response.StatusCode, error, code.HasValue ? Text.Get(" (mã {0})", code) : ""), code);
            }
            if (string.IsNullOrWhiteSpace(body)) return default!;
            return JsonSerializer.Deserialize<T>(body, JsonOptions) ?? throw new InvalidDataException(Text.Get("Real-Debrid trả về dữ liệu trống."));
        }
    }
    private sealed record CreatedTorrent(string Id);
}
