// What the update status item says, derived from the settings and what the last check did.

import type { ErrorCode, UpdateSettings } from '../../api';
import { formatTime } from '../../lib/format';
import { strings } from '../../strings';

export type UpdateStatus =
  | { kind: 'off' }
  | { kind: 'checking' }
  | { kind: 'notChecked' }
  | { kind: 'checked'; at: string }
  | { kind: 'offline' }
  | { kind: 'paused'; until: string };

export interface StatusInput {
  /** Null until the settings are read. */
  settings: UpdateSettings | null;
  checking: boolean;
  /** The code the last check or switch-on failed with, until a check succeeds. */
  failure: ErrorCode | null;
}

/** Null while the settings are unknown. A retry-after in the past no longer pauses anything. */
export function deriveStatus(
  { settings, checking, failure }: StatusInput,
  now: number,
): UpdateStatus | null {
  if (settings === null) return null;
  if (!settings.enabled) return { kind: 'off' };
  if (checking) return { kind: 'checking' };
  if (settings.retryAfter !== null && Date.parse(settings.retryAfter) > now) {
    return { kind: 'paused', until: settings.retryAfter };
  }
  if (failure === 'offline') return { kind: 'offline' };
  if (settings.lastCheck !== null) return { kind: 'checked', at: settings.lastCheck };
  return { kind: 'notChecked' };
}

const t = strings.updates.status;

export function statusText(status: UpdateStatus): string {
  switch (status.kind) {
    case 'off':
      return t.off;
    case 'checking':
      return t.checking;
    case 'notChecked':
      return t.notChecked;
    case 'checked':
      return t.checked(formatTime(status.at));
    case 'offline':
      return t.offline;
    case 'paused':
      return t.paused(formatTime(status.until));
  }
}

/** The dot beside the text: green when current, amber when something needs waiting out. */
export function statusDot(status: UpdateStatus): 'success' | 'warning' | 'on' | 'none' {
  switch (status.kind) {
    case 'checked':
      return 'success';
    case 'offline':
    case 'paused':
      return 'warning';
    case 'checking':
      return 'on';
    case 'off':
    case 'notChecked':
      return 'none';
  }
}
