<script lang="ts">
  import { untrack, type Snippet } from 'svelte';
  import type { ThumbRequest } from '../browser';
  import { thumbnails } from '../thumbnails.svelte';

  /** Shows a video's cached frame, asking for one while mounted; `children` is drawn until (or unless) there is one. */
  interface Props { request: ThumbRequest | null; children: Snippet }
  let { request, children }: Props = $props();

  // Keyed on the request's content, so a row re-rendering with an equal request keeps its place in the queue, while
  // one whose source changed (cloud became ready) hands the queue the new request.
  const id = $derived(request?.id ?? null);
  const key = $derived(request ? `${request.id}|${request.torrentId}|${request.account}` : null);
  $effect(() => {
    if (key === null) return;
    return untrack(() => (request ? thumbnails.want(request) : undefined));
  });
  const src = $derived(id ? thumbnails.url(id) : null);
  let broken = $state<string | null>(null);
</script>

{#if src && broken !== src}
  <img class="thumb-img" {src} alt="" draggable="false" decoding="async" onerror={() => (broken = src)} />
{:else}
  {@render children()}
{/if}

<style>
  .thumb-img { display: block; width: 100%; height: 100%; object-fit: cover; border-radius: inherit; background: #000; }
</style>
