using System.Net;
using System.Net.Http.Headers;
using System.Text;
using System.Text.Json;
using RdBatch.Core;

var tests = new (string Name, Func<Task> Run)[]
{
    ("Parse batches, deduplicate hex/base32 magnets and reject invalid input", TestParser),
    ("Reject traversal and protect Windows reserved filenames", TestPaths),
    ("Fresh download verifies bytes and never overwrites", TestFreshDownload),
    ("Resume uses Range + If-Range and joins exact bytes", TestResume),
    ("Server ignoring Range restarts without appending", TestIgnoredRange),
    ("Reject wrong Content-Range and truncated content", TestInvalidResponses),
    ("Cancellation preserves partial download", TestCancellation),
    ("Complete batch flow: add, select all, poll, unrestrict, download every file", TestPipeline),
    ("Flat batch avoids collisions across torrents, existing files and partials", TestFlatBatch),
    ("Flat resume keeps assigned names after changing the layout setting", TestFlatResume),
    ("Existing folder queue keeps its layout when flat mode is enabled", TestLegacyLayout),
    ("Flat mode defaults on for old settings and persists both choices", TestLayoutSettings),
    ("Stop one torrent, keep other work running, then resume exact partial bytes", TestStopAndResume),
    ("Retry a failed torrent in the same active session", TestRetryInSession),
    ("Removing a queued torrent prevents network work and preserves files", TestRemoveQueued),
    ("Stop all cancels active and queued work and persists stopped state", TestStopAll),
    ("Row actions follow stopped, failed, completed and deleting states", TestActionStates),
    ("One failed torrent does not stop other jobs", TestFailureIsolation),
    ("Torrent upload sends raw bytes and auth only to API", TestTorrentUpload),
    ("Saved queue restores paused state and preserves remote IDs", TestPersistence),
    ("401 reports invalid token without leaking credentials", TestApiFailure),
    ("English and Vietnamese messages, errors and queue details survive switching and restart", TestLanguages),
    ("Clear a mixed queue waits for writers and preserves completed and partial files", TestClearQueue),
    ("Clear an idle queue and save an empty queue", TestClearIdleQueue)
};
var failed = 0;
foreach (var test in tests)
{
    try { await test.Run(); Console.WriteLine("PASS " + test.Name); }
    catch (Exception ex) { failed++; Console.WriteLine("FAIL " + test.Name + "\n" + ex); }
}
Console.WriteLine($"{tests.Length - failed}/{tests.Length} passed");
return failed == 0 ? 0 : 1;

static void Check(bool condition, string message) { if (!condition) throw new Exception(message); }
static Task TestLanguages()
{
    using var temp = new TempDirectory();
    var store = new SettingsStore(temp.Path);
    Check(store.Load().Language == "vi", "Old/new settings should retain Vietnamese by default");
    try
    {
        Text.SetLanguage("en");
        var job = new DownloadJob { State = JobState.Completed };
        job.SetDetail("Đã lưu {0} file · {1}", 2, "C:\\Downloads\\Tiếng Việt");
        Check(job.StateLabel == "Completed" && job.Detail == "Saved 2 files · C:\\Downloads\\Tiếng Việt", "English state/detail or file path changed");
        Check(Text.Get("Kiểm tra") == "Connect", "English button translation");
        Check(InputParser.Parse("invalid").Errors.Single().StartsWith("Line 1:"), "Parser error not translated");
        Check(DownloadEngine.FriendlyError(new HttpRequestException()).Contains("Click Retry"), "Network error not translated");
        store.Save(new() { Language = "en", Jobs = [job] });
        Check(store.Load().Language == "en", "Language choice not saved");
        Text.SetLanguage("vi");
        var restored = store.Load().Jobs.Single();
        Check(restored.Detail == "Đã lưu 2 file · C:\\Downloads\\Tiếng Việt" && restored.StateLabel == "Hoàn tất", "Restored details lost their message template");
        Text.SetLanguage("en");
        Check(restored.Detail.StartsWith("Saved 2 files"), "Second language change left stale details");
        Text.SetLanguage("invalid");
        Check(Text.Language == "vi", "Unknown language should fall back to Vietnamese");
        return Task.CompletedTask;
    }
    finally { Text.SetLanguage("vi"); }
}

