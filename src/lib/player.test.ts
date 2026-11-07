import { describe, expect, it } from 'vitest';
import { baseName, clamp, formatTime, srtToVtt } from './player';

describe('player helpers', () => {
  it('formats time with and without hours and survives bad input', () => {
    expect(formatTime(0)).toBe('0:00');
    expect(formatTime(65.9)).toBe('1:05');
    expect(formatTime(3725)).toBe('1:02:05');
    expect(formatTime(NaN)).toBe('0:00');
    expect(formatTime(Infinity)).toBe('0:00');
    expect(formatTime(-3)).toBe('0:00');
  });

  it('converts SRT timestamps to WebVTT and keeps existing WebVTT', () => {
    const srt = '﻿1\r\n00:00:01,500 --> 00:00:03,000\r\nHello\r\n';
    expect(srtToVtt(srt)).toBe('WEBVTT\n\n1\n00:00:01.500 --> 00:00:03.000\nHello\n');
    expect(srtToVtt('WEBVTT\n\n00:01.000 --> 00:02.000\nHi')).toBe('WEBVTT\n\n00:01.000 --> 00:02.000\nHi\n');
  });

  it('clamps and takes base names', () => {
    expect(clamp(5, 0, 1)).toBe(1);
    expect(clamp(-1, 0, 1)).toBe(0);
    expect(baseName('a/b\\c.mkv')).toBe('c.mkv');
  });
});
