import { describe, expect, it } from 'vitest';
import { PROFILE_NAME_MAX, validateProfileName } from './profileName';

describe('validateProfileName', () => {
  it('accepts an ordinary name and trims before judging', () => {
    expect(validateProfileName('Winter Test')).toBeNull();
    expect(validateProfileName('  Winter Test  ')).toBeNull();
  });

  it('rejects an empty name or one that is only spaces', () => {
    expect(validateProfileName('')).toBe('empty');
    expect(validateProfileName('   \t')).toBe('empty');
  });

  it('allows exactly 100 characters and rejects 101', () => {
    expect(PROFILE_NAME_MAX).toBe(100);
    expect(validateProfileName('x'.repeat(100))).toBeNull();
    expect(validateProfileName('x'.repeat(101))).toBe('tooLong');
  });

  it('counts characters, not UTF-16 units', () => {
    expect(validateProfileName('🚜'.repeat(100))).toBeNull();
    expect(validateProfileName('🚜'.repeat(101))).toBe('tooLong');
  });
});
