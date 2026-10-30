namespace RdBatch.Core;

// Keep both final filenames and the downloader's temporary files distinct across the whole batch.
internal sealed class DownloadPathAllocator
{
    private readonly HashSet<string> _reserved = new(StringComparer.OrdinalIgnoreCase);
    private readonly object _gate = new();

    public DownloadPathAllocator(IEnumerable<DownloadJob> jobs)
    {
        foreach (var job in jobs.Where(j => j.OutputDirectory is not null))
            foreach (var file in job.Files)
                Reserve(SafePaths.UnderRoot(job.OutputDirectory!, file.RelativePath));
    }

    public string Allocate(string root, string relative)
    {
        lock (_gate)
        {
            var candidate = relative;
            var suffix = 2;
            while (Occupied(SafePaths.UnderRoot(root, candidate)))
                candidate = Path.Combine(Path.GetDirectoryName(relative) ?? "",
                    Path.GetFileNameWithoutExtension(relative) + $" ({suffix++})" + Path.GetExtension(relative));
            Reserve(SafePaths.UnderRoot(root, candidate));
            return candidate;
        }
    }

    private bool Occupied(string path) => Paths(path).Any(p => _reserved.Contains(p) || File.Exists(p) || Directory.Exists(p));
    private void Reserve(string path) { foreach (var p in Paths(path)) _reserved.Add(p); }
    private static string[] Paths(string path) => [path, path + ".part", path + ".part.json"];
}
