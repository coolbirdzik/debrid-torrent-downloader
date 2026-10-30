namespace RdBatch.Core;

public sealed class DownloadSession : IDisposable
{
    private readonly DownloadEngine _engine;
    private readonly string _root;
    private readonly bool _flatten;
    private readonly DownloadPathAllocator _paths;
    private readonly SemaphoreSlim _slots;
    private readonly object _gate = new();
    private readonly Dictionary<DownloadJob, Execution> _active = [];
    private TaskCompletionSource _idle = CompletedSource();
    private bool _disposed;

    internal DownloadSession(DownloadEngine engine, IEnumerable<DownloadJob> jobs, string root, int concurrency, bool flatten)
    {
        _engine = engine; _root = root; _flatten = flatten;
        _paths = new(jobs);
        _slots = new(Math.Clamp(concurrency, 1, 5));
    }

    public int ActiveCount { get { lock (_gate) return _active.Count; } }
    public Task WhenIdleAsync() { lock (_gate) return _idle.Task; }

    public Task EnqueueAsync(DownloadJob job, CancellationToken ct = default)
    {
        Execution execution;
        lock (_gate)
        {
            ObjectDisposedException.ThrowIf(_disposed, this);
            if (_active.TryGetValue(job, out var current)) return current.Done.Task;
            if (job.State == JobState.Completed || job.IsRemoving) return Task.CompletedTask;
            if (_active.Count == 0) _idle = new(TaskCreationOptions.RunContinuationsAsynchronously);
            execution = new(CancellationTokenSource.CreateLinkedTokenSource(ct));
            _active.Add(job, execution);
        }
        job.IsActive = true;
        job.IsStopping = false;
        job.State = JobState.Queued;
        job.SetDetail("Chờ lượt xử lý…");
        _engine.Publish(job);
        _ = ExecuteAsync(job, execution);
        return execution.Done.Task;
    }

    public Task PauseAsync(DownloadJob job) => CancelJobAsync(job, JobState.Paused);
    public Task StopAsync(DownloadJob job) => CancelJobAsync(job, JobState.Stopped);
    public Task StopAllAsync()
    {
        DownloadJob[] jobs;
        lock (_gate) jobs = _active.Keys.ToArray();
        return Task.WhenAll(jobs.Select(StopAsync));
    }

    private Task CancelJobAsync(DownloadJob job, JobState state)
    {
        Execution? execution;
        lock (_gate)
        {
            if (_active.TryGetValue(job, out execution))
            { execution.CancellationState = state; job.IsStopping = true; }
        }
        if (execution is null)
        {
            if (job.State is JobState.Queued or JobState.Paused or JobState.Stopped)
            { job.State = state; job.SetDetail(CancellationDetail(state)); _engine.Publish(job); }
            return Task.CompletedTask;
        }
        _engine.Publish(job);
        try { execution.Cancellation.Cancel(); }
        catch (ObjectDisposedException) { } // The transfer may have completed concurrently with Stop.
        return execution.Done.Task;
    }

    private async Task ExecuteAsync(DownloadJob job, Execution execution)
    {
        var acquired = false;
        try
        {
            await _slots.WaitAsync(execution.Cancellation.Token);
            acquired = true;
            execution.Cancellation.Token.ThrowIfCancellationRequested();
            if (job.IsRemoving) { job.State = JobState.Stopped; return; }
            await _engine.ProcessAsync(job, _root, _flatten, _paths, execution.Cancellation.Token);
        }
        catch (OperationCanceledException) when (execution.Cancellation.IsCancellationRequested)
        { job.State = execution.CancellationState; job.SetDetail(CancellationDetail(job.State)); }
        catch (Exception ex)
        { job.State = JobState.Failed; job.SetDetail(DownloadEngine.FriendlyErrorKey(ex)); }
        finally
        {
            if (acquired) _slots.Release();
            TaskCompletionSource? idle = null;
            lock (_gate)
            {
                _active.Remove(job);
                job.IsActive = false;
                job.IsStopping = false;
                if (_active.Count == 0) idle = _idle;
            }
            // Publish before callers awaiting Stop can remove the row; complete only this idle generation.
            _engine.Publish(job);
            execution.Done.TrySetResult();
            idle?.TrySetResult();
            execution.Cancellation.Dispose();
        }
    }

    private static string CancellationDetail(JobState state) => state == JobState.Stopped
        ? "Đã dừng · giữ file tạm · bấm Resume để tải tiếp"
        : "Đã tạm dừng · giữ file tạm · bấm Resume để tải tiếp";
    private static TaskCompletionSource CompletedSource()
    { var source = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously); source.SetResult(); return source; }
    public void Dispose()
    {
        lock (_gate)
        {
            if (_disposed) return;
            if (_active.Count > 0) throw new InvalidOperationException(Text.Get("Dừng hàng đợi trước khi đóng phiên tải."));
            _disposed = true;
        }
        _slots.Dispose();
    }
    private sealed class Execution(CancellationTokenSource cancellation)
    {
        public CancellationTokenSource Cancellation { get; } = cancellation;
        public JobState CancellationState { get; set; } = JobState.Paused;
        public TaskCompletionSource Done { get; } = new(TaskCreationOptions.RunContinuationsAsynchronously);
    }
}