static async Task TestClearQueue()
{
    using var temp = new TempDirectory();
    using var api = CachedTorrentApi(6);
    var blocked = new BlockingReadStream();
    using var files = Client((_, _) =>
    {
        var response = new HttpResponseMessage(HttpStatusCode.OK) { Content = new StreamContent(blocked) };
        response.Content.Headers.ContentLength = 6;
        return Task.FromResult(response);
    });
    var completedPath = Path.Combine(temp.Path, "keep.txt");
    await File.WriteAllTextAsync(completedPath, "keep");
    var active = new DownloadJob { TorrentId = "active" };
    var queued = new DownloadJob { TorrentId = "queued" };
    var completed = new DownloadJob { State = JobState.Completed };
    var failed = new DownloadJob { State = JobState.Failed };
    var jobs = new List<DownloadJob> { active, queued, completed, failed };
    using var session = new DownloadEngine(new(api, "t"), files).CreateSession(jobs, temp.Path, 1);
    var running = session.EnqueueAsync(active);
    await blocked.Blocked.Task.WaitAsync(TimeSpan.FromSeconds(10));
    var waiting = session.EnqueueAsync(queued);
    var stopping = session.StopAsync(active); // Clear must include a job that is already stopping.
    await QueueOperations.ClearAsync(jobs, session).WaitAsync(TimeSpan.FromSeconds(5));
    await Task.WhenAll(running, waiting, stopping);
    Check(jobs.Count == 0 && session.ActiveCount == 0, "Clear left rows or active writers");
    Check(queued.Files.Count == 0, "A queued torrent started downloading while the queue was cleared");
    var partialPath = Path.Combine(temp.Path, active.Files[0].RelativePath) + ".part";
    Check(new FileInfo(partialPath).Length == 3 && File.Exists(partialPath + ".json"), "Clear changed partial data or resume metadata");
    Check(await File.ReadAllTextAsync(completedPath) == "keep", "Clear changed a completed file");
    await session.EnqueueAsync(active);
    Check(!active.IsActive, "Stale Resume resurrected a removed job");
}

static async Task TestClearIdleQueue()
{
    using var temp = new TempDirectory();
    var jobs = new List<DownloadJob> { new() { State = JobState.Queued }, new() { State = JobState.Paused }, new() { State = JobState.Stopped }, new() { State = JobState.Completed }, new() { State = JobState.Failed } };
    await QueueOperations.ClearAsync(jobs);
    await QueueOperations.ClearAsync(jobs); // Empty is safe, too.
    var store = new SettingsStore(temp.Path);
    store.Save(new() { Jobs = jobs });
    Check(store.Load().Jobs.Count == 0, "Empty queue was not persisted");
}
static async Task Throws<T>(Func<Task> action) where T : Exception
{
    try { await action(); } catch (T) { return; }
    throw new Exception("Expected " + typeof(T).Name);
}
static HttpClient Client(Func<HttpRequestMessage, CancellationToken, Task<HttpResponseMessage>> handler) => new(new FakeHandler(handler)) { Timeout = Timeout.InfiniteTimeSpan };
static HttpResponseMessage Json(object value, HttpStatusCode code = HttpStatusCode.OK) => new(code) { Content = new StringContent(JsonSerializer.Serialize(value), Encoding.UTF8, "application/json") };
static HttpResponseMessage Bytes(byte[] value, HttpStatusCode code = HttpStatusCode.OK) => new(code) { Content = new ByteArrayContent(value) };

