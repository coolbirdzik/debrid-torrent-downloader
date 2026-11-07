<script lang="ts">
  import ArrowSquareOut from 'phosphor-svelte/lib/ArrowSquareOut';
  import ArrowsIn from 'phosphor-svelte/lib/ArrowsIn';
  import ArrowsOut from 'phosphor-svelte/lib/ArrowsOut';
  import FastForward from 'phosphor-svelte/lib/FastForward';
  import Link from 'phosphor-svelte/lib/Link';
  import Pause from 'phosphor-svelte/lib/Pause';
  import PictureInPicture from 'phosphor-svelte/lib/PictureInPicture';
  import Play from 'phosphor-svelte/lib/Play';
  import Rewind from 'phosphor-svelte/lib/Rewind';
  import SpeakerHigh from 'phosphor-svelte/lib/SpeakerHigh';
  import SpeakerSlash from 'phosphor-svelte/lib/SpeakerSlash';
  import Subtitles from 'phosphor-svelte/lib/Subtitles';
  import X from 'phosphor-svelte/lib/X';
  import { onDestroy, onMount, untrack } from 'svelte';
  import { api } from '../api';
  import { app } from '../app.svelte';
  import { t, tm } from '../i18n.svelte';
  import { baseName, clamp, formatTime, RATES, SKIP_SECONDS, srtToVtt } from '../player';
  import type { Msg, Provider, StreamFile, StreamSource } from '../types';
  import Select from './Select.svelte';

  /** `fileId` opens that file directly (Library); without it the torrent's videos are offered. `account` null skips the account check. */
  /** `select` lets the backend select files on a torrent the Library still owns, so it can be fetched for playback. */
  interface Props { provider: Provider; id: string; account: string | null; title: string; fileId?: number; select?: boolean; onclose: () => void }
  let { provider, id, account, title, fileId, select = false, onclose }: Props = $props();

  type Phase = 'loading' | 'waiting' | 'pick' | 'playing' | 'error';
  /** How often to ask the service while it fetches the torrent; slow enough not to trip its rate limit. */
  const WAIT_POLL_MS = 8000;
  let waiting = $state({ status: '', progress: 0 });
  let alive = true;
  let phase = $state<Phase>('loading');
  let files = $state<StreamFile[]>([]);
  let current = $state<StreamFile | null>(null);
  let source = $state<StreamSource | null>(null);
  let failure = $state<Msg | null>(null);
  let codecWarning = $state(false);
  let buffering = $state(true);

  let root: HTMLDivElement | undefined = $state();
  let video: HTMLVideoElement | undefined = $state();
  let bar: HTMLDivElement | undefined = $state();
  let subtitleInput: HTMLInputElement | undefined = $state();
  let subtitleUrl = $state<string | null>(null);

  let paused = $state(true);
  let time = $state(0);
  let duration = $state(0);
  let bufferedEnd = $state(0);
  let volume = $state(1);
  let muted = $state(false);
  let rate = $state(1);
  let fullscreen = $state(false);
  let controlsVisible = $state(true);
  let scrubbing = $state(false);
  let hideTimer: ReturnType<typeof setTimeout> | undefined;
  let audioTimer: ReturnType<typeof setTimeout> | undefined;
  let token = 0;

  const displayName = $derived(source ? baseName(source.filename) : current ? baseName(current.path) : title);
  const progress = $derived(duration > 0 ? (time / duration) * 100 : 0);
  const buffered = $derived(duration > 0 ? (bufferedEnd / duration) * 100 : 0);
  const showControls = $derived(controlsVisible || paused || scrubbing || phase !== 'playing');

  function remember(key: string, value: string) { try { localStorage.setItem(key, value); } catch { /* storage may be blocked */ } }
  function recall(key: string): string | null { try { return localStorage.getItem(key); } catch { return null; } }

  onMount(async () => {
    const savedVolume = Number(recall('player.volume'));
    if (Number.isFinite(savedVolume) && recall('player.volume') !== null) volume = clamp(savedVolume, 0, 1);
    document.addEventListener('fullscreenchange', onFullscreenChange);
    root?.focus();
    try {
      // The service may still be fetching the torrent: wait here instead of failing, until it is ready or the player closes.
      for (let first = true; alive; first = false) {
        const list = await api.streamFiles(provider, id, account, select && first);
        if (!alive) return;
        if (list.ready) { files = list.files; break; }
        waiting = { status: list.status, progress: list.progress };
        phase = 'waiting';
        await new Promise((done) => setTimeout(done, WAIT_POLL_MS));
      }
      if (!alive) return;
      const wanted = fileId === undefined ? undefined : files.find((f) => f.fileId === fileId);
      if (fileId !== undefined && !wanted) fail({ key: 'Video này chưa có link phát trên dịch vụ.', args: [] });
      else if (wanted) await play(wanted);
      else if (files.length === 0) fail({ key: 'Torrent này không có video có thể phát trực tiếp.', args: [] });
      else if (files.length === 1) await play(files[0]);
      else phase = 'pick';
    } catch (error) {
      fail(error as Msg);
    }
  });

  onDestroy(() => {
    alive = false;
    document.removeEventListener('fullscreenchange', onFullscreenChange);
    if (document.fullscreenElement) void document.exitFullscreen().catch(() => {});
    clearTimeout(hideTimer);
    clearTimeout(audioTimer);
    release();
  });

  /** Stops the transfer: pausing alone leaves WebView2 buffering the rest of the file in the background. */
  function release() {
    if (subtitleUrl) URL.revokeObjectURL(subtitleUrl);
    subtitleUrl = null;
    if (!video) return;
    video.pause();
    video.removeAttribute('src');
    video.load();
  }

  function fail(error: Msg) {
    failure = error;
    phase = 'error';
  }

  async function play(file: StreamFile) {
    const mine = ++token;
    current = file;
    source = null;
    failure = null;
    codecWarning = false;
    buffering = true;
    time = 0;
    duration = 0;
    bufferedEnd = 0;
    phase = 'loading';
    release();
    try {
      const resolved = await api.streamUrl(provider, id, file.linkIndex, account);
      if (mine !== token) return;
      source = resolved;
      phase = 'playing';
    } catch (error) {
      if (mine === token) fail(error as Msg);
    }
  }

  // Once the <video> exists and has a source, apply the saved volume and start.
  $effect(() => {
    if (phase !== 'playing' || !video || !source) return;
    const element = video;
    // Only the first start belongs here; volume/mute/rate changes must not restart a paused video.
    untrack(() => applySettings(element));
    void element.play().catch(() => {});
  });

  function applySettings(element: HTMLVideoElement) {
    element.volume = volume;
    element.muted = muted;
    element.playbackRate = rate;
  }

  /** Stops the transfer before the overlay goes away; relying on onDestroy alone can run after the binding is cleared. */
  function close() {
    clearTimeout(hideTimer);
    clearTimeout(audioTimer);
    release();
    onclose();
  }

  function onError() {
    const code = video?.error?.code;
    // MEDIA_ERR_SRC_NOT_SUPPORTED (4) or DECODE (3): the container or a codec is unsupported by the embedded browser.
    fail(code === 3 || code === 4
      ? { key: 'Định dạng hoặc codec của video này không phát được trong app (thường là HEVC/x265, AC3, DTS). Hãy mở bằng trình phát ngoài.', args: [] }
      : { key: 'Không tải được luồng video. Link có thể đã hết hạn; thử lại hoặc mở bằng trình phát ngoài.', args: [] });
  }

  function onPlaying() {
    buffering = false;
    wake();
    clearTimeout(audioTimer);
    // Some files (AC3/DTS audio) play video with no sound and no error. A decoded-bytes counter stuck at zero reveals it.
    audioTimer = setTimeout(() => {
      const v = video as (HTMLVideoElement & { webkitAudioDecodedByteCount?: number }) | undefined;
      if (v && !v.paused && !v.muted && v.volume > 0 && v.webkitAudioDecodedByteCount === 0) codecWarning = true;
    }, 4000);
  }

  function updateBuffered() {
    if (!video) return;
    const ranges = video.buffered;
    let end = 0;
    for (let i = 0; i < ranges.length; i++) if (ranges.start(i) <= video.currentTime + 0.5) end = Math.max(end, ranges.end(i));
    bufferedEnd = end;
  }

  function toggle() {
    if (!video || phase !== 'playing') return;
    if (video.paused) void video.play().catch(() => {});
    else video.pause();
  }

  function seekTo(seconds: number) {
    if (!video || !Number.isFinite(duration) || duration <= 0) return;
    video.currentTime = clamp(seconds, 0, duration);
    time = video.currentTime;
  }
  const skip = (delta: number) => seekTo((video?.currentTime ?? 0) + delta);

  function setVolume(value: number) {
    volume = clamp(value, 0, 1);
    muted = volume === 0;
    if (video) { video.volume = volume; video.muted = muted; }
    remember('player.volume', String(volume));
  }
  function toggleMute() {
    muted = !muted;
    if (!muted && volume === 0) setVolume(0.5);
    if (video) video.muted = muted;
  }

  function setRate(value: number) {
    rate = value;
    if (video) video.playbackRate = value;
  }

  function onFullscreenChange() { fullscreen = document.fullscreenElement === root; }
  async function toggleFullscreen() {
    try {
      if (document.fullscreenElement) await document.exitFullscreen();
      else await root?.requestFullscreen();
    } catch { /* the window may refuse; the player still works inline */ }
  }

  async function pip() {
    try {
      if (document.pictureInPictureElement) await document.exitPictureInPicture();
      else await video?.requestPictureInPicture();
    } catch { /* unsupported for this file */ }
  }

  async function copyLink() {
    if (!source) return;
    try {
      await navigator.clipboard.writeText(source.url);
      app.toast({ key: 'Đã sao chép link phát.', args: [] }, 'success');
    } catch { /* clipboard denied */ }
  }

  async function external() {
    if (!source) return;
    try {
      await api.openExternalPlayer(source.url, source.filename);
      video?.pause();
    } catch (error) {
      app.toast(error as Msg, 'error');
    }
  }

  async function loadSubtitle(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = '';
    if (!file) return;
    try {
      const vtt = srtToVtt(await file.text());
      if (subtitleUrl) URL.revokeObjectURL(subtitleUrl);
      subtitleUrl = URL.createObjectURL(new Blob([vtt], { type: 'text/vtt' }));
      // The new <track> element appears after the DOM updates; show it once it is attached.
      queueMicrotask(() => setTimeout(() => { if (video?.textTracks[0]) video.textTracks[0].mode = 'showing'; }, 50));
    } catch {
      app.toast({ key: 'Không đọc được file phụ đề.', args: [] }, 'error');
    }
  }

  function wake() {
    controlsVisible = true;
    clearTimeout(hideTimer);
    hideTimer = setTimeout(() => (controlsVisible = false), 2800);
  }

  function seekFromPointer(event: PointerEvent) {
    if (!bar) return;
    const rect = bar.getBoundingClientRect();
    seekTo(((event.clientX - rect.left) / rect.width) * duration);
  }
  function scrubStart(event: PointerEvent) {
    scrubbing = true;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    seekFromPointer(event);
  }
  function scrubMove(event: PointerEvent) { if (scrubbing) seekFromPointer(event); }
  function scrubEnd() { scrubbing = false; }

  function keydown(event: KeyboardEvent) {
    // A dropdown that handled the key (its own Escape/arrows/Space) already called preventDefault.
    if (event.defaultPrevented) return;
    const target = event.target as HTMLElement | null;
    // A focused button activates itself on Space/Enter.
    if (target?.tagName === 'BUTTON' && (event.key === ' ' || event.key === 'Enter')) return;
    if (target?.tagName === 'INPUT' && (target as HTMLInputElement).type === 'range' && event.key.startsWith('Arrow')) return;
    wake();
    switch (event.key) {
      case 'Escape':
        if (document.fullscreenElement) return; // the browser exits fullscreen itself
        event.preventDefault(); close(); return;
      case ' ': case 'k': case 'K': event.preventDefault(); toggle(); return;
      case 'ArrowLeft': event.preventDefault(); skip(event.shiftKey ? -SKIP_SECONDS * 3 : -5); return;
      case 'ArrowRight': event.preventDefault(); skip(event.shiftKey ? SKIP_SECONDS * 3 : 5); return;
      case 'j': case 'J': skip(-SKIP_SECONDS); return;
      case 'l': case 'L': skip(SKIP_SECONDS); return;
      case 'ArrowUp': event.preventDefault(); setVolume(volume + 0.05); return;
      case 'ArrowDown': event.preventDefault(); setVolume(volume - 0.05); return;
      case 'm': case 'M': toggleMute(); return;
      case 'f': case 'F': void toggleFullscreen(); return;
      case '<': setRate(RATES[Math.max(0, RATES.indexOf(rate) - 1)] ?? 1); return;
      case '>': setRate(RATES[Math.min(RATES.length - 1, RATES.indexOf(rate) + 1)] ?? 1); return;
      case 'Home': seekTo(0); return;
      case 'End': seekTo(duration); return;
      default:
        if (/^[0-9]$/.test(event.key)) seekTo((Number(event.key) / 10) * duration);
    }
  }
