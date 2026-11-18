// Library video thumbnails: one frame per video, read from the downloaded copy or the service link, cached on disk by
// the backend. Only rows currently on screen ask for one (VirtualList unmounts the rest), at most two run at once,
// and the newest request goes first so fast scrolling doesn't wait behind rows already gone.

import { convertFileSrc } from '@tauri-apps/api/core';
import { SvelteMap } from 'svelte/reactivity';
import { api } from './api';
import type { ThumbRequest } from './browser';

const SCHEME = 'thumb';
const WIDTH = 320;
const CONCURRENCY = 2;
/** A read that stalls (slow link, unsupported container that never errors) gives up after this long. */
const TIMEOUT_MS = 25_000;
/** A video that wasn't available (service still fetching, network error) is asked again after this long. */
const RETRY_MS = 10 * 60_000;

/** The embedded browser can't decode this file (or it has no picture), so asking again would never help. */
class Undecodable extends Error {}
/** A media element error: code 3 is a decode failure; code 4 is either an unsupported format or a source that
 * failed to open, which a probe of the source tells apart. */
class MediaFailure extends Error {
  constructor(readonly code: number) { super(`media error ${code}`); }
}

async function readable(src: string): Promise<boolean> {
  try {
    const response = await fetch(src, { headers: { Range: 'bytes=0-1' } });
    return response.ok;
  } catch {
    return false;
  }
}

const schemeUrl = (path: string) => convertFileSrc(path, SCHEME);

class Thumbnails {
  /** Thumbnail ID → image URL, or null for a video that gives no frame. Absent: not tried yet. */
  readonly known = new SvelteMap<string, string | null>();
  #loading: Promise<void> | null = null;
  #wanted = new Map<string, { request: ThumbRequest; count: number; at: number }>();
  #busy = new Set<string>();
  #retryAt = new Map<string, number>();
  #running = 0;
  #clock = 0;

  url(id: string): string | null {
    return this.known.get(id) ?? null;
  }

  /** A mounted row asks for its thumbnail; the returned function is called when the row goes away. */
  want(request: ThumbRequest): () => void {
    const current = this.#wanted.get(request.id);
    if (current) { current.count++; current.request = request; current.at = ++this.#clock; }
    else this.#wanted.set(request.id, { request, count: 1, at: ++this.#clock });
    void this.#load().then(() => this.#pump());
    return () => {
      const entry = this.#wanted.get(request.id);
      if (entry && --entry.count <= 0) this.#wanted.delete(request.id);
    };
  }

  #load(): Promise<void> {
    this.#loading ??= api.thumbnails().then(
      (map) => { for (const [id, hash] of Object.entries(map)) this.known.set(id, hash ? schemeUrl(`img-${hash}`) : null); },
      () => {}
    );
    return this.#loading;
  }

  #next(): ThumbRequest | null {
    const now = Date.now();
    let best: { request: ThumbRequest; at: number } | null = null;
    for (const [id, entry] of this.#wanted) {
      if (this.known.has(id) || this.#busy.has(id) || (this.#retryAt.get(id) ?? 0) > now) continue;
      if (!best || entry.at > best.at) best = entry;
    }
    return best?.request ?? null;
  }

  #pump() {
    while (this.#running < CONCURRENCY) {
      const request = this.#next();
      if (!request) return;
      this.#running++;
      this.#busy.add(request.id);
      void this.#run(request).finally(() => {
        this.#busy.delete(request.id);
        this.#running--;
        this.#pump();
      });
    }
  }

  async #run(request: ThumbRequest) {
    let token: string | null = null;
    try {
      token = await api.thumbnailSource(request.provider, request.sourceKey, request.fileId, request.torrentId, request.account);
      if (!token) { this.#retryAt.set(request.id, Date.now() + RETRY_MS); return; }
      const src = schemeUrl(`src-${token}`);
      const jpeg = await captureFrame(src).catch(async (error) => {
        if (error instanceof MediaFailure && (error.code === 3 || (error.code === 4 && await readable(src)))) throw new Undecodable(error.message);
        throw error;
      });
      const hash = await api.thumbnailSave(request.id, token, Array.from(jpeg));
      token = null;
      this.known.set(request.id, hash ? schemeUrl(`img-${hash}`) : null);
    } catch (error) {
      if (error instanceof Undecodable) {
        // Remembered on disk, so an HEVC/AC3-only file never costs another service link.
        const used = token;
        token = null;
        await api.thumbnailSave(request.id, used, null).catch(() => {});
        this.known.set(request.id, null);
      } else {
        this.#retryAt.set(request.id, Date.now() + RETRY_MS);
      }
    } finally {
      if (token) void api.thumbnailRelease(token).catch(() => {});
    }
  }
}

export const thumbnails = new Thumbnails();

/** Where to take the frame: past intros and logos, but not so deep that a long movie needs many range reads. */
export function frameTime(duration: number, attempt = 0): number {
  if (!Number.isFinite(duration) || duration <= 0) return attempt === 0 ? 5 : 30;
  const at = Math.min(duration * (attempt === 0 ? 0.15 : 0.35), attempt === 0 ? 300 : 900);
  return Math.max(0, Math.min(at, duration - 0.5));
}

/** Mean brightness 0–255 of a few sampled pixels; a near-black frame (fade, title card) is retried deeper in. */
function brightness(context: CanvasRenderingContext2D, width: number, height: number): number {
  const { data } = context.getImageData(0, 0, width, height);
  let sum = 0;
  let count = 0;
  for (let i = 0; i < data.length; i += 4 * 97) { sum += data[i] * 0.299 + data[i + 1] * 0.587 + data[i + 2] * 0.114; count++; }
  return count ? sum / count : 0;
}

function captureFrame(src: string): Promise<Uint8Array> {
  return new Promise((resolve, reject) => {
    const video = document.createElement('video');
    let attempt = 0;
    let settled = false;
    const finish = (error: Error | null, bytes?: Uint8Array) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      // Stops the transfer; dropping the element alone can leave WebView2 fetching in the background.
      video.removeAttribute('src');
      video.load();
      if (error) reject(error); else resolve(bytes!);
    };
    const timer = setTimeout(() => finish(new Error('timeout')), TIMEOUT_MS);
    video.muted = true;
    video.preload = 'metadata';
    video.crossOrigin = 'anonymous';
    video.playsInline = true;
    video.onerror = () => {
      finish(new MediaFailure(video.error?.code ?? 0));
    };
    video.onloadedmetadata = () => {
      if (!video.videoWidth || !video.videoHeight) { finish(new Undecodable('no video track')); return; }
      video.currentTime = frameTime(video.duration);
    };
    video.onseeked = () => {
      const width = WIDTH;
      const height = Math.max(1, Math.round((video.videoHeight / video.videoWidth) * WIDTH));
      const canvas = document.createElement('canvas');
      canvas.width = width;
      canvas.height = height;
      const context = canvas.getContext('2d', { willReadFrequently: true });
      if (!context) { finish(new Error('no canvas')); return; }
      try {
        context.drawImage(video, 0, 0, width, height);
        if (attempt === 0 && brightness(context, width, height) < 14) {
          attempt++;
          video.currentTime = frameTime(video.duration, 1);
          return;
        }
      } catch (error) {
        finish(error as Error);
        return;
      }
      canvas.toBlob((blob) => {
        if (!blob) { finish(new Error('encode failed')); return; }
        void blob.arrayBuffer().then((buffer) => finish(null, new Uint8Array(buffer)), (error) => finish(error as Error));
      }, 'image/jpeg', 0.78);
    };
    video.src = src;
  });
}
