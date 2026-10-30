using System.Globalization;
using System.Text.Json;

namespace RdBatch.Core;

/// <summary>Vietnamese source messages and their English translations.</summary>
public static class Text
{
    private static readonly Dictionary<string, string> English = LoadEnglish();
    public static string Language { get; private set; } = "en";
    public static event Action? LanguageChanged;
    public static string NormalizeLanguage(string? language) => language == "vi" ? "vi" : "en";

    public static void SetLanguage(string? language)
    {
        var normalized = NormalizeLanguage(language);
        if (Language == normalized) return;
        Language = normalized;
        LanguageChanged?.Invoke();
    }

    public static string Get(string source, params object?[] arguments)
    {
        // Also recognize exact messages saved by an older language/version.
        if (!English.ContainsKey(source))
            source = English.FirstOrDefault(pair => pair.Value == source).Key ?? source;
        var template = Language == "en" ? English.GetValueOrDefault(source, source) : source;
        return arguments.Length == 0 ? template : string.Format(
            CultureInfo.GetCultureInfo(Language == "en" ? "en-US" : "vi-VN"), template, arguments);
    }

    private static Dictionary<string, string> LoadEnglish()
    {
        using var stream = typeof(Text).Assembly.GetManifestResourceStream("RdBatch.Core.Strings.en.json")
            ?? throw new InvalidOperationException("Missing English translations.");
        return JsonSerializer.Deserialize<Dictionary<string, string>>(stream)!;
    }
}

// Keep templates and arguments so restored queue details can change language, too.
public sealed record LocalizedMessage(string Key, string[] Arguments)
{
    public static LocalizedMessage Create(string key, params object?[] arguments)
        => new(key, arguments.Select(value => Convert.ToString(value, CultureInfo.InvariantCulture) ?? "").ToArray());
    public string Render() => Text.Get(Key, Arguments.Cast<object?>().ToArray());
}
