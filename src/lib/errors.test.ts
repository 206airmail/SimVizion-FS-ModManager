import { describe, expect, it } from 'vitest';
import type { ErrorCode } from '../api';
import { strings } from '../strings';
import { ApiFailure, describeError, isErrorCode } from './errors';

const codes: ErrorCode[] = ['profileNotFound', 'folderMissing', 'folderUnreadable', 'internal'];

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
