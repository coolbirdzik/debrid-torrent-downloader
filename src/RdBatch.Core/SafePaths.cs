using System.Text.RegularExpressions;

namespace RdBatch.Core;

public static partial class SafePaths
{
    public static string Segment(string name)
    {
        var invalid = Path.GetInvalidFileNameChars().Concat("<>:\"/\\|?*").ToHashSet();
        var clean = new string(name.Select(c => invalid.Contains(c) || char.IsControl(c) ? '_' : c).ToArray()).Trim().TrimEnd('.', ' ');
        if (clean is "" or "." or "..") clean = "unnamed";
        if (ReservedName().IsMatch(clean)) clean = "_" + clean;
        return clean.Length > 130 ? clean[..100] + "_" + Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(System.Text.Encoding.UTF8.GetBytes(clean)))[..12] + Path.GetExtension(clean)[..Math.Min(12, Path.GetExtension(clean).Length)] : clean;
    }

    public static string Relative(string path)
    {
        var parts = path.Replace('\\', '/').Split('/', StringSplitOptions.RemoveEmptyEntries);
        if (parts.Any(p => p is "." or "..")) throw new InvalidDataException(Text.Get("Torrent có đường dẫn không an toàn."));
        return Path.Combine(parts.Length == 0 ? ["unnamed"] : parts.Select(Segment).ToArray());
    }

    public static string UnderRoot(string root, string relative)
    {
        var fullRoot = Path.GetFullPath(root).TrimEnd(Path.DirectorySeparatorChar) + Path.DirectorySeparatorChar;
        var fullPath = Path.GetFullPath(Path.Combine(fullRoot, relative));
        if (!fullPath.StartsWith(fullRoot, StringComparison.OrdinalIgnoreCase)) throw new InvalidDataException(Text.Get("Đường dẫn nằm ngoài thư mục tải."));
        return fullPath;
    }

    public static void EnsureNoReparsePoints(string root, string destination)
    {
        var fullRoot = Path.GetFullPath(root);
        var current = Path.GetDirectoryName(destination);
        while (current is not null)
        {
            if (Directory.Exists(current) && (File.GetAttributes(current) & FileAttributes.ReparsePoint) != 0)
                throw new IOException(Text.Get("Thư mục tải có junction/symlink. Hãy chọn một thư mục thông thường."));
            if (current.Equals(fullRoot, StringComparison.OrdinalIgnoreCase)) break;
            current = Path.GetDirectoryName(current);
        }
        if (File.Exists(destination) && (File.GetAttributes(destination) & FileAttributes.ReparsePoint) != 0)
            throw new IOException(Text.Get("File đích là symlink."));
    }

    [GeneratedRegex(@"^(CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(?:\.|$)", RegexOptions.IgnoreCase)] private static partial Regex ReservedName();
}
