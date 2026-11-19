import { describe, expect, it } from 'vitest';
import { compareVersions, isOlder } from './version';

describe('compareVersions', () => {
  it('compares numerically, part by part', () => {
    expect(compareVersions('1.10.0.0', '1.9.0.0')).toBeGreaterThan(0);
    expect(compareVersions('1.2.0.0', '1.2.0.1')).toBeLessThan(0);
  });

  it('drops a leading v and accepts other separators', () => {
    expect(compareVersions('v8.1.0.3', '8.1.0.3')).toBe(0);
    expect(compareVersions('V.1.1.5.0', '1.1.4.9')).toBeGreaterThan(0);
    expect(compareVersions('1-2_3+4', '1.2.3.4')).toBe(0);
  });

  it('treats missing trailing parts as 0', () => {
    expect(compareVersions('1.2', '1.2.0.0')).toBe(0);
    expect(compareVersions('1.2', '1.2.0.1')).toBeLessThan(0);
  });

  it('tells whether one version is older', () => {
    expect(isOlder('1.0.0.0', '1.0.0.1')).toBe(true);
    expect(isOlder('1.0.0.1', '1.0.0.1')).toBe(false);
  });
});