static Task TestParser()
{
    var parsed = InputParser.Parse("magnet:?xt=urn:btih:" + new string('0', 40) + "&dn=First+Torrent\n" +
        "magnet:?xt=urn:btih:" + new string('A', 32) + "\n" +
        "magnet:?xt=urn:btih:" + new string('1', 40) + " magnet:?xt=urn:btih:" + new string('2', 40) + "\ninvalid\nhttps://example.test/download?id=123");
    Check(parsed.Sources.Count == 4 && parsed.Errors.Count == 1, "Wrong parse or dedup count");
    Check(parsed.Sources[0].Name == "First Torrent", "Magnet display name");
    return Task.CompletedTask;
}
static Task TestPaths()
{
    Check(SafePaths.Segment("CON.txt") == "_CON.txt", "Reserved filename");
    Check(SafePaths.Segment("a:b?.txt") == "a_b_.txt", "Invalid characters");
    try { SafePaths.Relative("/good/../../escape"); throw new Exception("Traversal accepted"); } catch (InvalidDataException) { }
    using var temp = new TempDirectory();
    try { SafePaths.UnderRoot(temp.Path, "../escape"); throw new Exception("Escaped root"); } catch (InvalidDataException) { }
    return Task.CompletedTask;
}
static async Task TestFreshDownload()
{
    using var temp = new TempDirectory();
    var path = Path.Combine(temp.Path, "file.bin");
    byte[] data = [0, 1, 2, 3, 4, 5];
    using var http = Client((r, _) =>
    {
        Check(r.Headers.Authorization is null, "API token leaked to file host");
        return Task.FromResult(Bytes(data));
    });
    var downloader = new FileDownloader(http);
    await downloader.DownloadAsync("https://files.test/file", path, data.Length, _ => { }, default);
    Check((await File.ReadAllBytesAsync(path)).SequenceEqual(data), "Wrong file bytes");
    Check(!File.Exists(path + ".part"), "Partial not renamed");
    await Throws<IOException>(() => downloader.DownloadAsync("https://files.test/file", path, data.Length, _ => { }, default));
    Check((await File.ReadAllBytesAsync(path)).SequenceEqual(data), "Existing file overwritten");
}
static async Task SeedPartial(string destination)
{
    await File.WriteAllBytesAsync(destination + ".part", [0, 1, 2]);
    await File.WriteAllTextAsync(destination + ".part.json", JsonSerializer.Serialize(new { ExpectedBytes = 6L, Validator = "\"stable\"" }));
}
static async Task TestResume()
{
    using var temp = new TempDirectory();
    var path = Path.Combine(temp.Path, "file.bin");
    await SeedPartial(path);
    using var http = Client((r, _) =>
    {
        Check(r.Headers.Range?.Ranges.Single().From == 3, "Missing Range");
        Check(r.Headers.IfRange?.EntityTag?.Tag == "\"stable\"", "Missing If-Range");
        var response = Bytes([3, 4, 5], HttpStatusCode.PartialContent);
        response.Content.Headers.ContentRange = new ContentRangeHeaderValue(3, 5, 6);
        return Task.FromResult(response);
    });
    await new FileDownloader(http).DownloadAsync("https://files.test/file", path, 6, _ => { }, default);
    Check((await File.ReadAllBytesAsync(path)).SequenceEqual(new byte[] { 0, 1, 2, 3, 4, 5 }), "Bad resumed content");
}
static async Task TestIgnoredRange()
{
    using var temp = new TempDirectory();
    var path = Path.Combine(temp.Path, "file.bin");
    await SeedPartial(path);
    using var http = Client((_, _) => Task.FromResult(Bytes([0, 1, 2, 3, 4, 5])));
    await new FileDownloader(http).DownloadAsync("https://files.test/file", path, 6, _ => { }, default);
    Check(new FileInfo(path).Length == 6, "Full response appended to partial");
}
static async Task TestInvalidResponses()
{
    using var temp = new TempDirectory();
    var path = Path.Combine(temp.Path, "file.bin");
    await SeedPartial(path);
    using var http = Client((_, _) =>
    {
        var r = Bytes([2, 3, 4, 5], HttpStatusCode.PartialContent);
        r.Content.Headers.ContentRange = new ContentRangeHeaderValue(2, 5, 6);
        return Task.FromResult(r);
    });
    await Throws<InvalidDataException>(() => new FileDownloader(http).DownloadAsync("https://files.test/file", path, 6, _ => { }, default));
    Check(!File.Exists(path), "Malformed range finalized");
    using var shortHttp = Client((_, _) =>
    {
        var r = Bytes([0, 1]); r.Content.Headers.ContentLength = 6; return Task.FromResult(r);
    });
    await Throws<IOException>(() => new FileDownloader(shortHttp).DownloadAsync("https://files.test/file", path, 6, _ => { }, default));
    Check(!File.Exists(path), "Truncated download finalized");
}
static async Task TestCancellation()
{
    using var temp = new TempDirectory();
    var path = Path.Combine(temp.Path, "file.bin");
    using var ct = new CancellationTokenSource();
    using var http = Client((_, _) => Task.FromResult(Bytes(new byte[512 * 1024])));
    await Throws<OperationCanceledException>(() => new FileDownloader(http).DownloadAsync("https://files.test/file", path, 512 * 1024,
        _ => ct.Cancel(), ct.Token));
    Check(File.Exists(path + ".part") && !File.Exists(path), "Cancellation lost partial or finalized file");
}
static async Task TestPipeline()
{
    using var temp = new TempDirectory();
    var selected = false;
    var seen = new List<string>();
    using var apiHttp = Client(async (r, ct) =>
    {
        Check(r.Headers.Authorization?.Parameter == "test-token", "Missing API bearer token");
        var endpoint = r.RequestUri!.AbsolutePath;
        seen.Add(endpoint);
        if (endpoint.EndsWith("addMagnet")) return Json(new { id = "RD123" }, HttpStatusCode.Created);
        if (endpoint.Contains("selectFiles"))
        {
            Check(await r.Content!.ReadAsStringAsync(ct) == "files=all", "Did not select ALL files");
            selected = true; return new(HttpStatusCode.NoContent);
        }
        if (endpoint.Contains("torrents/info")) return Json(new
        {
            id = "RD123", filename = "Example", status = selected ? "downloaded" : "waiting_files_selection", progress = selected ? 100 : 0, bytes = 6,
            files = new[] { new { id = 1, path = "/Example/a.txt", bytes = 3, selected = 1 }, new { id = 2, path = "/Example/sub/b.txt", bytes = 3, selected = 1 } },
            links = new[] { "https://rd.test/a", "https://rd.test/b" }
        });
        if (endpoint.EndsWith("unrestrict/link"))
        {
            var body = await r.Content!.ReadAsStringAsync(ct);
            var name = body.Contains("%2Fa", StringComparison.OrdinalIgnoreCase) ? "a.txt" : "b.txt";
            return Json(new { filename = name, filesize = 3, download = "https://files.test/" + name });
        }
        throw new Exception("Unexpected API call: " + endpoint);
    });
    var count = 0;
    using var files = Client((r, _) => { Check(r.Headers.Authorization is null, "Token leaked"); count++; return Task.FromResult(Bytes([1, 2, 3])); });
    var job = new DownloadJob { Source = "magnet:?xt=urn:btih:" + new string('A', 40) };
    await new DownloadEngine(new(apiHttp, "test-token"), files).RunAsync([job], temp.Path, 3, default, flattenFiles: false);
    Check(job.State == JobState.Completed, job.Detail);
    Check(selected && count == 2 && job.Files.All(f => f.Completed), "Not every file downloaded");
    Check(File.Exists(Path.Combine(job.OutputDirectory!, "Example", "sub", "b.txt")), "Did not preserve torrent directory");
    Check(seen.Count(p => p.EndsWith("addMagnet")) == 1, "Duplicate remote add");
    // Retry with persisted ID and completed files: do not add or download anything twice.
    await new DownloadEngine(new(apiHttp, "test-token"), files).RunAsync([job], temp.Path, 1, default);
    Check(count == 2 && seen.Count(p => p.EndsWith("addMagnet")) == 1, "Resume duplicated work");
    Check(job.FlattenFiles == false && File.Exists(Path.Combine(job.OutputDirectory!, "Example", "sub", "b.txt")), "Changed layout of existing structured job");
}

