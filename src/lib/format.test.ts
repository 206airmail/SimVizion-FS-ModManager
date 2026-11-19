import { describe, expect, it } from 'vitest';
import { formatBytes, formatDate, formatDateTime, tailPath } from './format';

describe('formatBytes', () => {
  it.each([
    [0, '0 B'],
    [512, '512 B'],
    [20 * 1024, '20 KB'],
    [1536, '2 KB'],
    [38.2 * 1024 ** 2, '38.2 MB'],
    [694 * 1024 ** 2, '694 MB'],
    [3.9 * 1024 ** 3, '3.9 GB'],
    [23.54 * 1024 ** 3, '23.5 GB'],
    [150 * 1024 ** 3, '150 GB'],
  ])('formats %d bytes as %s', (bytes, expected) => {
    expect(formatBytes(bytes)).toBe(expected);
  });

  it('steps up a unit instead of printing 1024', () => {
    expect(formatBytes(1024 ** 3 - 1)).toBe('1.0 GB');
    expect(formatBytes(1024 ** 2 - 1)).toBe('1.0 MB');
  });

  it('treats negative input as zero', () => {
    expect(formatBytes(-5)).toBe('0 B');
  });
});

describe('formatDate', () => {
  it('formats the calendar date in UTC', () => {
    expect(formatDate('2026-11-15T23:30:00Z', 'en-US')).toBe('Nov 15, 2026');
  });

  it('returns unparseable input unchanged', () => {
    expect(formatDate('not a date', 'en-US')).toBe('not a date');
  });
});

describe('formatDateTime', () => {
  it('formats the date and the time of day in the given time zone', () => {
    expect(formatDateTime('2026-11-17T18:42:00Z', 'en-US', 'UTC')).toMatch(
      /^Nov 17, 2026,? 6:42\sPM$/,
    );
    expect(formatDateTime('2026-11-17T23:30:00Z', 'en-US', 'Asia/Tokyo')).toMatch(
      /^Nov 18, 2026,? 8:30\sAM$/,
    );
  });

  it('returns unparseable input unchanged', () => {
    expect(formatDateTime('yesterday', 'en-US', 'UTC')).toBe('yesterday');
  });
});

describe('tailPath', () => {
  it('keeps the last segments of a Windows path', () => {
    expect(tailPath('C:\\Users\\Player\\Documents\\My Games\\FarmingSimulator2025\\mods')).toBe(
      '…\\FarmingSimulator2025\\mods',
    );
  });

  it('keeps forward slashes', () => {
    expect(tailPath('/home/player/games/fs25/mods')).toBe('…/fs25/mods');
  });

  it('leaves short paths alone', () => {
    expect(tailPath('E:\\FS25\\Contracts')).toBe('E:\\FS25\\Contracts');
  });
});
