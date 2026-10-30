namespace RdBatch.Core;

public static class QueueOperations
{
    /// <summary>Wait for all writers, including already-stopping jobs, before removing queue records.</summary>
    public static async Task ClearAsync(ICollection<DownloadJob> jobs, DownloadSession? session = null)
    {
        var snapshot = jobs.ToArray();
        foreach (var job in snapshot) job.IsRemoving = true;
        try
        {
            if (session is not null) await Task.WhenAll(snapshot.Select(session.StopAsync));
            foreach (var job in snapshot) jobs.Remove(job);
        }
        finally
        {
            foreach (var job in snapshot.Where(jobs.Contains)) job.IsRemoving = false;
        }
    }
}
