using System.Windows;
using System.IO;
using RdBatch.Core;

namespace RdBatch.App;

public partial class App : Application
{
    private Mutex? _instance;
    protected override void OnStartup(StartupEventArgs e)
    {
        base.OnStartup(e);
        var settingsDirectory = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "RealDebridBatch");
        var store = new SettingsStore(settingsDirectory);
        var settings = store.Load();
        Text.SetLanguage(settings.Language);
        _instance = new Mutex(true, "Local\\RealDebridBatch-" + Environment.UserName, out var created);
        if (!created) { MessageBox.Show(Text.Get("Real-Debrid Batch đang mở. Hãy dùng cửa sổ hiện tại."), "Real-Debrid Batch"); Shutdown(); return; }
        new MainWindow(store, settings).Show();
    }
    protected override void OnExit(ExitEventArgs e) { _instance?.Dispose(); base.OnExit(e); }
}
