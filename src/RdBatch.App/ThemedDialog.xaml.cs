using System.Windows;
using RdBatch.Core;

namespace RdBatch.App;

public partial class ThemedDialog : Window
{
    public ThemedDialog(string title, string message, string? confirmLabel = null)
    {
        InitializeComponent();
        Title = title; Heading.Text = title; MessageText.Text = message;
        ConfirmButton.Content = confirmLabel ?? Text.Get("Đóng");
        var confirmation = confirmLabel is not null;
        CancelButton.Visibility = confirmation ? Visibility.Visible : Visibility.Collapsed;
        CancelButton.IsDefault = confirmation;
        ConfirmButton.IsDefault = !confirmation;
        MaxHeight = Math.Max(240, SystemParameters.WorkArea.Height - 48);
        Width = Math.Min(Width, SystemParameters.WorkArea.Width - 48);
        MessageScroll.MaxHeight = Math.Max(100, MaxHeight - 180);
        Loaded += (_, _) => { if (confirmation) CancelButton.Focus(); else ConfirmButton.Focus(); };
    }

    protected override void OnSourceInitialized(EventArgs e)
    { base.OnSourceInitialized(e); Appearance.ApplyTitleBar(this); }

    private void Accept(object sender, RoutedEventArgs e) => DialogResult = true;
    private void Cancel(object sender, RoutedEventArgs e) => DialogResult = false;

    public static bool Confirm(Window owner, string title, string message, string confirmLabel)
        => new ThemedDialog(title, message, confirmLabel) { Owner = owner }.ShowDialog() == true;

    public static void Inform(Window? owner, string title, string message)
    {
        var dialog = new ThemedDialog(title, message);
        if (owner is not null) dialog.Owner = owner;
        else dialog.WindowStartupLocation = WindowStartupLocation.CenterScreen;
        dialog.ShowDialog();
    }
}
