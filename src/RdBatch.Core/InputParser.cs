using System.Text.RegularExpressions;

namespace RdBatch.Core;

public sealed record ParsedSource(string Source, string Key, string Name);
public sealed record ParseResult(List<ParsedSource> Sources, List<string> Errors);

public static partial class InputParser
{
    public static ParseResult Parse(string text)
    {
        var sources = new List<ParsedSource>();
        var errors = new List<string>();
        var keys = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (var rawLine in text.Split(['\r', '\n'], StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries))
        {
            // Multiple magnets copied on one line are also accepted. Local paths may contain spaces.
            var tokens = rawLine.StartsWith("magnet:", StringComparison.OrdinalIgnoreCase)
                ? MagnetSplit().Split(rawLine).Where(x => !string.IsNullOrWhiteSpace(x)) : [rawLine];
            foreach (var token in tokens)
            {
                var source = token.Trim().Trim('"');
                ParsedSource? parsed = null;
                if (source.StartsWith("magnet:?", StringComparison.OrdinalIgnoreCase))
                {
                    var query = source[8..].Split('&').Select(p => p.Split('=', 2))
                        .Where(p => p.Length == 2).Select(p => (Key: p[0], Value: Uri.UnescapeDataString(p[1]))).ToList();
                    var hash = query.FirstOrDefault(p => p.Key.Equals("xt", StringComparison.OrdinalIgnoreCase) && p.Value.StartsWith("urn:btih:", StringComparison.OrdinalIgnoreCase)).Value;
                    if (hash is not null && HashPattern().IsMatch(hash[9..]))
                    {
                        var name = query.FirstOrDefault(p => p.Key.Equals("dn", StringComparison.OrdinalIgnoreCase)).Value;
                        parsed = new(source, "btih:" + NormalizeHash(hash[9..]), string.IsNullOrWhiteSpace(name) ? "Magnet · " + hash[9..][..12] : name.Replace('+', ' '));
                    }
                }
                else if (Uri.TryCreate(source, UriKind.Absolute, out var uri) && uri.Scheme is "http" or "https")
                    parsed = new(source, uri.AbsoluteUri, Path.GetFileName(uri.LocalPath) is { Length: > 0 } n ? n : uri.Host);
                else if (Path.IsPathFullyQualified(source) && source.EndsWith(".torrent", StringComparison.OrdinalIgnoreCase) && File.Exists(source))
                    parsed = new(Path.GetFullPath(source), Path.GetFullPath(source), Path.GetFileNameWithoutExtension(source));

                if (parsed is null) errors.Add(Text.Get("Dòng {0}: cần magnet, URL .torrent hoặc đường dẫn file .torrent tồn tại.", sources.Count + errors.Count + 1));
                else if (keys.Add(parsed.Key)) sources.Add(parsed);
            }
        }
        return new(sources, errors);
    }

    private static string NormalizeHash(string value)
    {
        if (value.Length == 40) return value.ToUpperInvariant();
        const string alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
        var bytes = new List<byte>();
        var buffer = 0;
        var bits = 0;
        foreach (var c in value.ToUpperInvariant())
        {
            buffer = (buffer << 5) | alphabet.IndexOf(c);
            bits += 5;
            if (bits >= 8) { bits -= 8; bytes.Add((byte)(buffer >> bits)); }
        }
        return Convert.ToHexString(bytes.ToArray());
    }

    [GeneratedRegex(@"\s+(?=magnet:\?)", RegexOptions.IgnoreCase)] private static partial Regex MagnetSplit();
    [GeneratedRegex(@"\A(?:[a-fA-F0-9]{40}|[a-zA-Z2-7]{32})\z")] private static partial Regex HashPattern();
}
