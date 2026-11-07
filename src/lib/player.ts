/** Small pure helpers for the video player. */

export const RATES = [0.5, 0.75, 1, 1.25, 1.5, 2];
export const SKIP_SECONDS = 10;

export function formatTime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return '0:00';
  const total = Math.floor(seconds);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = String(total % 60).padStart(2, '0');
  return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${s}` : `${m}:${s}`;
}

export const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));

/** SubRip to WebVTT, so a local .srt can be shown through a <track>. WebVTT input is returned as is. */
export function srtToVtt(text: string): string {
  const clean = text.replace(/^﻿/, '').replace(/\r\n?/g, '\n').trim();
  if (clean.startsWith('WEBVTT')) return clean + '\n';
  const body = clean.replace(/(\d{1,2}:\d{2}:\d{2}),(\d{3})/g, '$1.$2');
  return `WEBVTT\n\n${body}\n`;
}

export function fileExtension(name: string): string {
  const dot = name.lastIndexOf('.');
  return dot < 0 ? '' : name.slice(dot + 1).toLowerCase();
}

export function baseName(path: string): string {
  return path.replace(/\\/g, '/').split('/').pop() ?? path;
}
