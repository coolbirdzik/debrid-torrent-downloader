using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Input;
using System.Windows.Media;

namespace RdBatch.App;

public partial class MainWindow
{
    private readonly RangeSelection<BrowserFile> _fileSelection = new(file => file.IsSelected, (file, value) => file.IsSelected = value);
    private readonly RangeSelection<RemoteTorrentRow> _remoteSelection = new(row => row.IsSelected, (row, value) => row.IsSelected = value);

    public void SelectBrowserItem(BrowserFile file, ModifierKeys modifiers = ModifierKeys.None, bool toggle = false)
    {
        if (_closing || _clearing) return;
        var order = (BrowserViewBox.SelectedIndex == 1 ? BrowserTiles.Items : BrowserGrid.Items).OfType<BrowserFile>().ToArray();
        _refreshingBrowser = true;
        try { _fileSelection.Apply(order, file, modifiers.HasFlag(ModifierKeys.Shift), modifiers.HasFlag(ModifierKeys.Control), toggle); }
        finally { _refreshingBrowser = false; }
        UpdateBrowserSelection();
    }

    public void SelectRemoteItem(RemoteTorrentRow row, ModifierKeys modifiers = ModifierKeys.None, bool toggle = false)
    {
        if (_closing || _clearing || _remoteReviewTargets is not null || !row.CanDelete) return;
        _selectingRemoteTorrents = true;
        try { _remoteSelection.Apply(RemoteGrid.Items.OfType<RemoteTorrentRow>().ToArray(), row, modifiers.HasFlag(ModifierKeys.Shift), modifiers.HasFlag(ModifierKeys.Control), toggle); }
        finally { _selectingRemoteTorrents = false; }
        UpdateRemoteControls();
    }

    private void BrowserPointerDown(object sender, MouseButtonEventArgs e)
    {
        if (e.ChangedButton != MouseButton.Left) return;
        var file = PointerItem<BrowserFile>(e.OriginalSource as DependencyObject, out var toggle, out var focus);
        if (file is null) return;
        focus?.Focus();
        SelectBrowserItem(file, Keyboard.Modifiers, toggle);
        e.Handled = true; // Prevent the native checkbox/tile from toggling a second time.
    }

    private void RemotePointerDown(object sender, MouseButtonEventArgs e)
    {
        if (e.ChangedButton != MouseButton.Left) return;
        var row = PointerItem<RemoteTorrentRow>(e.OriginalSource as DependencyObject, out var toggle, out var focus);
        if (row is null) return;
        focus?.Focus();
        SelectRemoteItem(row, Keyboard.Modifiers, toggle);
        e.Handled = true;
    }

    private static T? PointerItem<T>(DependencyObject? element, out bool toggle, out UIElement? focus) where T : class
    {
        T? item = null;
        toggle = false; focus = null;
        while (element is not null)
        {
            if (element is FrameworkElement { DataContext: T data }) item ??= data;
            if (element is ToggleButton button) { toggle = true; focus = button; }
            if (element is DataGridRow row) focus ??= row;
            if (element is DataGrid or ItemsControl) break;
            element = element is FrameworkContentElement content ? content.Parent : VisualTreeHelper.GetParent(element);
        }
        return item;
    }
}
