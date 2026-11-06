<script lang="ts">
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import { onMount } from 'svelte';
  import { app } from '$lib/app.svelte';
  import { library } from '$lib/library.svelte';
  import { t, tm } from '$lib/i18n.svelte';
  import LibraryPage from '$lib/components/LibraryPage.svelte';
  import Overlays from '$lib/components/Overlays.svelte';
  import QueuePage from '$lib/components/QueuePage.svelte';
  import Rail from '$lib/components/Rail.svelte';
  import RemotePage from '$lib/components/RemotePage.svelte';
  import SettingsPage from '$lib/components/SettingsPage.svelte';

  const filesDone = $derived(app.jobs.reduce((sum, job) => sum + job.files.filter((file) => file.completed).length, 0));

  onMount(() => {
    let unlisten: (() => void) | undefined;
    app.init().catch((error) => app.toast({ key: String(error?.key ?? error), args: [] }, 'error'));
    // Dropping .torrent files anywhere fills the Library input, which only lives on the Library page.
    getCurrentWebview()
      .onDragDropEvent((event) => {
        if (event.payload.type === 'drop') {
          library.appendSources(event.payload.paths);
          app.page = 'library';
        }
      })
      .then((stop) => (unlisten = stop))
      .catch(() => {});
    return () => unlisten?.();
  });
</script>

{#if app.ready}
  <div class="shell">
    <aside><Rail /></aside>
    <main>
      {#if app.warning}<div class="warning card">{tm(app.warning)}</div>{/if}
      <div class="page-host">
        {#if app.page === 'library'}<LibraryPage />
        {:else if app.page === 'queue'}<QueuePage />
        {:else if app.page === 'remote'}<RemotePage />
        {:else}<SettingsPage />{/if}
      </div>
    </main>
    <footer class="status">
      <span class="ell" title={tm(app.status)}>{tm(app.status)}</span>
      <span class="right">{t('Hàng đợi: {0}/{1} torrent hoàn tất · {2} file đã tải', app.completedCount, app.jobs.length, filesDone)}</span>
    </footer>
  </div>
  <Overlays />
{/if}

<style>
  .shell { display: grid; grid-template-columns: 248px 1fr; grid-template-rows: 1fr 34px; height: 100vh; }
  aside { grid-row: 1; }
  main { grid-row: 1; padding: 24px 24px 0 8px; min-width: 0; min-height: 0; display: flex; flex-direction: column; }
  .page-host { flex: 1; min-height: 0; }
  .warning { padding: 10px 14px; margin-bottom: 12px; color: var(--partial); }
  .status { grid-column: 1 / -1; display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 0 24px; font-size: 11px; color: var(--muted); }
  .ell { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .right { white-space: nowrap; }
</style>
