using System.Runtime.InteropServices;
using System.Windows;
using System.Windows.Interop;
using Microsoft.Win32;

namespace RdBatch.App;

public static class Appearance
{
    public static bool IsDark { get; private set; }
    public static string Normalize(string? choice) => choice is "light" or "dark" ? choice : "system";

    public static void Apply(string? choice, Window? window = null)
    {
        var mode = Normalize(choice);
        IsDark = mode == "dark" || mode == "system" && SystemUsesDarkMode();
        var resources = Application.Current.Resources;
        var styles = resources.MergedDictionaries.FirstOrDefault(dictionary => dictionary.Source?.OriginalString.EndsWith("Styles.xaml", StringComparison.OrdinalIgnoreCase) == true) ?? resources;
        var name = IsDark ? "Dark" : "Light";
        // Microsoft's Fluent controls, followed by app surface/type tokens.
        styles.MergedDictionaries[0] = new ResourceDictionary
        {
            Source = new Uri($"pack://application:,,,/PresentationFramework.Fluent;component/Themes/Fluent.{name}.xaml")
        };
        styles.MergedDictionaries[1] = new ResourceDictionary
        {
            Source = new Uri($"/RealDebridBatch;component/Themes/{name}.xaml", UriKind.Relative)
        };
        // A merged-dictionary swap leaves DynamicResource references in already-shown windows on the old
        // theme. Reassigning the application dictionary forces a full resource invalidation.
        if (window is not null && ReferenceEquals(styles, resources))
        {
            Application.Current.Resources = new ResourceDictionary();
            Application.Current.Resources = resources;
        }
        if (window is not null) ApplyTitleBar(window);
    }

    private static bool SystemUsesDarkMode()
    {
        try { return Registry.GetValue(@"HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize", "AppsUseLightTheme", 1) is int value && value == 0; }
        catch (Exception ex) when (ex is System.Security.SecurityException or UnauthorizedAccessException or System.IO.IOException) { return false; }
    }

    public static void ApplyTitleBar(Window window)
    {
        var handle = new WindowInteropHelper(window).Handle;
        if (handle == IntPtr.Zero) return;
        var dark = IsDark ? 1 : 0;
        _ = DwmSetWindowAttribute(handle, 20, ref dark, sizeof(int));
    }

    [DllImport("dwmapi.dll")]
    private static extern int DwmSetWindowAttribute(IntPtr window, int attribute, ref int value, int size);
}