static HttpClient CachedTorrentApi(int fileSize = 1) => Client(async (r, ct) =>
{
    if (r.RequestUri!.AbsolutePath.Contains("torrents/info"))
    {
        var id = r.RequestUri.Segments.Last();
        return Json(new
        {
            id, filename = "Remote folder", status = "downloaded", progress = 100, bytes = 2 * fileSize,
            files = new[] { new { id = 1, path = "/Remote folder/one/same.txt", bytes = fileSize, selected = 1 }, new { id = 2, path = "/Remote folder/two/same.txt", bytes = fileSize, selected = 1 } },
            links = new[] { "https://rd.test/" + id + "/1", "https://rd.test/" + id + "/2" }
        });
    }
    Check(r.RequestUri.AbsolutePath.EndsWith("unrestrict/link"), "Unexpected cached-torrent API request");
    var body = await r.Content!.ReadAsStringAsync(ct);
    var source = Uri.UnescapeDataString(body["link=".Length..]);
    return Json(new { filename = "same.txt", filesize = fileSize, download = source.Replace("rd.test", "files.test") });
});

static async Task TestStopAndResume()
{
    using var temp = new TempDirectory();
    using var api = CachedTorrentApi(6);
    var blocked = new BlockingReadStream();
    var targetRequests = 0;
    var resumed = false;
    using var files = Client((r, _) =>
    {
        if (r.RequestUri!.AbsolutePath == "/target/1")
        {
            if (++targetRequests == 1)
            {
                var response = new HttpResponseMessage(HttpStatusCode.OK) { Content = new StreamContent(blocked) };
                response.Content.Headers.ContentLength = 6;
                response.Headers.ETag = new EntityTagHeaderValue("\"stable\"");
                return Task.FromResult(response);
            }
            Check(r.Headers.Range?.Ranges.Single().From == 3, "Resume did not request the remaining bytes");
            var partial = Bytes([3, 4, 5], HttpStatusCode.PartialContent);
            partial.Content.Headers.ContentRange = new(3, 5, 6);
            resumed = true;
            return Task.FromResult(partial);
        }
        return Task.FromResult(Bytes([0, 1, 2, 3, 4, 5]));
    });
    var target = new DownloadJob { TorrentId = "target" };
    var other = new DownloadJob { TorrentId = "other" };
    using var session = new DownloadEngine(new(api, "t"), files).CreateSession([target, other], temp.Path, 2);
    var work = session.EnqueueAsync(target);
    await blocked.Blocked.Task.WaitAsync(TimeSpan.FromSeconds(10));
    Check(!target.CanRetry && !target.CanResume && target.CanStop, "Active row actions incorrect");
    var otherWork = session.EnqueueAsync(other);
    await session.StopAsync(target).WaitAsync(TimeSpan.FromSeconds(5));
    Check(target.State == JobState.Stopped && target.CanResume && !target.IsActive, "Stop did not finish or enable Resume");
    var destination = Path.Combine(temp.Path, target.Files[0].RelativePath);
    Check(new FileInfo(destination + ".part").Length == 3 && !File.Exists(destination), "Stop lost partial bytes");
    await session.EnqueueAsync(target); // Resume while the other torrent still belongs to this session.
    await Task.WhenAll(work, otherWork);
    Check(other.State == JobState.Completed && target.State == JobState.Completed && resumed, "Stop affected another torrent or Resume failed");
    Check((await File.ReadAllBytesAsync(destination)).SequenceEqual(new byte[] { 0, 1, 2, 3, 4, 5 }), "Resumed file content differs");
    Check(session.ActiveCount == 0 && !target.IsStopping, "Session did not become idle");
}

