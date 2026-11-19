import { describe, expect, it } from 'vitest';
import type { UpdateSettings } from '../../api';
import { strings } from '../../strings';
import { deriveStatus, statusDot, statusText, type StatusInput } from './status';

const NOW = Date.parse('2026-11-19T10:45:00Z');
const on: UpdateSettings = {
  enabled: true,
  lastCheck: '2026-11-19T10:42:00Z',
  retryAfter: null,
};
const input = (overrides: Partial<StatusInput> = {}): StatusInput => ({
  settings: on,
  checking: false,
  failure: null,
  ...overrides,
});

describe('deriveStatus', () => {
  it('says nothing until the settings are known', () => {
    expect(deriveStatus(input({ settings: null }), NOW)).toBeNull();
  });

  it('is off whenever the switch is off, whatever else is going on', () => {
    const off = { ...on, enabled: false };
    expect(deriveStatus(input({ settings: off, checking: true, failure: 'offline' }), NOW)).toEqual(
      { kind: 'off' },
    );
  });

  it('is checking while a check runs', () => {
    expect(deriveStatus(input({ checking: true }), NOW)).toEqual({ kind: 'checking' });
  });

  it('is paused until a future retry time, and ignores one that has passed', () => {
    const paused = { ...on, retryAfter: '2026-11-19T10:47:00Z' };
    expect(deriveStatus(input({ settings: paused }), NOW)).toEqual({
      kind: 'paused',
      until: '2026-11-19T10:47:00Z',
    });
    const past = { ...on, retryAfter: '2026-11-19T10:30:00Z' };
    expect(deriveStatus(input({ settings: past }), NOW)).toEqual({
      kind: 'checked',
      at: on.lastCheck,
    });
  });

  it('is offline after an offline failure, but not after another one', () => {
    expect(deriveStatus(input({ failure: 'offline' }), NOW)).toEqual({ kind: 'offline' });
    expect(deriveStatus(input({ failure: 'internal' }), NOW)).toEqual({
      kind: 'checked',
      at: on.lastCheck,
    });
  });

  it('shows the last check, or that there has been none', () => {
    expect(deriveStatus(input(), NOW)).toEqual({ kind: 'checked', at: on.lastCheck });
    expect(deriveStatus(input({ settings: { ...on, lastCheck: null } }), NOW)).toEqual({
      kind: 'notChecked',
    });
  });
});

describe('statusText and statusDot', () => {
  it('words every state', () => {
    const t = strings.updates.status;
    expect(statusText({ kind: 'off' })).toBe(t.off);
    expect(statusText({ kind: 'checking' })).toBe(t.checking);
    expect(statusText({ kind: 'notChecked' })).toBe(t.notChecked);
    expect(statusText({ kind: 'offline' })).toBe(t.offline);
    expect(statusText({ kind: 'checked', at: '2026-11-19T10:42:00Z' })).toMatch(
      /^Updates: checked /,
    );
    expect(statusText({ kind: 'paused', until: '2026-11-19T10:47:00Z' })).toMatch(
      /^Updates: paused until /,
    );
  });

  it('marks what is fine and what needs waiting out', () => {
    expect(statusDot({ kind: 'checked', at: '' })).toBe('success');
    expect(statusDot({ kind: 'offline' })).toBe('warning');
    expect(statusDot({ kind: 'paused', until: '' })).toBe('warning');
    expect(statusDot({ kind: 'off' })).toBe('none');
  });
});
