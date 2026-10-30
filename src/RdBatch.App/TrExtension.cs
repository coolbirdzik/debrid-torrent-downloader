using System.ComponentModel;
using System.Globalization;
using System.Windows.Data;
using System.Windows.Markup;
using RdBatch.Core;

namespace RdBatch.App;

public sealed class TranslationSource : INotifyPropertyChanged
{
    public static TranslationSource Instance { get; } = new();
    public int Revision { get; private set; }
    public event PropertyChangedEventHandler? PropertyChanged;
    private TranslationSource() => Text.LanguageChanged += () =>
    {
        Revision++;
        PropertyChanged?.Invoke(this, new(nameof(Revision)));
    };
}

[MarkupExtensionReturnType(typeof(string))]
public sealed class TrExtension(string key) : MarkupExtension
{
    public override object ProvideValue(IServiceProvider serviceProvider) => new Binding(nameof(TranslationSource.Revision))
    {
        Source = TranslationSource.Instance,
        Mode = BindingMode.OneWay,
        Converter = new TranslationConverter(key)
    }.ProvideValue(serviceProvider);

    private sealed class TranslationConverter(string key) : IValueConverter
    {
        public object Convert(object value, Type targetType, object parameter, CultureInfo culture) => Text.Get(key);
        public object ConvertBack(object value, Type targetType, object parameter, CultureInfo culture) => throw new NotSupportedException();
    }
}