static async Task TestRetryInSession()
{
    using var temp = new TempDirectory();
    using var api = CachedTorrentApi();
    var hold = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
    var blocked = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
    var fail = true;
    using var files = Client(async (r, ct) =>
    {
        if (r.RequestUri!.AbsolutePath == "/other/1")
        { blocked.TrySetResult(); await hold.Task.WaitAsync(ct); }
        if (r.RequestUri.AbsolutePath == "/retry/1" && fail) return new(HttpStatusCode.NotFound);
        return Bytes([42]);
    });
    var job = new DownloadJob { TorrentId = "retry" };
    var other = new DownloadJob { TorrentId = "other" };
    using var session = new DownloadEngine(new(api, "t"), files).CreateSession([job, other], temp.Path, 2);
    var otherWork = session.EnqueueAsync(other);
    await blocked.Task.WaitAsync(TimeSpan.FromSeconds(10));
    // First failure is deliberately retried by the engine before becoming a visible row error.
    await session.EnqueueAsync(job);
    Check(job.State == JobState.Failed && job.CanRetry && other.IsActive, "Retry is unavailable while another torrent runs");
    var names = job.Files.Select(f => f.RelativePath).ToArray();
    fail = false;
    await session.EnqueueAsync(job);
    Check(job.State == JobState.Completed && names[0] == job.Files[0].RelativePath, "Retry changed the destination or failed");
    hold.SetResult(); await otherWork;
}

static async Task TestRemoveQueued()
{
    using var temp = new TempDirectory();
    using var api = CachedTorrentApi(6);
    var blocked = new BlockingReadStream();
    using var files = Client((_, _) =>
    {
        var response = new HttpResponseMessage(HttpStatusCode.OK) { Content = new StreamContent(blocked) };
        response.Content.Headers.ContentLength = 6;
        return Task.FromResult(response);
    });
    var active = new DownloadJob { TorrentId = "active" };
    var removed = new DownloadJob { TorrentId = "removed" };
    var jobs = new List<DownloadJob> { active, removed };
    using var session = new DownloadEngine(new(api, "t"), files).CreateSession(jobs, temp.Path, 1);
    var running = session.EnqueueAsync(active);
    await blocked.Blocked.Task.WaitAsync(TimeSpan.FromSeconds(10));
    var queued = session.EnqueueAsync(removed);
    removed.IsRemoving = true;
    await session.StopAsync(removed).WaitAsync(TimeSpan.FromSeconds(5));
    jobs.Remove(removed);
    await session.EnqueueAsync(removed); // A stale Resume action must not resurrect a deleting row.
    Check(!removed.IsActive && removed.Files.Count == 0 && jobs.Count == 1, "Deleted queued torrent ran or reappeared");
    await session.StopAsync(active);
    await Task.WhenAll(running, queued);
    Check(File.Exists(Path.Combine(temp.Path, active.Files[0].RelativePath) + ".part"), "Removing a row deleted partial files");
}

