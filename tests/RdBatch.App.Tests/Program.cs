using System.IO;
using System.Text.Json;
using System.Text.RegularExpressions;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Threading;
using RdBatch.App;
using RdBatch.Core;

internal static class Program
{
    [STAThread]
    private static int Main()
    {
        var directory = Path.Combine(Path.GetTempPath(), "RdBatch.App.Tests", Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(directory);
        MainWindow? window = null;
        try
        {
            var app = new Application
            {
                Resources = (ResourceDictionary)Application.LoadComponent(new Uri("/RealDebridBatch;component/Styles.xaml", UriKind.Relative))
            };
            var store = new SettingsStore(directory);
            Text.SetLanguage("vi");
            window = new MainWindow(store, store.Load());
            Check(window.Icon is not null, "Window icon missing");
            Check(Get<Button>(window, "ClearQueueButton").Content as string == "Xóa toàn bộ", "Vietnamese clear label missing");
            Check(!Get<Button>(window, "ClearQueueButton").IsEnabled, "Empty queue should disable Clear");
            Get<ComboBox>(window, "LanguageBox").SelectedIndex = 1;
            FlushBindings();
            Check(Get<Button>(window, "ConnectButton").Content as string == "Connect", $"Live button binding did not switch to English: language={Text.Language}, revision={TranslationSource.Instance.Revision}, content={Get<Button>(window, "ConnectButton").Content}");
            Check(Get<Button>(window, "ClearQueueButton").Content as string == "Clear queue", "Clear label did not switch to English");
            Check(Get<DataGrid>(window, "QueueGrid").Columns[1].Header as string == "SIZE", "DataGrid header translation missing");
            Check(Get<TextBlock>(window, "SourcePlaceholder").Text.Contains("Or drop .torrent files"), "Multiline placeholder was not translated");
            Check(store.Load().Language == "en", "UI language choice was not saved");

            Get<TextBox>(window, "SourcesBox").Text = "magnet:?xt=urn:btih:" + new string('A', 40) + "&dn=Example+Torrent\ninvalid";
            Get<Button>(window, "AddButton").RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
            FlushBindings();
            Check(window.Jobs.Count == 1 && Get<Button>(window, "ClearQueueButton").IsEnabled, "Add/Clear controls are inconsistent");
            Check(Get<TextBlock>(window, "StatusText").Text.Contains("1 invalid lines"), "English status missing");
            Check(Get<TextBlock>(window, "StatusText").ToolTip is string errors && errors.StartsWith("Line 2:"), "English parser tooltip missing");
            window.Jobs[0].State = JobState.Completed;
            window.Jobs[0].SetDetail("Đã lưu {0} file · {1}", 2, "C:\\Downloads");
            PrepareScreenshot(window);
            Render(window, "english");
            Get<ComboBox>(window, "LanguageBox").SelectedIndex = 0;
            FlushBindings();
            Check(Get<Button>(window, "ConnectButton").Content as string == "Kiểm tra", "Switching back left a stale button");
            Check(Get<TextBlock>(window, "StatusText").Text.StartsWith("Đã thêm 1 torrent"), "Switching back left a stale status");
            Check(window.Jobs[0].Detail.StartsWith("Đã lưu 2 file"), "Switching back left a stale queue detail");
            Get<TextBlock>(window, "StatusText").Text = Text.Get("Sẵn sàng · Token được mã hóa bằng tài khoản Windows khi lưu.");
            Render(window, "vietnamese");
            ValidateCatalog();
            Console.WriteLine("PASS WPF language bindings, headers, multiline text, status, saved preference, queue controls and icon");
            Console.WriteLine("PASS English/Vietnamese layout renders and translation format placeholders");
            return 0;
        }
        catch (Exception ex) { Console.Error.WriteLine(ex); return 1; }
        finally { window?.Close(); Text.SetLanguage("vi"); Directory.Delete(directory, true); }
    }

    private static T Get<T>(MainWindow window, string name) where T : FrameworkElement => (T)window.FindName(name);
    private static void Check(bool condition, string message) { if (!condition) throw new InvalidOperationException(message); }
    private static void FlushBindings() => Dispatcher.CurrentDispatcher.Invoke(DispatcherPriority.ApplicationIdle, new Action(() => { }));

    private static void PrepareScreenshot(MainWindow window)
    {
        // Public documentation uses sample data, with no real account or user-specific paths.
        Get<TextBox>(window, "FolderBox").Text = @"C:\Downloads\Real-Debrid";
        Get<TextBox>(window, "SourcesBox").Clear();
        var completed = window.Jobs[0];
        completed.Name = "Sample documents";
        completed.RemoteProgress = 100;
        completed.LocalProgress = 100;
        completed.TotalBytes = (long)(12.5 * 1024 * 1024);
        completed.SetDetail("Đã lưu {0} file · {1}", 2, @"C:\Downloads\Real-Debrid");
        var stopped = new DownloadJob
        {
            Name = "Sample videos", State = JobState.Stopped,
            TotalBytes = 256 * 1024 * 1024, RemoteProgress = 100, LocalProgress = 52
        };
        stopped.SetDetail("Đã dừng · giữ file tạm · bấm Resume để tải tiếp");
        window.Jobs.Add(stopped);
        window.Jobs.Add(new() { Name = "Linux image", State = JobState.Queued, TotalBytes = 2L * 1024 * 1024 * 1024 });
        Get<ComboBox>(window, "LanguageBox").SelectedIndex = 0;
        Get<ComboBox>(window, "LanguageBox").SelectedIndex = 1;
        Get<TextBlock>(window, "StatusText").Text = Text.Get("Sẵn sàng · Token được mã hóa bằng tài khoản Windows khi lưu.");
    }

    private static void Render(MainWindow window, string language)
    {
        const int width = 1224;
        const int height = 920;
        var root = (FrameworkElement)window.Content;
        root.Measure(new Size(width, height));
        root.Arrange(new Rect(0, 0, width, height));
        root.UpdateLayout();
        FlushBindings();
        var bitmap = new RenderTargetBitmap(width, height, 96, 96, PixelFormats.Pbgra32);
        var background = new DrawingVisual();
        using (var drawing = background.RenderOpen()) drawing.DrawRectangle(window.Background, null, new Rect(0, 0, width, height));
        bitmap.Render(background);
        bitmap.Render(root);
        var encoder = new PngBitmapEncoder();
        encoder.Frames.Add(BitmapFrame.Create(bitmap));
        var output = Path.GetFullPath(Path.Combine("artifacts", "ui-checks"));
        Directory.CreateDirectory(output);
        using var stream = File.Create(Path.Combine(output, language + ".png"));
        encoder.Save(stream);
    }

    private static void ValidateCatalog()
    {
        using var stream = typeof(Text).Assembly.GetManifestResourceStream("RdBatch.Core.Strings.en.json")!;
        var catalog = JsonSerializer.Deserialize<Dictionary<string, string>>(stream)!;
        foreach (var (source, translation) in catalog)
        {
            var sourceArguments = Regex.Matches(source, @"\{\d+[^}]*\}").Select(match => match.Value).Order().ToArray();
            var translatedArguments = Regex.Matches(translation, @"\{\d+[^}]*\}").Select(match => match.Value).Order().ToArray();
            Check(sourceArguments.SequenceEqual(translatedArguments), "Translation arguments differ: " + source);
        }
    }
}
