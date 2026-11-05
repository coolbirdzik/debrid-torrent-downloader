namespace RdBatch.App;

public sealed class RangeSelection<T>(Func<T, bool> selected, Action<T, bool> setSelected) where T : class
{
    public T? Anchor { get; set; }

    public void Apply(IReadOnlyList<T> items, T target, bool shift, bool control, bool toggle)
    {
        var index = IndexOf(items, target);
        if (index < 0) return;
        var anchorIndex = Anchor is null ? -1 : IndexOf(items, Anchor);
        if (shift && anchorIndex >= 0)
        {
            var first = Math.Min(anchorIndex, index);
            var last = Math.Max(anchorIndex, index);
            for (var i = 0; i < items.Count; i++)
            {
                if (i >= first && i <= last) setSelected(items[i], true);
                else if (!control) setSelected(items[i], false);
            }
            return; // Repeated Shift clicks keep the same anchor.
        }
        if (control || toggle) setSelected(target, !selected(target));
        else foreach (var item in items) setSelected(item, ReferenceEquals(item, target));
        Anchor = target;
    }

    private static int IndexOf(IReadOnlyList<T> items, T target)
    {
        for (var i = 0; i < items.Count; i++) if (ReferenceEquals(items[i], target)) return i;
        return -1;
    }
}