static async Task TestStopAll()
{
    using var temp = new TempDirectory();
    using var api = CachedTorrentApi(6);
    var blocked = new BlockingReadStream();
    using var files = Client((_, _) =>
    {
        var response = new HttpResponseMessage(HttpStatusCode.OK) { Content = new StreamContent(blocked) };
        response.Content.Headers.ContentLength = 6;
        return Task.FromResult(response);
    });
    var jobs = new[] { new DownloadJob { TorrentId = "one" }, new DownloadJob { TorrentId = "two" } };
    using var session = new DownloadEngine(new(api, "t"), files).CreateSession(jobs, temp.Path, 1);
    var first = session.EnqueueAsync(jobs[0]);
    await blocked.Blocked.Task.WaitAsync(TimeSpan.FromSeconds(10));
    var second = session.EnqueueAsync(jobs[1]);
    await session.StopAllAsync().WaitAsync(TimeSpan.FromSeconds(5));
    await Task.WhenAll(first, second);
    Check(jobs.All(j => j.State == JobState.Stopped && !j.IsActive && j.CanResume), "Stop all left active or non-resumable work");
    var store = new SettingsStore(Path.Combine(temp.Path, "settings"));
    store.Save(new() { Jobs = jobs.ToList() });
    Check(store.Load().Jobs.All(j => j.State == JobState.Stopped), "Restart lost explicitly stopped state");
}

static Task TestActionStates()
{
    var job = new DownloadJob { State = JobState.Failed };
    Check(job.CanRetry && !job.CanResume && !job.CanStop && job.CanRemove, "Failed controls");
    job.State = JobState.Stopped;
    Check(!job.CanRetry && job.CanResume && !job.CanStop && job.CanRemove, "Stopped controls");
    job.IsRemoving = true;
    Check(!job.CanRetry && !job.CanResume && !job.CanStop && !job.CanRemove, "Deleting controls");
    job.IsRemoving = false; job.State = JobState.Completed;
    Check(!job.CanRetry && !job.CanResume && !job.CanStop && job.CanRemove, "Completed controls");
    return Task.CompletedTask;
}

static async Task TestFlatBatch()
{
    using var temp = new TempDirectory();
    await File.WriteAllTextAsync(Path.Combine(temp.Path, "same.txt"), "existing");
    await File.WriteAllTextAsync(Path.Combine(temp.Path, "same (2).txt.part"), "partial");
    await File.WriteAllTextAsync(Path.Combine(temp.Path, "same (3).txt.part.json"), "metadata");
    Directory.CreateDirectory(Path.Combine(temp.Path, "same (4).txt"));
    using var api = CachedTorrentApi();
    using var files = Client(async (r, _) =>
    {
        await Task.Yield(); // Allow another torrent to reserve its destination while this one is transferring.
        return Bytes([(byte)(r.RequestUri!.AbsolutePath.Contains("first") ? 1 : 2)]);
    });
    var jobs = new[] { new DownloadJob { TorrentId = "first" }, new DownloadJob { TorrentId = "second" } };
    await new DownloadEngine(new(api, "t"), files).RunAsync(jobs, temp.Path, 2, default);
    Check(jobs.All(j => j.State == JobState.Completed), string.Join("; ", jobs.Select(j => j.Detail)));
    Check(jobs.All(j => j.FlattenFiles == true && j.OutputDirectory == temp.Path), "Flat mode created torrent directories");
    var outputs = jobs.SelectMany(j => j.Files).ToArray();
    Check(outputs.Length == 4 && outputs.Select(f => f.RelativePath).Distinct(StringComparer.OrdinalIgnoreCase).Count() == 4, "Batch filenames collided");
    Check(outputs.All(f => Path.GetFileName(f.RelativePath) == f.RelativePath), "Flat mode retained subdirectories");
    foreach (var job in jobs)
        foreach (var file in job.Files)
            Check((await File.ReadAllBytesAsync(Path.Combine(temp.Path, file.RelativePath))).Single() == (job.TorrentId == "first" ? 1 : 2), "Files mixed between torrents");
    Check(await File.ReadAllTextAsync(Path.Combine(temp.Path, "same.txt")) == "existing", "Overwrote existing file");
    Check(await File.ReadAllTextAsync(Path.Combine(temp.Path, "same (2).txt.part")) == "partial", "Overwrote another partial");
    Check(await File.ReadAllTextAsync(Path.Combine(temp.Path, "same (3).txt.part.json")) == "metadata", "Overwrote another partial's metadata");
    Check(Directory.GetDirectories(temp.Path).Length == 1, "Created extra directories in flat mode");
}

