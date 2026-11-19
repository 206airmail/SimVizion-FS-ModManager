// How a failed Api call reaches the screen: every rejection becomes an ApiFailure (a code plus
// optional data), and `describeError` words it from `strings.errors`.

import type { ErrorCode } from '../api';
import { strings } from '../strings';

/** An Api call that failed. `code` picks the wording; `detail` is data for logs, never shown. */
export class ApiFailure extends Error {
  readonly code: ErrorCode;
  readonly detail: string | null;

  constructor(code: ErrorCode, detail: string | null = null) {
    super(detail === null ? code : `${code}: ${detail}`);
    this.name = 'ApiFailure';
    this.code = code;
    this.detail = detail;
  }
}

export function isErrorCode(value: unknown): value is ErrorCode {
  return typeof value === 'string' && Object.hasOwn(strings.errors, value);
}

/** The text to show for anything a call threw. Only a known code gets specific wording. */
export function describeError(error: unknown): string {
  const code = error instanceof ApiFailure && isErrorCode(error.code) ? error.code : 'internal';
  return strings.errors[code];
}
