import { describe, expect, it } from 'vitest';
import type { ModEntry, UpdatePlan } from '../../api';
import { defaultFolders, installableUpdates, isCurrent, previewHost } from './choices';

const plan: UpdatePlan = {
  modName: 'FS25_Tractor',
  toVersion: '1.3.0.0',
  source: 'gitHub',
  folders: [
    { profileId: 'main', modId: 'FS25_Tractor.zip', version: '1.2.0.0' },
    { profileId: 'testing', modId: 'FS25_Tractor.zip', version: '1.3.0.0' },
    { profileId: 'offline', modId: 'FS25_Tractor.zip', version: '1.1.0.0' },
  ],
};

describe('defaultFolders', () => {
  it('ticks the folder on screen and every older copy, but not a current one', () => {
    expect(defaultFolders(plan, 'main')).toEqual(['main', 'offline']);
  });

  it('keeps the folder on screen ticked even when it is the one that is current', () => {
    expect(defaultFolders(plan, 'testing')).toEqual(['main', 'testing', 'offline']);
  });

  it('ticks only older copies when the folder on screen does not hold the mod', () => {
    expect(defaultFolders(plan, 'elsewhere')).toEqual(['main', 'offline']);
  });
});

describe('isCurrent', () => {
  it('is true from the new version up', () => {
    const [main, testing] = plan.folders;
    expect(isCurrent(testing!, plan.toVersion)).toBe(true);
    expect(isCurrent(main!, plan.toVersion)).toBe(false);
    expect(isCurrent({ ...main!, version: '2.0' }, plan.toVersion)).toBe(true);
  });
});

describe('installableUpdates', () => {
  const make = (update: ModEntry['update']) => ({ update }) as ModEntry;
  it('keeps only mods with an installable update', () => {
    const mods = [
      make({ type: 'available', version: '2', installable: true }),
      make({ type: 'available', version: '2', installable: false }),
      make({ type: 'upToDate' }),
      make({ type: 'unknown' }),
    ];
    expect(installableUpdates(mods)).toEqual([mods[0]]);
  });
});

describe('previewHost', () => {
  it('names the host of an http or https address', () => {
    expect(previewHost(' https://www.kingmods.net/en/fs25/mods/1 ')).toBe('www.kingmods.net');
    expect(previewHost('http://example.org:8080/x')).toBe('example.org');
  });

  it('says nothing for text that is not such an address yet', () => {
    for (const text of [
      '',
      'www.example.org',
      'https:',
      'ftp://example.org',
      'javascript:alert(1)',
    ]) {
      expect(previewHost(text), text).toBeNull();
    }
  });
});
