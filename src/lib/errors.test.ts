import { describe, expect, it } from 'vitest';
import type { ErrorCode } from '../api';
import { strings } from '../strings';
import { ApiFailure, describeError, isErrorCode } from './errors';

// A Record forces this list to name every generated ErrorCode: a new code fails the type check
// until it is listed here (and worded in strings.errors).
const allCodes: Record<ErrorCode, true> = {
  profileNotFound: true,
  folderMissing: true,
  folderUnreadable: true,
  gameRunning: true,
  serverRunning: true,
  settingsMissing: true,
  settingsUnreadable: true,
  settingsChanged: true,
  writeFailed: true,
  writesDisabled: true,
  nothingToUndo: true,
  noBackup: true,
  recycleUnavailable: true,
  notEnoughSpace: true,
  busy: true,
  cancelled: true,
  targetChanged: true,
  cannotRemoveProfile: true,
  savegameMissing: true,
  savegameUnreadable: true,
  offline: true,
  rateLimited: true,
  updatesDisabled: true,
  verifyFailed: true,
  notInstallable: true,
  invalidLink: true,
  notLaunchable: true,
  invalidGameFolder: true,
  appUpdateFailed: true,
  replaceIncomplete: true,
  runningUnknown: true,
  internal: true,
};
const codes = Object.keys(allCodes) as ErrorCode[];

describe('error wording', () => {
  it('has calm, distinct wording for every error code and no stray codes', () => {
    expect(Object.keys(strings.errors).sort()).toEqual([...codes].sort());
    for (const code of codes) {
      const text = strings.errors[code];
      expect(text.length, code).toBeGreaterThan(20);
      expect(text, code).not.toContain(code);
      expect(text, code).not.toMatch(/!|\bERROR\b/);
    }
  });

  it('says nothing was changed for every refusal that leaves the file alone', () => {
    const untouched: ErrorCode[] = [
      'gameRunning',
      'serverRunning',
      'settingsMissing',
      'settingsUnreadable',
      'settingsChanged',
      'writeFailed',
      'writesDisabled',
      'nothingToUndo',
      'noBackup',
    ];
    for (const code of untouched) {
      expect(strings.errors[code], code).toMatch(
        /nothing was (changed|written|restored)|original file is unchanged/,
      );
    }
  });
});

describe('describeError', () => {
  it('words each error code with its own message', () => {
    const messages = codes.map((code) => describeError(new ApiFailure(code)));
    for (const [index, code] of codes.entries()) {
      expect(messages[index], code).toBe(strings.errors[code]);
    }
    expect(new Set(messages).size).toBe(codes.length);
  });

  it('does not show the detail, which is data for logs', () => {
    expect(describeError(new ApiFailure('folderMissing', 'C:\\secret\\path'))).toBe(
      strings.errors.folderMissing,
    );
  });

  it('treats a plain Error as internal and never shows its message', () => {
    expect(describeError(new Error('boom'))).toBe(strings.errors.internal);
  });

  it('treats a value that is not an Error as internal', () => {
    for (const value of ['text', 42, null, undefined, { code: 'folderMissing' }]) {
      expect(describeError(value)).toBe(strings.errors.internal);
    }
  });

  it('treats an ApiFailure with an unknown code as internal', () => {
    expect(describeError(new ApiFailure('brandNew' as ErrorCode))).toBe(strings.errors.internal);
  });
});

describe('ApiFailure', () => {
  it('is an Error carrying its code and detail', () => {
    const failure = new ApiFailure('folderUnreadable', 'denied');
    expect(failure).toBeInstanceOf(Error);
    expect(failure.code).toBe('folderUnreadable');
    expect(failure.detail).toBe('denied');
    expect(new ApiFailure('internal').detail).toBeNull();
  });
});

describe('isErrorCode', () => {
  it('accepts exactly the known codes', () => {
    for (const code of codes) expect(isErrorCode(code)).toBe(true);
    for (const value of ['nope', '', 'toString', 7, null]) expect(isErrorCode(value)).toBe(false);
  });
});