static async Task TestFlatResume()
{
    using var temp = new TempDirectory();
    using var storage = new TempDirectory();
    using var ct = new CancellationTokenSource();
    using var api = CachedTorrentApi();
    var requests = 0;
    using var interruptedFiles = Client((_, _) =>
    {
        if (++requests == 2) ct.Cancel();
        return Task.FromResult(Bytes([42]));
    });
    var job = new DownloadJob { TorrentId = "resume" };
    await new DownloadEngine(new(api, "t"), interruptedFiles).RunAsync([job], temp.Path, 1, ct.Token);
    Check(job.State == JobState.Paused && job.Files.Count(f => f.Completed) == 1, "Did not pause after first file");
    var names = job.Files.Select(f => f.RelativePath).ToArray();
    var store = new SettingsStore(storage.Path);
    store.Save(new() { FlattenFiles = false, Jobs = [job] });
    var loaded = store.Load();
    var transfers = 0;
    using var resumedFiles = Client((_, _) => { transfers++; return Task.FromResult(Bytes([42])); });
    await new DownloadEngine(new(api, "t"), resumedFiles).RunAsync(loaded.Jobs, Path.Combine(temp.Path, "new-location"), 1, default, flattenFiles: loaded.FlattenFiles);
    var resumed = loaded.Jobs.Single();
    Check(resumed.State == JobState.Completed, resumed.Detail);
    Check(transfers == 1 && resumed.Files.Select(f => f.RelativePath).SequenceEqual(names), "Resume downloaded completed files or renamed output");
    Check(resumed.FlattenFiles == true && resumed.OutputDirectory == temp.Path && Directory.GetDirectories(temp.Path).Length == 0, "Resume changed layout or output folder");
}

static async Task TestLegacyLayout()
{
    using var temp = new TempDirectory();
    var directory = Path.Combine(temp.Path, "Legacy torrent");
    Directory.CreateDirectory(directory);
    await File.WriteAllBytesAsync(Path.Combine(directory, "same.txt"), [42]);
    using var api = CachedTorrentApi();
    using var files = Client((_, _) => Task.FromResult(Bytes([42])));
    var job = new DownloadJob
    {
        TorrentId = "legacy", OutputDirectory = directory,
        Files = [new() { LinkIndex = 0, RelativePath = "same.txt", ExpectedBytes = 1, Completed = true }]
    };
    await new DownloadEngine(new(api, "t"), files).RunAsync([job], temp.Path, 1, default);
    Check(job.State == JobState.Completed && job.FlattenFiles == false && job.OutputDirectory == directory, job.Detail);
    Check(!File.Exists(Path.Combine(temp.Path, "same.txt")), "Legacy queue moved files to flat folder");
}

static Task TestLayoutSettings()
{
    using var temp = new TempDirectory();
    var store = new SettingsStore(temp.Path);
    File.WriteAllText(Path.Combine(temp.Path, "settings.json"), "{\"Concurrency\":2,\"Jobs\":[]}");
    Check(store.Load().FlattenFiles, "Old settings did not default to flat mode");
    foreach (var flatten in new[] { false, true })
    {
        store.Save(new() { FlattenFiles = flatten });
        Check(store.Load().FlattenFiles == flatten, "Layout setting was not persisted");
    }
    return Task.CompletedTask;
}

