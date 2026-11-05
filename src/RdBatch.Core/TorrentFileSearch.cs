namespace RdBatch.Core;

public static class TorrentFileSearch
{
    private static readonly HashSet<string> VideoExtensions = new(StringComparer.OrdinalIgnoreCase)
    { ".mp4", ".mkv", ".avi", ".mov", ".wmv", ".webm", ".m4v", ".ts", ".m2ts", ".mts", ".mpg", ".mpeg", ".flv", ".vob", ".ogv", ".3gp", ".divx", ".rmvb" };

    public static bool IsVideo(string path) => VideoExtensions.Contains(Path.GetExtension(path));

    public static IEnumerable<TorrentFile> Filter(IEnumerable<TorrentFile> files, string query, bool videosOnly, string folder = "")
    {
        var prefix = Normalize(folder).Trim('/') is { Length: > 0 } directory ? directory + "/" : "";
        var terms = query.Split((char[]?)null, StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries);
        return files.Where(file =>
            Normalize(file.Path).TrimStart('/').StartsWith(prefix, StringComparison.OrdinalIgnoreCase) &&
            (!videosOnly || IsVideo(file.Path)) &&
            terms.All(term => Normalize(file.Path).Contains(term, StringComparison.OrdinalIgnoreCase)));
    }

    public static string Normalize(string path) => path.Replace('\\', '/');
}
