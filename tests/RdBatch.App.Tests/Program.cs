using System.IO;
using System.ComponentModel;
using System.Net;
using System.Net.Http;
using System.Text.Json;
using System.Text.RegularExpressions;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
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
            SynchronizationContext.SetSynchronizationContext(new DispatcherSynchronizationContext());
            var store = new SettingsStore(directory);
            var remoteAdds = 0;
            var apiRequests = 0;
            var handler = new FakeApi(request =>
            {
                apiRequests++;
                var path = request.RequestUri!.AbsolutePath;
                object response = path.EndsWith("/user") ? new { username = "Sample", type = "premium", premium = 3600 } :
                    path.EndsWith("/addMagnet") ? new { id = "preview-" + ++remoteAdds } :
                    path.Contains("/info/") ? new
                    {
                        id = "preview-1", filename = "Example", status = "waiting_files_selection", progress = 0, bytes = 600,
                        files = new[]
                        {
                            new TorrentFile(1, "/Example/Season 01/S01E01.MKV", 734832640, 0),
                            new TorrentFile(2, "/Example/Season 01/S01E02.mp4", 697321472, 0),
                            new TorrentFile(3, "/Example/Season 02/S02E01.mkv", 802031616, 0),
                            new TorrentFile(4, "/Example/readme.txt", 2416, 0),
                            new TorrentFile(5, "/Example/Subtitles/S01E01.srt", 43129, 0),
                            new TorrentFile(6, "/Example/Extras/trailer.mov", 52489216, 0)
                        }, links = Array.Empty<string>()
                    } : throw new InvalidOperationException("Inspection must not select or download files: " + path);
                return new(HttpStatusCode.OK) { Content = new StringContent(JsonSerializer.Serialize(response)) };
            });
            window = new MainWindow(store, store.Load(), new HttpClient(handler));
            Check(Text.Language == "en", "First launch should use English");
            Get<ComboBox>(window, "ThemeBox").SelectedIndex = 1;
            FlushBindings();
            Check(window.Icon is not null, "Window icon missing");
            Check(Get<Button>(window, "ClearQueueButton").Content as string == "Clear queue", "English default label missing");
            Get<TextBox>(window, "FolderBox").Text = @"C:\Downloads\Real-Debrid";
            Render(window, "empty");
            Get<ComboBox>(window, "LanguageBox").SelectedIndex = 0;
            FlushBindings();
            Check(Get<Button>(window, "ClearQueueButton").Content as string == "Xóa toàn bộ", "Vietnamese clear label missing");
            Check(!Get<Button>(window, "ClearQueueButton").IsEnabled, "Empty queue should disable Clear");
            Get<ComboBox>(window, "LanguageBox").SelectedIndex = 1;
            FlushBindings();
            Check(Get<Button>(window, "ConnectButton").Content as string == "Connect", $"Live button binding did not switch to English: language={Text.Language}, revision={TranslationSource.Instance.Revision}, content={Get<Button>(window, "ConnectButton").Content}");
            Check(Get<Button>(window, "ClearQueueButton").Content as string == "Clear queue", "Clear label did not switch to English");
            Check(Get<DataGrid>(window, "QueueGrid").Columns[1].Header as string == "Size", "DataGrid header translation missing");
            Check(Get<TextBlock>(window, "SourcePlaceholder").Text.Contains("Or drop .torrent files"), "Multiline placeholder was not translated");
            Check(store.Load().Language == "en", "UI language choice was not saved");

            Get<TextBox>(window, "SourcesBox").Text = "magnet:?xt=urn:btih:" + new string('A', 40) + "&dn=Example+Torrent\ninvalid";
            Get<PasswordBox>(window, "TokenBox").Password = "test-token";
            Get<Button>(window, "AddButton").RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
            PumpUntil(() => Get<Button>(window, "AddButton").IsEnabled);
            Check(window.Library.Count == 1 && window.BrowserFiles.Count == 6 && window.Jobs.Count == 0, "Inspection should show files without queueing or downloading");
            Check(Get<TextBlock>(window, "StatusText").Text.Contains("1 errors"), "English status missing");
            Check(Get<TextBlock>(window, "StatusText").ToolTip is string errors && errors.StartsWith("Line 2:"), "English parser tooltip missing");
            Get<TextBox>(window, "SourcesBox").Clear();
            Check(window.FindName("SourceExpander") is null, "Source input should have no automatic expand/collapse control");
            Get<TextBox>(window, "SearchFilesBox").Text = "s01";
            Check(window.BrowserFiles.Count == 3, "Search should include nested files and subtitles");
            Get<CheckBox>(window, "VideoOnlyBox").IsChecked = true;
            Check(window.BrowserFiles.Count == 2, "Video filter should exclude subtitle matches");
            Click(window, "SelectResultsButton");
            Check(window.BrowserFiles.All(file => file.IsSelected), "Select results missed visible files");
            Get<TextBox>(window, "SearchFilesBox").Text = "S02";
            Check(window.BrowserFiles.Count == 1 && !window.BrowserFiles[0].IsSelected && !Get<Button>(window, "QueueFilesButton").IsEnabled, "Changing the search retained hidden selections");
            Get<TextBox>(window, "SearchFilesBox").Text = "  S01  ";
            Click(window, "SelectResultsButton");
            Render(window, "browser-english");
            Render(window, "browser-compact-english", 1124, 696);
            Get<ComboBox>(window, "BrowserViewBox").SelectedIndex = 1;
            Check(window.BrowserFiles.All(file => file.IsSelected) && Get<ScrollViewer>(window, "BrowserTilesScroll").Visibility == Visibility.Visible, "Grid switch lost selection or did not switch the view");
            Check(store.Load().BrowserView == "grid", "Grid preference was not saved");
            Render(window, "browser-grid-english");
            Render(window, "browser-grid-compact-english", 1124, 696);
            Get<ComboBox>(window, "LanguageBox").SelectedIndex = 0;
            Check(window.BrowserFiles.All(file => file.IsSelected), "Switching language should preserve file selection");
            Render(window, "browser-vietnamese");
            Render(window, "browser-compact-vietnamese", 1124, 696);
            Render(window, "browser-grid-vietnamese");
            Get<ComboBox>(window, "ThemeBox").SelectedIndex = 2;
            Render(window, "browser-vietnamese-dark");
            Get<ComboBox>(window, "LanguageBox").SelectedIndex = 1;
            Render(window, "browser-english-dark");
            Get<ComboBox>(window, "ThemeBox").SelectedIndex = 1;
            Get<ComboBox>(window, "BrowserViewBox").SelectedIndex = 0;
            Click(window, "QueueFilesButton");
            Check(window.Jobs.Count == 1 && window.Jobs[0].SelectedFileIds!.SequenceEqual([1, 2]), "Queue should contain only the two searched videos");
            Check(window.Jobs[0].TorrentId == "preview-1" && window.BrowserFiles.All(file => !file.CanQueue), "Queued results should reuse the preview ID and prevent duplicate queueing");
            Get<TextBox>(window, "SearchFilesBox").Text = "S02";
            Click(window, "SelectResultsButton"); Click(window, "QueueFilesButton");
            Check(window.Jobs.Count == 2 && window.Jobs[1].TorrentId is null && window.Jobs[1].SelectedFileIds!.SequenceEqual([3]), "A later selection must use a separate remote torrent");
            Check(remoteAdds == 1, "Search/queue should not create extra remote torrents before starting");
            var saved = store.Load();
            Check(saved.Library.Single().Files.Count == 6 && saved.Jobs[0].SelectedFileIds!.SequenceEqual([1, 2]), "File metadata or queue selection was not persisted");
            window.Jobs.RemoveAt(1);
            Get<TextBox>(window, "SearchFilesBox").Clear();
            var season = window.BrowserFolders[0].Children[0].Children.Single(folder => folder.Name == "Season 02");
            Get<TreeView>(window, "FolderTree").RaiseEvent(new RoutedPropertyChangedEventArgs<object>(null!, season, TreeView.SelectedItemChangedEvent));
            Check(window.BrowserFiles.Count == 1 && window.BrowserFiles[0].File.Id == 3, "Folder browsing leaked sibling files");
            Click(window, "ShowAllFilesButton");
            Get<CheckBox>(window, "VideoOnlyBox").IsChecked = true;
            Check(window.BrowserFiles.Count == 4, "All torrents should restore the full search scope");
            Get<TextBox>(window, "SearchFilesBox").Text = "nothing-matches";
            Check(window.BrowserFiles.Count == 0 && !Get<Button>(window, "QueueFilesButton").IsEnabled && Get<TextBlock>(window, "BrowserEmpty").Visibility == Visibility.Visible, "Empty search state is not actionable");
            var beforePaste = apiRequests;
            Get<TextBox>(window, "SourcesBox").Text = "magnet:?xt=urn:btih:" + new string('B', 40) + "\nmagnet:?xt=urn:btih:" + new string('C', 40);
            PumpFor(TimeSpan.FromMilliseconds(1100));
            Check(apiRequests == beforePaste && window.Library.Count == 1 && remoteAdds == 1, "Pasting must not upload or inspect until Add is clicked");
            Click(window, "AddButton");
            PumpUntil(() => window.Library.Count == 3 && Get<Button>(window, "AddButton").IsEnabled);
            Check(remoteAdds == 3 && window.Library.Skip(1).All(draft => draft.Files.Count == 6) && Get<TextBox>(window, "SourcesBox").Text.Length == 0, "Manual bulk import did not inspect every pasted source");
            window.Library.RemoveAt(2);
            window.Library.RemoveAt(1);
            Get<TextBox>(window, "SourcesBox").Text = "magnet:?xt=urn:btih:" + new string('A', 40);
            Click(window, "AddButton");
            PumpUntil(() => Get<Button>(window, "AddButton").IsEnabled);
            Check(remoteAdds == 3 && window.Library.Count == 1, "Re-inspecting an existing source created a duplicate");
            Get<TextBox>(window, "SearchFilesBox").Text = "s01";
            Get<TabControl>(window, "WorkspaceTabs").SelectedIndex = 1;
            FlushBindings();
            window.Jobs[0].State = JobState.Completed;
            window.Jobs[0].SetDetail("Đã lưu {0} file · {1}", 2, "C:\\Downloads");
            PrepareScreenshot(window);
            Render(window, "english");
            Render(window, "compact-english", 1124, 696);
            var grid = Get<DataGrid>(window, "QueueGrid");
            grid.SelectedItem = window.Jobs[1];
            Check(Get<Button>(window, "ResumeSelectedButton").IsEnabled && !Get<Button>(window, "StopSelectedButton").IsEnabled, "Selected stopped row should offer Resume");
            grid.SelectedItem = null;
            Get<ComboBox>(window, "ThemeBox").SelectedIndex = 2;
            FlushBindings();
            Check(Appearance.IsDark && store.Load().Appearance == "dark", "Dark appearance was not applied and persisted");
            Check(((SolidColorBrush)window.Background).Color == ((SolidColorBrush)window.FindResource("Surface")).Color, "Flat window background did not update to the surface token");
            Render(window, "english-dark");
            Get<ComboBox>(window, "LanguageBox").SelectedIndex = 0;
            FlushBindings();
            Check(Get<Button>(window, "ConnectButton").Content as string == "Kiểm tra", "Switching back left a stale button");
            Check(window.Jobs[0].Detail.StartsWith("Đã lưu 2 file"), "Switching back left a stale queue detail");
            Get<TextBlock>(window, "StatusText").Text = Text.Get("Sẵn sàng · Token được mã hóa bằng tài khoản Windows khi lưu.");
            Render(window, "vietnamese-dark");
            Get<ComboBox>(window, "ThemeBox").SelectedIndex = 1;
            FlushBindings();
            Check(!Appearance.IsDark && store.Load().Appearance == "light", "Light appearance did not persist");
            Render(window, "vietnamese");
            Render(window, "compact-vietnamese", 1124, 696);
            Get<TabControl>(window, "WorkspaceTabs").SelectedIndex = 0;
            Render(window, "browser-queued-vietnamese");
            Render(window, "browser-queued-compact-vietnamese", 1124, 696);
            Get<ComboBox>(window, "ThemeBox").SelectedIndex = 2;
            Render(window, "browser-queued-vietnamese-dark");
            Get<ComboBox>(window, "LanguageBox").SelectedIndex = 1;
            Check(Get<Button>(window, "QueueFilesButton").Content as string == "Add to queue", "Browser translation is stale");
            Render(window, "browser-queued-english-dark");
            TestVirtualLibrary(directory);
            TestFolderRemoval(directory);
            TestRemoteDeletion(directory);
            TestDeleteActiveTorrent(directory);
            TestImportWhileWaiting(directory);
            TestRangeSelection(directory);
            TestThemedConfirmation(directory);
            ValidateCatalog();
            Console.WriteLine("PASS explicit bulk import, list/grid views, nested search, video filters, selection, queue deduplication and persisted file IDs");
            Console.WriteLine("PASS virtual removal/undo across torrents, queued-file removal, global remaining queue, persisted exclusions and unchanged local files");
            Console.WriteLine("PASS recursive folder removal, filter independence, sibling/torrent isolation, root/context-menu/Delete actions, empty roots, Undo and restart");
            Console.WriteLine("PASS new-batch reset, retained remote history, exact delete review, cancellation, partial failures, account checks and local-file preservation");
            Console.WriteLine("PASS deleting an active torrent waits for the writer to stop and preserves exact partial bytes");
            Console.WriteLine("PASS zero-seed status, editable input while running, manual concurrent import and preservation of newly typed sources");
            Console.WriteLine("PASS shared Shift/Ctrl range selection, pointer toggles, sorted/filtered order, List/Grid anchors and selected-file queueing");
            Console.WriteLine("PASS real light/dark confirmation dialogs, safe Cancel default and clear-queue confirm/cancel behavior");
            Console.WriteLine("PASS WPF language bindings, headers, multiline text, status, saved preference, queue controls and icon");
            Console.WriteLine("PASS light/dark appearances, compact layouts, row selection and translation format placeholders");
            return 0;
        }
        catch (Exception ex) { Console.Error.WriteLine(ex); return 1; }
        finally { window?.Close(); Text.SetLanguage("vi"); Directory.Delete(directory, true); }
    }

    private static T Get<T>(MainWindow window, string name) where T : FrameworkElement => (T)window.FindName(name);
    private static void Check(bool condition, string message) { if (!condition) throw new InvalidOperationException(message); }
    private static void FlushBindings() => Dispatcher.CurrentDispatcher.Invoke(DispatcherPriority.ApplicationIdle, new Action(() => { }));
    private static void Click(MainWindow window, string name) => Get<Button>(window, name).RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
    private static void PumpUntil(Func<bool> ready)
    {
        var timeout = DateTime.UtcNow.AddSeconds(15);
        while (!ready() && DateTime.UtcNow < timeout) { FlushBindings(); Thread.Sleep(10); }
        Check(ready(), "Timed out waiting for UI inspection");
        FlushBindings();
    }
    private static void PumpFor(TimeSpan duration)
    {
        var until = DateTime.UtcNow + duration;
        while (DateTime.UtcNow < until) { FlushBindings(); Thread.Sleep(10); }
    }

    private static void TestVirtualLibrary(string directory)
    {
        var store = new SettingsStore(Path.Combine(directory, "virtual-library"));
        var localFile = Path.Combine(directory, "keep-local.txt");
        File.WriteAllText(localFile, "keep these bytes");
        var first = new TorrentDraft { SourceKey = "first", Name = "Pack", TorrentId = "first-id", Files = [
            new(1, "/Pack/Season/keep.mkv", 374832640, 0), new(2, "/Pack/Extras/trailer.mp4", 32489216, 0),
            new(3, "/Pack/readme.txt", 2416, 0), new(4, "/Pack/Subtitles/readme.srt", 43129, 0)] };
        var second = new TorrentDraft { SourceKey = "second", Name = "Other", TorrentId = "second-id", Files = [
            new(1, "/Other/keep2.mp4", 697321472, 0), new(2, "/Other/readme.txt", 1452, 0)] };
        var existing = new DownloadJob { SourceKey = "first", TorrentId = "already-queued", SelectedFileIds = [1] };
        var settings = new AppSettings { Language = "en", Appearance = "light", Library = [first, second], Jobs = [existing] };
        using var noNetwork = new HttpClient(new FakeApi(_ => throw new InvalidOperationException("Virtual library actions must not call the network")));
        var browser = new MainWindow(store, settings, noNetwork);
        try
        {
            Get<ComboBox>(browser, "BrowserViewBox").SelectedIndex = 1;
            Render(browser, "virtual-grid");
            Get<TextBox>(browser, "SearchFilesBox").Text = "trailer";
            Render(browser, "virtual-grid-filtered");
            var tile = Descendants<System.Windows.Controls.Primitives.ToggleButton>(Get<ItemsControl>(browser, "BrowserTiles")).Single();
            tile.IsChecked = true; FlushBindings();
            Check(browser.BrowserFiles.Single().IsSelected && Get<Button>(browser, "RemoveBrowserFilesButton").IsEnabled, "Grid tile selection did not update shared actions");
            Get<ComboBox>(browser, "BrowserViewBox").SelectedIndex = 0;
            Check(browser.BrowserFiles.Single().IsSelected, "Grid/list switch cleared selection");
            Click(browser, "RemoveBrowserFilesButton");
            Check(first.ExcludedFileIds.SetEquals([2]) && second.ExcludedFileIds.Count == 0 && browser.BrowserFiles.Count == 0, "Removal hid the same file ID in a different torrent");
            Click(browser, "UndoBrowserRemovalButton");
            Check(first.ExcludedFileIds.Count == 0 && browser.BrowserFiles.Count == 1, "Undo did not restore the removed result");
            Get<TextBox>(browser, "SearchFilesBox").Text = "readme";
            Check(browser.BrowserFiles.Count == 3, "Search did not include both torrents");
            Click(browser, "SelectResultsButton"); Click(browser, "RemoveBrowserFilesButton");
            Check(first.ExcludedFileIds.SetEquals([3, 4]) && second.ExcludedFileIds.SetEquals([2]), "Bulk virtual removal missed matching files");
            Check(first.Files.Count == 4 && second.Files.Count == 2 && browser.Jobs.Count == 1, "Virtual removal changed metadata or an existing queue");
            Get<TextBox>(browser, "SearchFilesBox").Text = "trailer";
            Click(browser, "SelectResultsButton"); Click(browser, "RemoveBrowserFilesButton");
            Click(browser, "ShowAllFilesButton");
            Check(browser.BrowserFiles.Count == 2 && browser.BrowserFiles.All(file => file.Name.StartsWith("keep")), "Removed files returned after clearing filters");
            Check(browser.BrowserFolders[0].Children[0].Children.Count == 1, "Empty excluded folders remained in the tree");
            Get<TextBox>(browser, "SearchFilesBox").Text = "keep.mkv";
            Click(browser, "SelectResultsButton");
            Check(!Get<Button>(browser, "QueueFilesButton").IsEnabled && Get<Button>(browser, "RemoveBrowserFilesButton").IsEnabled, "Already queued files should still be removable from the virtual library");
            Click(browser, "RemoveBrowserFilesButton");
            Check(existing.SelectedFileIds!.SequenceEqual([1]) && browser.Jobs.Count == 1, "Virtual removal changed the existing queue selection");
            Click(browser, "UndoBrowserRemovalButton");
            Get<TextBox>(browser, "SearchFilesBox").Text = "no-matches";
            Click(browser, "QueueRemainingButton");
            Check(browser.Jobs.Count == 2 && browser.Jobs[1].SourceKey == "second" && browser.Jobs[1].SelectedFileIds!.SequenceEqual([1]), "Queue remaining included excluded/queued files or was limited by the active search");
            Click(browser, "QueueRemainingButton");
            Check(browser.Jobs.Count == 2, "Queue remaining duplicated jobs");
            Get<ComboBox>(browser, "BrowserViewBox").SelectedIndex = 1;
            var restored = store.Load();
            Check(restored.BrowserView == "grid" && restored.Library.Sum(draft => draft.VisibleFiles.Count()) == 2 && restored.Library[0].ExcludedFileIds.SetEquals([2, 3, 4]), "Restart lost the pruned library or grid preference");
            Check(File.ReadAllText(localFile) == "keep these bytes", "Virtual removal changed a local file");
            using var noNetworkAfterRestart = new HttpClient(new FakeApi(_ => throw new InvalidOperationException("Restoring the virtual library must stay offline")));
            var reopened = new MainWindow(store, restored, noNetworkAfterRestart);
            try
            {
                Check(reopened.BrowserFiles.Count == 2 && Get<ScrollViewer>(reopened, "BrowserTilesScroll").Visibility == Visibility.Visible, "Restart did not restore remaining files and grid view");
            }
            finally { reopened.Close(); }
            Get<TextBox>(browser, "SourcesBox").Text = "pending batch";
            Get<TextBox>(browser, "SearchFilesBox").Text = "keep";
            Get<CheckBox>(browser, "VideoOnlyBox").IsChecked = true;
            Click(browser, "NewBatchButton");
            Check(browser.Library.Count == 0 && browser.BrowserFiles.Count == 0 && browser.Jobs.Count == 2, "New batch did not clear the virtual library or changed the queue");
            Check(Get<TextBox>(browser, "SourcesBox").Text.Length == 0 && Get<TextBox>(browser, "SearchFilesBox").Text.Length == 0 && Get<CheckBox>(browser, "VideoOnlyBox").IsChecked == false, "New batch retained input/search filters");
            Check(!Get<Button>(browser, "UndoBrowserRemovalButton").IsEnabled && Get<Border>(browser, "SourcesPanel").Visibility == Visibility.Visible, "New batch kept stale Undo or hid the input");
            var reset = store.Load();
            Check(reset.Library.Count == 0 && reset.RemoteTorrents.Select(torrent => torrent.Id).ToHashSet().SetEquals(["first-id", "second-id", "already-queued"]), "New batch lost known remote IDs");
            Check(File.ReadAllText(localFile) == "keep these bytes", "New batch touched local files");
        }
        finally { browser.Close(); }
    }

    private static void TestFolderRemoval(string directory)
    {
        var store = new SettingsStore(Path.Combine(directory, "folder-removal"));
        var first = new TorrentDraft { Name = "Pack", SourceKey = "folder-first", TorrentId = "folder-first-id", ExcludedFileIds = [4], Files = [
            new(1, "/Pack/Season/keep.mkv", 3, 0), new(2, "/Pack/Season/Nested/notes.txt", 2, 0),
            new(3, "/Pack/Season 10/sibling.mkv", 5, 0), new(4, "/Pack/preexcluded.txt", 1, 0)] };
        var second = new TorrentDraft { Name = "Other", SourceKey = "folder-second", TorrentId = "folder-second-id", Files = [new(1, "/Pack/Season/keep.mkv", 3, 0)] };
        var pending = new TorrentDraft { Name = "Pending metadata", SourceKey = "pending", TorrentId = "pending-id" };
        var queued = new DownloadJob { SourceKey = first.SourceKey, TorrentId = "queued-id", SelectedFileIds = [1] };
        using var http = new HttpClient(new FakeApi(_ => throw new InvalidOperationException("Virtual folder removal must stay offline")));
        var browser = new MainWindow(store, new() { Language = "en", Appearance = "light", Library = [first, second, pending], Jobs = [queued] }, http);
        try
        {
            Get<TextBox>(browser, "FolderBox").Text = @"C:\Downloads\Real-Debrid";
            var tree = Get<TreeView>(browser, "FolderTree");
            Render(browser, "folder-initial");
            void Select(TorrentFolder folder)
            {
                folder.IsSelected = true;
                tree.RaiseEvent(new RoutedPropertyChangedEventArgs<object>(null!, folder, TreeView.SelectedItemChangedEvent));
                FlushBindings();
            }
            Select(browser.BrowserFolders[0].Children.Single(folder => folder.Torrent == first).Children.Single(folder => folder.Name == "Season"));
            Get<TextBox>(browser, "SearchFilesBox").Text = "keep";
            Get<CheckBox>(browser, "VideoOnlyBox").IsChecked = true;
            Get<ComboBox>(browser, "BrowserViewBox").SelectedIndex = 1;
            Check(browser.BrowserFiles.Count == 1 && !browser.BrowserFiles[0].IsSelected && Get<Button>(browser, "RemoveBrowserFolderButton").IsEnabled, "A selected folder must be removable without ticking files");
            Render(browser, "folder-selected");
            Click(browser, "RemoveBrowserFolderButton");
            Check(first.ExcludedFileIds.SetEquals([1, 2, 4]) && second.VisibleFiles.Count() == 1 && first.VisibleFiles.Single().Id == 3, "Folder removal missed filtered/nested files or crossed a sibling/torrent boundary: " + string.Join(",", first.ExcludedFileIds));
            Check(browser.Jobs.Single() == queued && queued.SelectedFileIds!.SequenceEqual([1]), "Folder removal changed the queue");
            Check(browser.BrowserFolders[0].Children.Single(folder => folder.Torrent == first).Children.All(folder => folder.Name != "Season"), "Removed folder remained in the tree");
            Check(store.Load().Library[0].ExcludedFileIds.SetEquals([1, 2, 4]), "Recursive folder removal was not saved");
            Click(browser, "UndoBrowserRemovalButton");
            Check(first.ExcludedFileIds.SetEquals([4]) && first.VisibleFiles.Count() == 3, "Folder Undo resurrected previously excluded files or lost descendants");
            Click(browser, "ShowAllFilesButton");
            Render(browser, "folder-restored");
            var allItem = (TreeViewItem)tree.ItemContainerGenerator.ContainerFromItem(browser.BrowserFolders[0]);
            var torrentItem = (TreeViewItem)allItem.ItemContainerGenerator.ContainerFromItem(browser.BrowserFolders[0].Children.Single(folder => folder.Torrent == first));
            var menu = torrentItem.ContextMenu;
            menu.PlacementTarget = torrentItem; FlushBindings();
            var removeItem = (MenuItem)menu.Items[0];
            Check(removeItem.DataContext is TorrentFolder { Torrent: var target } && target == first, "Folder context menu targeted the wrong source");
            Check(ReferenceEquals(removeItem.CommandTarget, torrentItem) && ((System.Windows.Input.RoutedCommand)removeItem.Command).CanExecute(removeItem.CommandParameter, removeItem.CommandTarget), "Folder menu command was not routed to its tree item");
            ((System.Windows.Input.RoutedCommand)removeItem.Command).Execute(removeItem.CommandParameter, removeItem.CommandTarget);
            Check(first.IsHidden && !first.VisibleFiles.Any() && browser.BrowserFolders[0].Children.All(folder => folder.Torrent != first), "Deleting the torrent root left an empty folder");
            Check(store.Load().RemoteTorrents.Any(item => item.Id == "folder-first-id") && browser.Jobs.Count == 1, "Root removal lost remote tracking or changed the queue");
            Check(store.Load().Library[0].IsHidden, "Hidden torrent root returned in saved state");
            Click(browser, "UndoBrowserRemovalButton");
            Check(!first.IsHidden && first.ExcludedFileIds.SetEquals([4]), "Root Undo lost the prior curation");
            Select(browser.BrowserFolders[0].Children.Single(folder => folder.Torrent == pending));
            var hwnd = new System.Windows.Interop.WindowInteropHelper(browser).EnsureHandle();
            var key = new System.Windows.Input.KeyEventArgs(System.Windows.Input.Keyboard.PrimaryDevice,
                System.Windows.Interop.HwndSource.FromHwnd(hwnd), 0, System.Windows.Input.Key.Delete) { RoutedEvent = System.Windows.Input.Keyboard.PreviewKeyDownEvent };
            tree.RaiseEvent(key);
            Check(pending.IsHidden && browser.BrowserFolders[0].Children.All(folder => folder.Torrent != pending), "Delete did not remove an empty metadata root");
            Click(browser, "ShowAllFilesButton");
            Check(!Get<Button>(browser, "RemoveBrowserFolderButton").IsEnabled, "All torrents container should not be removable as a folder");
            Click(browser, "SelectResultsButton"); Click(browser, "RemoveBrowserFilesButton");
            Check(browser.BrowserFolders[0].Children.Count == 0, "Deleting all files left empty torrent roots");
            var saved = store.Load();
            using var offline = new HttpClient(new FakeApi(_ => throw new Exception("Restart should stay offline")));
            var reopened = new MainWindow(store, saved, offline);
            try { Check(reopened.BrowserFolders[0].Children.Count == 0 && reopened.BrowserFiles.Count == 0, "Empty/hidden folders returned after restart"); }
            finally { reopened.Close(); }
        }
        finally { browser.Close(); }
    }

    private static object RemoteItem(string id, string name, string status = "downloaded", double progress = 100)
        => new { id, filename = name, hash = "hash-" + id, bytes = 734832640L, host = "real-debrid.com", split = 2000, progress, status, added = "2026-10-30T08:15:00.000Z", links = Array.Empty<string>() };
    private static HttpResponseMessage Json(object value) => new(HttpStatusCode.OK) { Content = new StringContent(JsonSerializer.Serialize(value)) };

    private static void TestRemoteDeletion(string directory)
    {
        var store = new SettingsStore(Path.Combine(directory, "remote-deletion"));
        var local = Path.Combine(directory, "keep-download.part");
        File.WriteAllText(local, "preserve partial bytes");
        var draft = new TorrentDraft { SourceKey = "same-source", Name = "Selected torrent", TorrentId = "good", RemoteAccount = "account", Files = [new(1, "/video.mkv", 3, 0)] };
        var good = new DownloadJob { Name = "Selected videos", SourceKey = "same-source", TorrentId = "good", State = JobState.Paused, RemoteAccount = "account" };
        var failed = new DownloadJob { Name = "Retry later", SourceKey = "failed-source", TorrentId = "failed", State = JobState.Paused, RemoteAccount = "account" };
        var keep = new DownloadJob { Name = "Keep running", SourceKey = "same-source", TorrentId = "keep", State = JobState.Paused, RemoteAccount = "account" };
        var other = new DownloadJob { Name = "Other account", SourceKey = "other", TorrentId = "other-account", State = JobState.Paused, RemoteAccount = "someone-else" };
        // A recorded ID that is no longer on the account must not be listed.
        var refs = new List<RemoteTorrentReference> { new() { Id = "stale", Name = "Deleted on the website", SourceKey = "stale", Account = "account" } };
        var calls = new List<string>();
        var username = "account";
        var emptyAccount = false;
        var settings = new AppSettings { Language = "en", Appearance = "light", Library = [draft], Jobs = [good, failed, keep, other], RemoteTorrents = refs };
        using var http = new HttpClient(new FakeApi(request =>
        {
            var path = request.RequestUri!.AbsolutePath;
            calls.Add(request.Method + " " + path);
            if (path.EndsWith("/user")) return Json(new { username, type = "premium", premium = 3600 });
            if (path.EndsWith("/torrents"))
            {
                Check(request.RequestUri.Query.Contains("limit="), "Torrent list request is not paged");
                return emptyAccount ? new(HttpStatusCode.NoContent) : Json(new[] { RemoteItem("good", "Selected videos"), RemoteItem("failed", "Retry later", "downloading", 41.6),
                    RemoteItem("gone", "Gone torrent", "dead", 0), RemoteItem("keep", "Keep running"), RemoteItem("web-only", "Added on the website", "waiting_files_selection", 0) });
            }
            Check(request.Method == HttpMethod.Delete, "Remote deletion sent an unexpected mutation");
            var id = request.RequestUri.Segments.Last();
            Check(id is "good" or "failed" or "gone" or "web-only", "Deletion expanded beyond the reviewed IDs");
            if (id == "good") Check(!good.IsActive && good.State == JobState.Stopped, "DELETE ran before stopping the linked task");
            if (id == "failed") Check(failed.State == JobState.Stopped, "Failed DELETE did not stop the linked task first");
            return new(id == "failed" ? HttpStatusCode.InternalServerError : id == "gone" ? HttpStatusCode.NotFound : HttpStatusCode.NoContent);
        }));
        var browser = new MainWindow(store, settings, http);
        try
        {
            Get<TabControl>(browser, "WorkspaceTabs").SelectedIndex = 2;
            Check(calls.Count == 0 && browser.RemoteTorrents.Count == 0 && Get<StackPanel>(browser, "RemoteEmpty").Visibility == Visibility.Visible, "Remote list loaded without a token");
            Get<PasswordBox>(browser, "TokenBox").Password = "test-token";
            Get<TextBox>(browser, "FolderBox").Text = @"C:\Downloads\Real-Debrid";
            Click(browser, "RemoteRefreshButton");
            PumpUntil(() => browser.RemoteTorrents.Count == 5 && Get<Button>(browser, "RemoteRefreshButton").IsEnabled);
            Check(browser.RemoteTorrents.Select(row => row.Id).ToHashSet().SetEquals(["good", "failed", "gone", "keep", "web-only"]), "Remote list does not match the account");
            Check(browser.RemoteTorrents.Single(row => row.Id == "good").IsQueued && !browser.RemoteTorrents.Single(row => row.Id == "web-only").IsQueued, "Queue links are not marked");
            Check(browser.RemoteTorrents.Single(row => row.Id == "failed").Status == "Downloading 42%" && Get<TextBlock>(browser, "RemoteSummary").Text.Contains("account"), "Remote status or account summary missing");
            Check(calls.All(call => call.StartsWith("GET")), "Loading the list changed the account");
            Render(browser, "remote-list");
            Get<ComboBox>(browser, "ThemeBox").SelectedIndex = 2;
            Render(browser, "remote-list-dark");
            Get<ComboBox>(browser, "ThemeBox").SelectedIndex = 1;
            Click(browser, "RemoteSelectAllButton");
            Check(browser.RemoteTorrents.All(row => row.IsSelected), "Select all omitted a listed torrent");
            browser.RemoteTorrents.Single(row => row.Id == "keep").IsSelected = false;
            Click(browser, "RemoteReviewButton");
            Check(!calls.Any(call => call.StartsWith("DELETE")) && ((IEnumerable<RemoteTorrentRow>)Get<DataGrid>(browser, "RemoteGrid").ItemsSource).Count() == 4, "Review should show exact selected IDs without network changes");
            Check(!Get<Button>(browser, "RemoteRefreshButton").IsEnabled, "Reload must stay disabled during review");
            Render(browser, "remote-review");
            Render(browser, "remote-review-compact", 1124, 696);
            Get<ComboBox>(browser, "LanguageBox").SelectedIndex = 0;
            Render(browser, "remote-review-vietnamese");
            Render(browser, "remote-review-compact-vietnamese", 1124, 696);
            Get<ComboBox>(browser, "ThemeBox").SelectedIndex = 2;
            Render(browser, "remote-review-vietnamese-dark");
            Get<ComboBox>(browser, "LanguageBox").SelectedIndex = 1;
            Get<ComboBox>(browser, "ThemeBox").SelectedIndex = 1;
            Click(browser, "RemoteCancelButton");
            Check(!calls.Any(call => call.StartsWith("DELETE")) && browser.Jobs.Count == 4, "Canceling review changed remote torrents or the queue");
            Click(browser, "RemoteReviewButton");
            browser.RemoteTorrents.Single(row => row.Id == "keep").IsSelected = true; // Changes after review cannot expand the frozen scope.
            Click(browser, "RemoteConfirmButton");
            PumpUntil(() => Get<Button>(browser, "RemoteConfirmButton").Visibility == Visibility.Collapsed);
            Check(calls.Count(call => call.StartsWith("DELETE")) == 4 && calls.Count(call => call.EndsWith("/failed")) == 1, "Deletion replayed a failed DELETE or missed reviewed targets");
            Check(browser.RemoteTorrents.Select(row => row.Id).ToHashSet().SetEquals(["failed", "keep"]), "Deleted or absent rows stayed in the list");
            Check(browser.RemoteTorrents.Single(row => row.Id == "failed").Tone == "error", "Failed deletion is not visible on its row");
            Check(browser.Jobs.Count == 3 && !browser.Jobs.Contains(good) && browser.Jobs.Contains(keep) && other.State == JobState.Paused && failed.State == JobState.Stopped, "Deletion changed unrelated jobs or discarded a failed job");
            Check(draft.TorrentId is null && draft.Files.Count == 1, "Deleted remote ID was retained or virtual metadata was discarded");
            var remaining = store.Load();
            Check(!remaining.RemoteTorrents.Any(item => item.Id is "good" or "gone" or "web-only"), "Deleted remote IDs returned after saving");
            Check(File.ReadAllText(local) == "preserve partial bytes", "Remote deletion touched a local file");
            Render(browser, "remote-results");
            // The token now answers for another account: refuse before any DELETE.
            username = "someone-else";
            var deletes = calls.Count(call => call.StartsWith("DELETE"));
            browser.RemoteTorrents.Single(row => row.Id == "failed").IsSelected = true;
            Click(browser, "RemoteReviewButton"); Click(browser, "RemoteConfirmButton");
            PumpUntil(() => Get<Button>(browser, "RemoteConfirmButton").Visibility == Visibility.Collapsed);
            Check(calls.Count(call => call.StartsWith("DELETE")) == deletes && browser.RemoteTorrents.Single(row => row.Id == "failed").Status.Contains("someone-else"), "Deletion crossed accounts");
            Get<PasswordBox>(browser, "TokenBox").Password = "another-token";
            Check(browser.RemoteTorrents.Count == 0 && Get<TextBlock>(browser, "RemoteSummary").Text == "List not loaded", "Changing the token kept another account's list");
            emptyAccount = true;
            Click(browser, "RemoteRefreshButton");
            PumpUntil(() => Get<Button>(browser, "RemoteRefreshButton").IsEnabled && Get<TextBlock>(browser, "RemoteSummary").Text != "List not loaded");
            Check(browser.RemoteTorrents.Count == 0 && Get<TextBlock>(browser, "RemoteEmptyText").Text.Contains("no torrents"), "An empty account response was not handled");
        }
        finally { browser.Close(); }
    }

    private static void TestDeleteActiveTorrent(string directory)
    {
        var output = Path.Combine(directory, "active-deletion", "downloads");
        var job = new DownloadJob { Name = "Active test", SourceKey = "active", TorrentId = "active", RemoteAccount = "account", SelectedFileIds = [1] };
        var blocked = new BlockingTransferStream();
        var deleted = false;
        using var api = new HttpClient(new FakeApi(request =>
        {
            var path = request.RequestUri!.AbsolutePath;
            if (request.Method == HttpMethod.Delete)
            {
                Check(path.EndsWith("/delete/active") && !job.IsActive && blocked.Canceled, "Remote DELETE ran while a file writer was active");
                deleted = true; return new(HttpStatusCode.NoContent);
            }
            object result = path.EndsWith("/user") ? new { username = "account", type = "premium", premium = 3600 } :
                path.EndsWith("/torrents") ? new[] { RemoteItem("active", "Active test", "downloaded") } :
                path.Contains("/info/") ? new { id = "active", filename = "Active test", status = "downloaded", progress = 100, bytes = 6, files = new[] { new TorrentFile(1, "/file.bin", 6, 1) }, links = new[] { "https://rd.test/file" } } :
                path.EndsWith("/unrestrict/link") ? new { filename = "file.bin", filesize = 6, download = "https://files.test/file" } : throw new Exception("Unexpected API request");
            return new(HttpStatusCode.OK) { Content = new StringContent(JsonSerializer.Serialize(result)) };
        }));
        using var transfers = new HttpClient(new FakeApi(_ =>
        {
            var response = new HttpResponseMessage(HttpStatusCode.OK) { Content = new StreamContent(blocked) };
            response.Content.Headers.ContentLength = 6;
            return response;
        }));
        var browser = new MainWindow(new SettingsStore(Path.Combine(directory, "active-deletion", "settings")),
            new() { Language = "en", OutputDirectory = output, Jobs = [job] }, api, transfers);
        try
        {
            Get<PasswordBox>(browser, "TokenBox").Password = "test-token";
            Click(browser, "StartButton");
            PumpUntil(() => blocked.Blocked && job.IsActive);
            Get<TabControl>(browser, "WorkspaceTabs").SelectedIndex = 2;
            PumpUntil(() => browser.RemoteTorrents.Count == 1 && Get<Button>(browser, "RemoteRefreshButton").IsEnabled);
            browser.RemoteTorrents.Single(row => row.Id == "active").IsSelected = true;
            Click(browser, "RemoteReviewButton"); Click(browser, "RemoteConfirmButton");
            PumpUntil(() => Get<Button>(browser, "RemoteConfirmButton").Visibility == Visibility.Collapsed);
            Check(deleted && browser.Jobs.Count == 0 && !job.IsActive, "Active deletion did not remove the stopped queue entry");
            var partial = Directory.GetFiles(output, "*.part", SearchOption.AllDirectories).Single();
            Check(File.ReadAllBytes(partial).SequenceEqual(new byte[] { 1, 2, 3 }), "Remote deletion changed partial bytes");
        }
        finally { browser.Close(); }
    }

    private static void TestImportWhileWaiting(string directory)
    {
        var added = 0;
        var waiting = new DownloadJob { Name = "Waiting for sources", TorrentId = "waiting", SourceKey = "waiting", SelectedFileIds = [1] };
        using var api = new HttpClient(new FakeApi(request =>
        {
            var path = request.RequestUri!.AbsolutePath;
            object response = path.EndsWith("/user") ? new { username = "account", type = "premium", premium = 3600 } :
                path.EndsWith("/torrents") ? Array.Empty<object>() :
                path.EndsWith("/addMagnet") ? new { id = "imported-" + ++added } :
                path.EndsWith("/info/waiting") ? new { id = "waiting", filename = "Waiting for sources", status = "downloading", progress = 0, bytes = 3, seeders = 0, speed = 0,
                    files = new[] { new TorrentFile(1, "/waiting.mkv", 3, 1) }, links = Array.Empty<string>() } :
                path.Contains("/info/imported-") ? new { id = request.RequestUri.Segments.Last(), filename = "Added torrent", status = "waiting_files_selection", progress = 0, bytes = 3,
                    files = new[] { new TorrentFile(1, "/new.mp4", 3, 0) }, links = Array.Empty<string>() } : throw new Exception("Unexpected request while waiting: " + path);
            return new(HttpStatusCode.OK) { Content = new StringContent(JsonSerializer.Serialize(response)) };
        }));
        using var transfers = new HttpClient(new FakeApi(_ => throw new Exception("No local transfer should start without download links")));
        var browser = new MainWindow(new SettingsStore(Path.Combine(directory, "concurrent-import", "settings")),
            new() { Language = "en", OutputDirectory = Path.Combine(directory, "concurrent-import", "downloads"), Jobs = [waiting] }, api, transfers);
        try
        {
            Get<PasswordBox>(browser, "TokenBox").Password = "test-token";
            Click(browser, "StartButton");
            PumpUntil(() => waiting.IsActive && waiting.State == JobState.Waiting && waiting.Detail.Contains("0 seed"));
            Check(Get<TextBox>(browser, "SourcesBox").IsEnabled && Get<Button>(browser, "AddButton").IsEnabled && Get<Button>(browser, "AddFilesButton").IsEnabled, "Running queue locked source input/import");
            var first = "magnet:?xt=urn:btih:" + new string('D', 40);
            var next = "magnet:?xt=urn:btih:" + new string('E', 40);
            Get<TextBox>(browser, "SourcesBox").Text = first;
            Click(browser, "AddButton");
            Check(Get<TextBox>(browser, "SourcesBox").IsEnabled && Get<Button>(browser, "PasteButton").IsEnabled, "Metadata inspection locked the next source input");
            Get<TextBox>(browser, "SourcesBox").Text += Environment.NewLine + next;
            PumpUntil(() => browser.Library.Count == 1 && Get<Button>(browser, "AddButton").IsEnabled);
            Check(Get<TextBox>(browser, "SourcesBox").Text == next && added == 1, "Completing an import erased or automatically submitted newly typed sources");
            Check(waiting.IsActive && waiting.TorrentId == "waiting" && waiting.Files.Count == 0, "Import interrupted or re-added the waiting torrent");
            Click(browser, "AddButton");
            PumpUntil(() => browser.Library.Count == 2 && Get<Button>(browser, "AddButton").IsEnabled);
            Check(added == 2 && Get<TextBox>(browser, "SourcesBox").Text.Length == 0, "Could not manually import the next source during a running queue");
            foreach (var tab in new[] { 0, 1, 2 })
            {
                Get<TabControl>(browser, "WorkspaceTabs").SelectedIndex = tab;
                Check(Get<Border>(browser, "SourcesPanel").Visibility == Visibility.Visible && Get<TextBox>(browser, "SourcesBox").IsEnabled, "Changing tabs hid or disabled the persistent input");
            }
            Click(browser, "StopAllButton");
            PumpUntil(() => !waiting.IsActive && Get<StackPanel>(browser, "AccountPanel").IsEnabled);
        }
        finally { browser.Close(); }
    }

    private static void TestRangeSelection(string directory)
    {
        var bytes = new[] { 30, 10, 60, 20, 50, 40 };
        var draft = new TorrentDraft { Name = "Selection sample", SourceKey = "selection", TorrentId = "r1",
            Files = Enumerable.Range(1, 6).Select(id => new TorrentFile(id, $"/Selection sample/{id:00}.mkv", bytes[id - 1], 0)).ToList() };
        using var http = new HttpClient(new FakeApi(request => request.RequestUri!.AbsolutePath switch
        {
            var path when path.EndsWith("/user") => Json(new { username = "account", type = "premium", premium = 3600 }),
            var path when path.EndsWith("/torrents") => Json(Enumerable.Range(1, 6).Select(id => RemoteItem("r" + id, "Torrent " + id)).ToArray()),
            _ => throw new Exception("Selection must not change the account")
        }));
        var browser = new MainWindow(new SettingsStore(Path.Combine(directory, "range-selection")),
            new() { Language = "en", Appearance = "light", Library = [draft] }, http);
        try
        {
            Render(browser, "range-list-initial");
            var rows = browser.BrowserFiles.ToArray();
            void CheckFiles(params int[] ids)
            {
                FlushBindings();
                Check(browser.BrowserFiles.Where(file => file.IsSelected).Select(file => file.File.Id).ToHashSet().SetEquals(ids), "Range selection differs: " + string.Join(",", browser.BrowserFiles.Where(file => file.IsSelected).Select(file => file.File.Id)));
            }
            var grid = Get<DataGrid>(browser, "BrowserGrid");
            var firstRow = (DataGridRow)grid.ItemContainerGenerator.ContainerFromItem(rows[0]);
            var checkbox = Descendants<CheckBox>(firstRow).Single();
            checkbox.RaiseEvent(new MouseButtonEventArgs(Mouse.PrimaryDevice, 0, MouseButton.Left) { RoutedEvent = Mouse.PreviewMouseDownEvent });
            CheckFiles(1);
            browser.SelectBrowserItem(rows[3], ModifierKeys.Shift, toggle: true); CheckFiles(1, 2, 3, 4);
            Check(grid.SelectedItems.Count == 4, "List row highlighting did not follow range selection");
            browser.SelectBrowserItem(rows[1], ModifierKeys.Shift); CheckFiles(1, 2);
            browser.SelectBrowserItem(rows[5], ModifierKeys.Control); CheckFiles(1, 2, 6);
            browser.SelectBrowserItem(rows[3], ModifierKeys.Control | ModifierKeys.Shift); CheckFiles(1, 2, 4, 5, 6);
            Get<ComboBox>(browser, "BrowserViewBox").SelectedIndex = 1;
            browser.SelectBrowserItem(rows[4], ModifierKeys.Shift, toggle: true); CheckFiles(5, 6);
            Render(browser, "range-grid");
            Get<ComboBox>(browser, "BrowserViewBox").SelectedIndex = 0;
            grid.Items.SortDescriptions.Add(new SortDescription(nameof(BrowserFile.SizeLabel), ListSortDirection.Ascending));
            var sorted = grid.Items.OfType<BrowserFile>().ToArray();
            browser.SelectBrowserItem(sorted[0]); browser.SelectBrowserItem(sorted[2], ModifierKeys.Shift); CheckFiles(1, 2, 4);
            Get<TextBox>(browser, "SearchFilesBox").Text = "03";
            browser.SelectBrowserItem(browser.BrowserFiles.Single(), ModifierKeys.Shift); CheckFiles(3);
            Get<ComboBox>(browser, "BrowserViewBox").SelectedIndex = 1;
            Render(browser, "range-filtered-grid");
            var tile = Descendants<System.Windows.Controls.Primitives.ToggleButton>(Get<ItemsControl>(browser, "BrowserTiles")).Single();
            tile.RaiseEvent(new MouseButtonEventArgs(Mouse.PrimaryDevice, 0, MouseButton.Left) { RoutedEvent = Mouse.PreviewMouseDownEvent });
            CheckFiles();
            Get<TextBox>(browser, "SearchFilesBox").Clear();
            var visible = Get<ItemsControl>(browser, "BrowserTiles").Items.OfType<BrowserFile>().ToArray();
            browser.SelectBrowserItem(visible[0]); browser.SelectBrowserItem(visible[2], ModifierKeys.Shift, toggle: true);
            var queuedIds = visible.Take(3).Select(file => file.File.Id).ToHashSet();
            Click(browser, "QueueFilesButton");
            Check(browser.Jobs.Single().SelectedFileIds!.ToHashSet().SetEquals(queuedIds), "Queue ignored range selection");
            Get<PasswordBox>(browser, "TokenBox").Password = "test-token";
            Get<TabControl>(browser, "WorkspaceTabs").SelectedIndex = 2;
            PumpUntil(() => browser.RemoteTorrents.Count == 6 && Get<Button>(browser, "RemoteRefreshButton").IsEnabled);
            var remotes = browser.RemoteTorrents.OrderBy(row => row.Id).ToArray();
            browser.SelectRemoteItem(remotes[1], toggle: true);
            browser.SelectRemoteItem(remotes[4], ModifierKeys.Shift, toggle: true);
            FlushBindings();
            Check(browser.RemoteTorrents.Where(row => row.IsSelected).Select(row => row.Id).ToHashSet().SetEquals(["r2", "r3", "r4", "r5"]), "Remote torrent range selection missed rows");
            browser.SelectRemoteItem(remotes[0], ModifierKeys.Control);
            browser.SelectRemoteItem(remotes[2], ModifierKeys.Control | ModifierKeys.Shift);
            Check(browser.RemoteTorrents.Count(row => row.IsSelected) == 5, "Ctrl+Shift discarded existing remote selections");
        }
        finally { browser.Close(); }
    }

    private static void TestThemedConfirmation(string directory)
    {
        foreach (var mode in new[] { "light", "dark" })
        {
            using var http = new HttpClient(new FakeApi(_ => throw new Exception("Dialog checks must stay offline")));
            var browser = new MainWindow(new SettingsStore(Path.Combine(directory, "dialog-" + mode)),
                new() { Language = "en", Appearance = mode, Jobs = [new() { Name = "Keep until confirmed" }] }, http);
            try
            {
                _ = new System.Windows.Interop.WindowInteropHelper(browser).EnsureHandle();
                ThemedDialog? captured = null;
                void Schedule(bool accept, string artifact)
                {
                    Dispatcher.CurrentDispatcher.BeginInvoke(DispatcherPriority.ContextIdle, new Action(() =>
                    {
                        captured = Application.Current.Windows.OfType<ThemedDialog>().Single(dialog => dialog.IsVisible);
                        RenderDialog(captured, artifact);
                        ((Button)captured.FindName(accept ? "ConfirmButton" : "CancelButton")).RaiseEvent(new RoutedEventArgs(Button.ClickEvent));
                    }));
                }
                Schedule(false, "confirmation-" + mode);
                Click(browser, "ClearQueueButton");
                Check(captured is not null && browser.Jobs.Count == 1, "Cancel changed the queue or failed to show the themed dialog");
                Check(((SolidColorBrush)captured!.Background).Color == ((SolidColorBrush)browser.FindResource("Surface")).Color, "Confirmation background ignored the active theme");
                Check(((Button)captured.FindName("CancelButton")).IsDefault && !((Button)captured.FindName("ConfirmButton")).IsDefault, "Destructive confirmation should default to Cancel");
                Schedule(true, "confirmation-accepted-" + mode);
                Click(browser, "ClearQueueButton");
                PumpUntil(() => browser.Jobs.Count == 0);
                Check(captured!.DialogResult == true, "Confirmation did not accept the explicit action");
                var information = new ThemedDialog("Connection failed", "Example message");
                try
                {
                    Check(((Button)information.FindName("CancelButton")).Visibility == Visibility.Collapsed && ((Button)information.FindName("ConfirmButton")).IsDefault, "Information dialog has incorrect buttons");
                    RenderDialog(information, "information-" + mode);
                }
                finally { information.Close(); }
            }
            finally { browser.Close(); }
        }
    }

    private static void RenderDialog(ThemedDialog dialog, string name)
    {
        FlushBindings();
        var root = (FrameworkElement)dialog.Content;
        const int width = 540;
        root.Measure(new Size(width, double.PositiveInfinity));
        var height = (int)Math.Ceiling(root.DesiredSize.Height);
        root.Arrange(new Rect(0, 0, width, height)); root.UpdateLayout(); FlushBindings();
        var bitmap = new RenderTargetBitmap(width, height, 96, 96, PixelFormats.Pbgra32);
        var background = new DrawingVisual();
        using (var drawing = background.RenderOpen()) drawing.DrawRectangle(dialog.Background, null, new Rect(0, 0, width, height));
        bitmap.Render(background); bitmap.Render(root);
        var encoder = new PngBitmapEncoder(); encoder.Frames.Add(BitmapFrame.Create(bitmap));
        var path = Path.GetFullPath(Path.Combine("artifacts", "ui-checks", name + ".png"));
        using var stream = File.Create(path); encoder.Save(stream);
    }

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

    private static void Render(MainWindow window, string language, int width = 1384, int height = 900)
    {
        FlushBindings();
        var root = (FrameworkElement)window.Content;
        root.InvalidateMeasure(); root.InvalidateArrange();
        root.Measure(new Size(width, height));
        root.Arrange(new Rect(0, 0, width, height));
        root.UpdateLayout();
        FlushBindings();
        var names = Get<TabControl>(window, "WorkspaceTabs").SelectedIndex switch
        {
            0 => new[] { "SourcesPanel", "SourcesBox", "AddButton", "QueueFilesButton", "QueueRemainingButton", "RemoveBrowserFilesButton", "RemoveBrowserFolderButton", "UndoBrowserRemovalButton", "NewBatchButton", "BrowserViewBox", "SelectResultsButton", "SearchFilesBox", "LanguageBox", "ThemeBox" },
            1 => new[] { "SourcesPanel", "SourcesBox", "AddButton", "StartButton", "ClearQueueButton", "LanguageBox", "ThemeBox" },
            _ => new[] { "RemoteGrid", "RemoteSummary", "RemoteCancelButton", "RemoteConfirmButton", "LanguageBox", "ThemeBox" }
        };
        foreach (var name in names)
        {
            var control = (FrameworkElement)window.FindName(name);
            var bounds = control.TransformToAncestor(root).TransformBounds(new Rect(control.RenderSize));
            Check(bounds.Left >= -1 && bounds.Top >= -1 && bounds.Right <= width + 1 && bounds.Bottom <= height + 1, $"{language}: {name} is outside the visible layout: {bounds}");
        }
        if (width >= 1384 && Get<TabControl>(window, "WorkspaceTabs").SelectedIndex == 1)
        {
            var grid = Get<DataGrid>(window, "QueueGrid");
            foreach (var button in Descendants<Button>(grid).Where(button => button.Visibility == Visibility.Visible && button.ActualWidth > 0))
            {
                var bounds = button.TransformToAncestor(grid).TransformBounds(new Rect(button.RenderSize));
                Check(bounds.Left >= 0 && bounds.Right < grid.ActualWidth, $"{language}: row action is clipped: {button.Content}");
            }
        }
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

    private static IEnumerable<T> Descendants<T>(DependencyObject parent) where T : DependencyObject
    {
        for (var index = 0; index < VisualTreeHelper.GetChildrenCount(parent); index++)
        {
            var child = VisualTreeHelper.GetChild(parent, index);
            if (child is T match) yield return match;
            foreach (var descendant in Descendants<T>(child)) yield return descendant;
        }
    }
}

internal sealed class FakeApi(Func<HttpRequestMessage, HttpResponseMessage> respond) : HttpMessageHandler
{
    protected override Task<HttpResponseMessage> SendAsync(HttpRequestMessage request, CancellationToken cancellationToken)
        => Task.FromResult(respond(request));
}

internal sealed class BlockingTransferStream : MemoryStream
{
    private int _reads;
    public bool Blocked { get; private set; }
    public bool Canceled { get; private set; }
    public override bool CanSeek => false;
    public override async ValueTask<int> ReadAsync(Memory<byte> buffer, CancellationToken cancellationToken = default)
    {
        if (_reads++ == 0) { new byte[] { 1, 2, 3 }.CopyTo(buffer); return 3; }
        Blocked = true;
        try { await Task.Delay(Timeout.Infinite, cancellationToken); }
        catch (OperationCanceledException) { Canceled = true; throw; }
        return 0;
    }
}