static async Task TestFailureIsolation()
{
    using var temp = new TempDirectory();
    using var api = Client((r, _) =>
    {
        if (r.RequestUri!.AbsolutePath.EndsWith("bad")) return Task.FromResult(Json(new { error = "unknown_resource", error_code = 7 }, HttpStatusCode.NotFound));
        if (r.RequestUri.AbsolutePath.Contains("info")) return Task.FromResult(Json(new
        {
            id = "good", filename = "Good", status = "downloaded", progress = 100, bytes = 1,
            files = new[] { new { id = 1, path = "/a", bytes = 1, selected = 1 } }, links = new[] { "https://rd.test/a" }
        }));
        return Task.FromResult(Json(new { filename = "a", filesize = 1, download = "https://files.test/a" }));
    });
    using var files = Client((_, _) => Task.FromResult(Bytes([42])));
    var jobs = new[] { new DownloadJob { TorrentId = "bad" }, new DownloadJob { TorrentId = "good" } };
    await new DownloadEngine(new(api, "t"), files).RunAsync(jobs, temp.Path, 2, default);
    Check(jobs[0].State == JobState.Failed && jobs[1].State == JobState.Completed, "One failure stopped batch");
}
static async Task TestTorrentUpload()
{
    using var http = Client(async (r, ct) =>
    {
        Check(r.Method == HttpMethod.Put && r.RequestUri!.AbsolutePath.EndsWith("addTorrent"), "Wrong upload method");
        Check((await r.Content!.ReadAsByteArrayAsync(ct)).SequenceEqual(new byte[] { 100, 49, 58, 101 }), "Not raw torrent bytes");
        Check(r.Headers.Authorization?.Parameter == "secret", "Upload token missing");
        return Json(new { id = "uploaded" }, HttpStatusCode.Created);
    });
    Check(await new RealDebridClient(http, "secret").AddTorrentAsync([100, 49, 58, 101], default) == "uploaded", "Upload result ID");
}
static Task TestPersistence()
{
    using var temp = new TempDirectory();
    var store = new SettingsStore(temp.Path);
    store.Save(new() { Jobs = [new() { TorrentId = "remote", State = JobState.Downloading, Files = [new() { RelativePath = "a", ExpectedBytes = 123, Completed = true }] }] });
    var saved = store.Load();
    Check(saved.Jobs.Single().State == JobState.Paused && saved.Jobs[0].TorrentId == "remote", "Remote ID or paused state lost");
    Check(saved.Jobs[0].Files.Single().Completed, "File completion state lost");
    Check(!File.Exists(Path.Combine(temp.Path, "settings.json.tmp")), "Atomic write left temporary file");
    return Task.CompletedTask;
}
static async Task TestApiFailure()
{
    using var http = Client((_, _) => Task.FromResult(Json(new { error = "bad_token", error_code = 8 }, HttpStatusCode.Unauthorized)));
    try { await new RealDebridClient(http, "super-secret-token").GetUserAsync(default); throw new Exception("401 accepted"); }
    catch (RdApiException ex) { Check(!DownloadEngine.FriendlyError(ex).Contains("super-secret"), "Credential leaked"); }
}

sealed class FakeHandler(Func<HttpRequestMessage, CancellationToken, Task<HttpResponseMessage>> handler) : HttpMessageHandler
{
    protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken) => handler(request, cancellationToken);
}
sealed class BlockingReadStream : Stream
{
    private int _phase;
    public TaskCompletionSource Blocked { get; } = new(TaskCreationOptions.RunContinuationsAsynchronously);
    public override bool CanRead => true;
    public override bool CanSeek => false;
    public override bool CanWrite => false;
    public override long Length => 6;
    public override long Position { get => throw new NotSupportedException(); set => throw new NotSupportedException(); }
    public override async ValueTask<int> ReadAsync(Memory<byte> buffer, CancellationToken cancellationToken = default)
    {
        if (_phase++ == 0) { new byte[] { 0, 1, 2 }.CopyTo(buffer); return 3; }
        Blocked.TrySetResult();
        await Task.Delay(Timeout.Infinite, cancellationToken);
        return 0;
    }
    public override int Read(byte[] buffer, int offset, int count) => throw new NotSupportedException();
    public override void Flush() => throw new NotSupportedException();
    public override long Seek(long offset, SeekOrigin origin) => throw new NotSupportedException();
    public override void SetLength(long value) => throw new NotSupportedException();
    public override void Write(byte[] buffer, int offset, int count) => throw new NotSupportedException();
}
sealed class TempDirectory : IDisposable
{
    public string Path { get; } = System.IO.Path.Combine(System.IO.Path.GetTempPath(), "RdBatch.Tests", Guid.NewGuid().ToString("N"));
    public TempDirectory() => Directory.CreateDirectory(Path);
    public void Dispose() => Directory.Delete(Path, true);
}
