using System.Text.Json;

namespace RdBatch.Core;

public sealed class SettingsStore(string directory)
{
    private string FilePath => Path.Combine(directory, "settings.json");
    private readonly JsonSerializerOptions _options = new() { WriteIndented = true };
    private bool _saveBlocked;
    public LocalizedMessage? LoadWarningMessage { get; private set; }
    public string? LoadWarning => LoadWarningMessage?.Render();

    public AppSettings Load()
    {
        if (!File.Exists(FilePath)) return new();
        try
        {
            var settings = JsonSerializer.Deserialize<AppSettings>(File.ReadAllText(FilePath), _options) ?? new();
            if (settings.Jobs is null || settings.Jobs.Any(j => j is null || j.Files is null)) throw new JsonException("Invalid queue");
            settings.Concurrency = Math.Clamp(settings.Concurrency, 1, 5);
            settings.Language = Text.NormalizeLanguage(settings.Language);
            foreach (var job in settings.Jobs.Where(j => j.State is not (JobState.Completed or JobState.Failed or JobState.Queued or JobState.Stopped)))
            { job.State = JobState.Paused; job.SetDetail("Bấm Bắt đầu để tiếp tục"); }
            return settings;
        }
        catch (Exception ex) when (ex is JsonException or IOException or UnauthorizedAccessException)
        {
            try
            {
                var backup = FilePath + ".corrupt-" + DateTime.UtcNow.ToString("yyyyMMdd-HHmmss-fff");
                File.Copy(FilePath, backup);
                LoadWarningMessage = LocalizedMessage.Create("Không đọc được hàng đợi cũ. Đã giữ bản gốc: {0}", backup);
            }
            catch (Exception backupError) when (backupError is IOException or UnauthorizedAccessException)
            {
                _saveBlocked = true;
                LoadWarningMessage = LocalizedMessage.Create("Không đọc hoặc sao lưu được hàng đợi cũ. Tạm ngừng ghi settings.json để giữ bản gốc.");
            }
            return new();
        }
    }

    public void Save(AppSettings settings)
    {
        if (_saveBlocked) throw new IOException(LoadWarning);
        Directory.CreateDirectory(directory);
        var temporary = FilePath + ".tmp";
        File.WriteAllText(temporary, JsonSerializer.Serialize(settings, _options));
        File.Move(temporary, FilePath, true);
    }
}