</script>

<div class="player" onmousedown={(e) => { if ((e.target as HTMLElement).closest('.pbtn')) e.preventDefault(); }} class:hide-cursor={phase === 'playing' && !showControls} bind:this={root} role="dialog" aria-modal="true" aria-label={t('Trình phát video')} tabindex="-1" onkeydown={keydown} onpointermove={wake}>
  <header class="top" class:hidden={!showControls}>
    <b class="ell" title={displayName}>{displayName}</b>
    {#if files.length > 1 && current}
      <Select label={t('Chọn video')} width="260px" value={current.linkIndex} options={files.map((f) => ({ value: f.linkIndex, label: baseName(f.path) }))} onchange={(index: number) => { const next = files.find((f) => f.linkIndex === index); if (next) void play(next); }} />
    {/if}
    <button class="pbtn" onclick={close} aria-label={t('Đóng')} title={t('Đóng (Esc)')}><X size={20} weight="bold" /></button>
  </header>

  {#if phase === 'playing' && source}
    <!-- svelte-ignore a11y_media_has_caption -->
    <video
      bind:this={video}
      src={source.url}
      preload="auto"
      playsinline
      onclick={toggle}
      ondblclick={toggleFullscreen}
      onerror={onError}
      onplaying={onPlaying}
      onwaiting={() => (buffering = true)}
      oncanplay={() => (buffering = false)}
      onpause={() => (paused = true)}
      onplay={() => (paused = false)}
      ontimeupdate={() => { if (!scrubbing) time = video?.currentTime ?? 0; updateBuffered(); }}
      onprogress={updateBuffered}
      ondurationchange={() => (duration = video?.duration ?? 0)}
      onloadedmetadata={() => { duration = video?.duration ?? 0; if (video) applySettings(video); }}
      onended={() => (paused = true)}
    >
      {#if subtitleUrl}<track kind="subtitles" src={subtitleUrl} label="Subtitles" default />{/if}
    </video>
  {/if}

  {#if phase === 'waiting'}
    <div class="center" role="status">
      <span class="spinner"></span>
      <span>{t('Dịch vụ đang chuẩn bị video · {0}%', waiting.progress.toFixed(0))}</span>
      <span class="sub">{waiting.status.replaceAll('_', ' ')}</span>
    </div>
  {/if}

  {#if phase === 'loading' || (phase === 'playing' && buffering)}
    <div class="center" role="status"><span class="spinner"></span><span>{phase === 'loading' ? t('Đang lấy link phát…') : t('Đang đệm…')}</span></div>
  {/if}

  {#if phase === 'pick'}
    <div class="center pick">
      <h2 class="section">{t('Chọn video để phát')}</h2>
      <ul>
        {#each files as file (file.linkIndex)}
          <li><button onclick={() => play(file)}><Play size={16} weight="fill" /><span class="ell">{file.path}</span></button></li>
        {/each}
      </ul>
    </div>
  {/if}

  {#if phase === 'error' && failure}
    <div class="center" role="alert">
      <p class="msg">{tm(failure)}</p>
      <div class="row">
        {#if source}<button class="btn primary" onclick={external}><ArrowSquareOut size={16} weight="bold" />{t('Mở bằng trình phát ngoài')}</button>
        <button class="btn" onclick={copyLink}><Link size={16} weight="bold" />{t('Sao chép link')}</button>{/if}
        {#if current}<button class="btn" onclick={() => current && play(current)}>{t('Thử lại')}</button>{/if}
        <button class="btn" onclick={close}>{t('Đóng')}</button>
      </div>
    </div>
  {/if}

  {#if codecWarning && phase === 'playing'}
    <div class="warn" role="status">
      <span>{t('Không nghe thấy âm thanh? Codec âm thanh (AC3/DTS) có thể không được hỗ trợ trong app.')}</span>
      <button class="btn" onclick={external}>{t('Mở bằng trình phát ngoài')}</button>
      <button class="pbtn" onclick={() => (codecWarning = false)} aria-label={t('Đóng')}><X size={16} weight="bold" /></button>
    </div>
  {/if}

  {#if phase === 'playing'}
    <footer class="bottom" class:hidden={!showControls}>
      <div class="seek" bind:this={bar} role="slider" tabindex="0" aria-label={t('Thanh tua')} aria-valuemin="0" aria-valuemax={Math.floor(duration)} aria-valuenow={Math.floor(time)} aria-valuetext={`${formatTime(time)} / ${formatTime(duration)}`} onpointerdown={scrubStart} onpointermove={scrubMove} onpointerup={scrubEnd} onpointercancel={scrubEnd}>
        <span class="buffered" style:width="{buffered}%"></span>
        <span class="played" style:width="{progress}%"></span>
        <span class="thumb" style:left="{progress}%"></span>
      </div>
      <div class="controls">
        <button class="pbtn" onclick={() => skip(-SKIP_SECONDS)} aria-label={t('Lùi {0} giây', SKIP_SECONDS)} title={t('Lùi {0} giây', SKIP_SECONDS)}><Rewind size={20} weight="fill" /></button>
        <button class="pbtn big" onclick={toggle} aria-label={paused ? t('Phát') : t('Tạm dừng')} title={paused ? t('Phát (Space)') : t('Tạm dừng (Space)')}>
          {#if paused}<Play size={24} weight="fill" />{:else}<Pause size={24} weight="fill" />{/if}
        </button>
        <button class="pbtn" onclick={() => skip(SKIP_SECONDS)} aria-label={t('Tới {0} giây', SKIP_SECONDS)} title={t('Tới {0} giây', SKIP_SECONDS)}><FastForward size={20} weight="fill" /></button>
        <button class="pbtn" onclick={toggleMute} aria-label={muted ? t('Bật tiếng') : t('Tắt tiếng')} title={t('Tắt/bật tiếng (M)')}>
          {#if muted || volume === 0}<SpeakerSlash size={20} weight="fill" />{:else}<SpeakerHigh size={20} weight="fill" />{/if}
        </button>
        <input class="vol" type="range" min="0" max="1" step="0.01" value={muted ? 0 : volume} aria-label={t('Âm lượng')} oninput={(e) => setVolume(Number(e.currentTarget.value))} />
        <span class="time">{formatTime(time)} / {formatTime(duration)}</span>
        <span class="grow"></span>
        <Select label={t('Tốc độ phát')} width="86px" value={rate} options={RATES.map((r) => ({ value: r, label: `${r}×` }))} onchange={setRate} />
        <button class="pbtn" onclick={() => subtitleInput?.click()} aria-label={t('Thêm phụ đề')} title={t('Thêm phụ đề (.srt / .vtt)')}><Subtitles size={20} weight="fill" /></button>
        <input bind:this={subtitleInput} type="file" accept=".srt,.vtt" hidden onchange={loadSubtitle} />
        <button class="pbtn" onclick={pip} aria-label={t('Hình trong hình')} title={t('Hình trong hình')}><PictureInPicture size={20} weight="fill" /></button>
        <button class="pbtn" onclick={copyLink} aria-label={t('Sao chép link')} title={t('Sao chép link')}><Link size={20} weight="bold" /></button>
        <button class="pbtn" onclick={external} aria-label={t('Mở bằng trình phát ngoài')} title={t('Mở bằng trình phát ngoài')}><ArrowSquareOut size={20} weight="bold" /></button>
        <button class="pbtn" onclick={toggleFullscreen} aria-label={t('Toàn màn hình')} title={t('Toàn màn hình (F)')}>
          {#if fullscreen}<ArrowsIn size={20} weight="bold" />{:else}<ArrowsOut size={20} weight="bold" />{/if}
        </button>
      </div>
    </footer>
  {/if}
</div>

<style>
  .player { position: fixed; inset: 0; z-index: 60; background: #05070b; color: #f2f4f8; display: flex; flex-direction: column; outline: none; user-select: none; }
  .player.hide-cursor { cursor: none; }
  video { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: contain; background: #000; }
  .top, .bottom { position: absolute; left: 0; right: 0; z-index: 2; display: flex; padding: 12px 16px; transition: opacity 0.2s; }
  .top { top: 0; align-items: center; gap: 12px; background: linear-gradient(#000c, #0000); }
  .top b { flex: 1; min-width: 0; font-size: 14px; }
  .bottom { bottom: 0; flex-direction: column; gap: 8px; padding-top: 28px; background: linear-gradient(#0000, #000d); }
  .hidden { opacity: 0; pointer-events: none; }
  .ell { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .controls { display: flex; align-items: center; gap: 4px; }
  .grow { flex: 1; }
  .time { font-size: 12px; font-variant-numeric: tabular-nums; margin-left: 8px; color: #c9ced8; white-space: nowrap; }
  .pbtn { display: inline-flex; align-items: center; justify-content: center; width: 36px; height: 36px; border-radius: 10px; border: 0; background: transparent; color: #aab1c0; cursor: pointer; }
  .pbtn:hover { background: #ffffff1c; color: #fff; }
  .pbtn.big { width: 44px; height: 44px; color: #fff; }
  .pbtn:focus-visible { outline: 2px solid var(--cyan); }
  .seek { position: relative; height: 16px; cursor: pointer; touch-action: none; }
  .seek::before { content: ''; position: absolute; left: 0; right: 0; top: 6px; height: 4px; border-radius: 2px; background: #ffffff30; }
  .seek:hover::before { height: 6px; top: 5px; }
  .buffered, .played { position: absolute; left: 0; top: 6px; height: 4px; border-radius: 2px; pointer-events: none; }
  .seek:hover .buffered, .seek:hover .played { height: 6px; top: 5px; }
  .buffered { background: #ffffff40; }
  .played { background: var(--cyan); }
  .thumb { position: absolute; top: 3px; width: 10px; height: 10px; margin-left: -5px; border-radius: 50%; background: var(--cyan); pointer-events: none; opacity: 0; transition: opacity 0.15s; }
  .seek:hover .thumb { opacity: 1; }
  .vol { width: 76px; accent-color: var(--cyan); }
  .center { position: absolute; inset: 0; z-index: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; text-align: center; padding: 24px; pointer-events: none; }
  .center .row, .center ul, .center button { pointer-events: auto; }
  .center.pick { pointer-events: auto; overflow: auto; justify-content: flex-start; padding-top: 72px; }
  .pick ul { list-style: none; margin: 0; padding: 0; width: min(640px, 100%); display: flex; flex-direction: column; gap: 4px; }
  .pick li button { width: 100%; display: flex; align-items: center; gap: 10px; padding: 10px 14px; border: 0; border-radius: 10px; background: #ffffff10; color: #f2f4f8; text-align: left; cursor: pointer; }
  .pick li button:hover { background: #ffffff22; }
  .msg { max-width: 520px; line-height: 1.5; }
  .sub { font-size: 12px; color: #8a92a3; }
  .row { display: flex; gap: 8px; flex-wrap: wrap; justify-content: center; }
  .spinner { width: 34px; height: 34px; border-radius: 50%; border: 3px solid #ffffff30; border-top-color: var(--cyan); animation: spin 0.8s linear infinite; }
  .warn { position: absolute; z-index: 3; top: 64px; left: 50%; transform: translateX(-50%); display: flex; align-items: center; gap: 10px; padding: 10px 14px; border-radius: 12px; background: #1b2130; font-size: 13px; max-width: min(720px, 92vw); }
  @keyframes spin { to { transform: rotate(360deg); } }
  @media (prefers-reduced-motion: reduce) { .spinner { animation-duration: 3s; } }
</style>
