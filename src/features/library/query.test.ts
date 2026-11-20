import { beforeAll, describe, expect, it } from 'vitest';
import type { Api, IssueKind, ModEntry, ModKind, ModSource, Profile } from '../../api';
import { createMockApi } from '../../api/mock';
import {
  compareVersions,
  countByKind,
  defaultQuery,
  queryMods,
  worstSeverity,
  type LibraryQuery,
  type SortKey,
} from './query';

let counter = 0;

function makeMod(overrides: Partial<ModEntry> = {}): ModEntry {
  counter += 1;
  const fileName = overrides.fileName ?? `FS25_Mod${counter}.zip`;
  return {
    id: fileName,
    profileId: 'main',
    modName: fileName.replace(/\.zip$/, ''),
    fileName,
    format: 'zip',
    title: `Mod ${counter}`,
    author: 'Author',
    version: '1.0.0.0',
    descVersion: 106,
    kind: 'vehicle',
    description: '',
    icon: null,
    sizeBytes: 1000,
    modified: '2025-01-01T00:00:00Z',
    multiplayer: true,
    dependencies: [],
    issues: [],
    source: { type: 'unknown' },
    update: { type: 'unknown' },
    alsoIn: [],
    tags: [],
    note: null,
    links: [],
    ...overrides,
  };
}

const ids = (mods: readonly ModEntry[]) => mods.map((m) => m.id);
const query = (overrides: Partial<LibraryQuery>): LibraryQuery => ({
  ...defaultQuery,
  ...overrides,
});

describe('queryMods search', () => {
  const kestrel = makeMod({
    id: 'kestrel',
    title: 'Kestrel TX-840 Tractor',
    author: 'RustyHinge',
    modName: 'FS25_KestrelTX840',
    fileName: 'FS25_KestrelTX840.zip',
  });
  const refuel = makeMod({
    id: 'refuel',
    title: 'Easy Refuel',
    author: 'MudAnvil',
    modName: 'FS25_EasyRefuel',
    fileName: 'FS25_EasyRefuel.zip',
  });
  const barn = makeMod({
    id: 'barn',
    title: 'Hay Barn',
    author: 'RustyHinge',
    modName: 'FS25_HayBarn',
    fileName: 'FS25_HayBarn.zip',
  });
  const odd = makeMod({
    id: 'odd',
    title: 'Plain',
    author: 'Nobody',
    modName: 'SpecialName',
    fileName: 'other.zip',
    description: 'secretword',
  });
  const mods = [kestrel, refuel, barn, odd];
  const search = (text: string) =>
    ids(queryMods(mods, query({ search: text, sortKey: 'author' }))).sort();

  it('matches everything for an empty or blank search', () => {
    expect(search('')).toEqual(['barn', 'kestrel', 'odd', 'refuel']);
    expect(search('   ')).toEqual(['barn', 'kestrel', 'odd', 'refuel']);
  });

  it('requires every term to match some field', () => {
    expect(search('kestrel rusty')).toEqual(['kestrel']);
    expect(search('rusty zzz')).toEqual([]);
    expect(search('hay mud')).toEqual([]);
  });

  it('is case-insensitive and trims whitespace', () => {
    expect(search('RUSTY')).toEqual(['barn', 'kestrel']);
    expect(search('  hinge   TRACTOR ')).toEqual(['kestrel']);
  });

  it('searches title, author, mod name and file name but not the description', () => {
    expect(search('easy')).toEqual(['refuel']);
    expect(search('mudanvil')).toEqual(['refuel']);
    expect(search('specialname')).toEqual(['odd']);
    expect(search('other.zip')).toEqual(['odd']);
    expect(search('secretword')).toEqual([]);
  });

  it('matches substrings', () => {
    expect(search('tx84')).toEqual(['kestrel']);
  });
});

describe('queryMods filters', () => {
  const mods = [
    makeMod({ id: 'v1', kind: 'vehicle', title: 'V1' }),
    makeMod({
      id: 'v2',
      kind: 'vehicle',
      title: 'V2',
      issues: [{ kind: 'conflictCopy', severity: 'warning', detail: null }],
    }),
    makeMod({
      id: 'p1',
      kind: 'placeable',
      title: 'P1',
      update: { type: 'available', version: '2.0.0.0', installable: true },
    }),
    makeMod({
      id: 's1',
      kind: 'script',
      title: 'S1',
      issues: [{ kind: 'descVersionMismatch', severity: 'info', detail: null }],
      update: { type: 'available', version: '1.1.0.0', installable: true },
    }),
    makeMod({ id: 'm1', kind: 'map', title: 'M1', update: { type: 'upToDate' } }),
    makeMod({ id: 'o1', kind: 'other', title: 'O1' }),
  ];
  const run = (overrides: Partial<LibraryQuery>) => ids(queryMods(mods, query(overrides)));

  it('keeps everything by default', () => {
    expect(run({})).toEqual(['m1', 'o1', 'p1', 's1', 'v1', 'v2']);
  });

  it.each<[ModKind, string[]]>([
    ['vehicle', ['v1', 'v2']],
    ['placeable', ['p1']],
    ['script', ['s1']],
    ['map', ['m1']],
    ['other', ['o1']],
  ])('filters by kind %s', (kind, expected) => {
    expect(run({ kind })).toEqual(expected);
  });

  it('filters to mods with issues', () => {
    expect(run({ status: 'issues' })).toEqual(['s1', 'v2']);
  });

  it('filters to mods with an update available', () => {
    expect(run({ status: 'updates' })).toEqual(['p1', 's1']);
  });

  it('filters by tag without regard to case', () => {
    const tagged = [
      makeMod({ id: 't1', title: 'T1', tags: ['Favourite', 'Keep'] }),
      makeMod({ id: 't2', title: 'T2', tags: ['favourite'] }),
      makeMod({ id: 't3', title: 'T3', tags: ['Keep'] }),
      makeMod({ id: 't4', title: 'T4' }),
    ];
    const runTagged = (tag: string) => ids(queryMods(tagged, query({ tag })));
    expect(runTagged('')).toEqual(['t1', 't2', 't3', 't4']);
    expect(runTagged('Favourite')).toEqual(['t1', 't2']);
    expect(runTagged('FAVOURITE')).toEqual(['t1', 't2']);
    expect(runTagged('keep')).toEqual(['t1', 't3']);
    expect(runTagged('nothing')).toEqual([]);
    expect(ids(queryMods(tagged, query({ tag: 'keep', search: 't3' })))).toEqual(['t3']);
  });

  it('combines kind, status and search', () => {
    expect(run({ kind: 'script', status: 'updates' })).toEqual(['s1']);
    expect(run({ kind: 'vehicle', status: 'updates' })).toEqual([]);
    expect(run({ kind: 'vehicle', status: 'issues', search: 'v' })).toEqual(['v2']);
    expect(run({ status: 'issues', search: 'p1' })).toEqual([]);
  });
});

describe('queryMods sorting', () => {
  const src = (type: 'modHub' | 'gitHub' | 'manual' | 'unknown'): ModSource =>
    type === 'modHub'
      ? { type, id: '1' }
      : type === 'gitHub'
        ? { type, owner: 'o', repo: 'r' }
        : type === 'manual'
          ? { type, host: 'example.com' }
          : { type };
  const a = makeMod({
    id: 'a',
    title: 'Delta Plow',
    author: 'zed',
    version: '1.10.0.0',
    kind: 'vehicle',
    sizeBytes: 300,
    modified: '2025-03-01T00:00:00Z',
    source: src('modHub'),
  });
  const b = makeMod({
    id: 'b',
    title: 'alpha Seeder',
    author: 'Amy',
    version: '1.9.0.0',
    kind: 'script',
    sizeBytes: 100,
    modified: '2024-12-01T00:00:00Z',
    source: src('gitHub'),
  });
  const c = makeMod({
    id: 'c',
    title: 'Charlie Barn',
    author: 'bob',
    version: '2.0.0.0',
    kind: 'placeable',
    sizeBytes: 200,
    modified: '2026-01-01T00:00:00Z',
    source: src('manual'),
  });
  const d = makeMod({
    id: 'd',
    title: 'Bravo Map',
    author: 'Carl',
    version: '1.0.0.0',
    kind: 'map',
    sizeBytes: 400,
    modified: '2025-06-01T00:00:00Z',
    source: src('unknown'),
  });
  const mods = [a, b, c, d];

  const expectedAsc: Record<SortKey, string[]> = {
    title: ['b', 'd', 'c', 'a'],
    author: ['b', 'c', 'd', 'a'],
    version: ['d', 'b', 'a', 'c'],
    kind: ['d', 'c', 'b', 'a'],
    size: ['b', 'c', 'a', 'd'],
    modified: ['b', 'a', 'd', 'c'],
    source: ['b', 'c', 'a', 'd'],
  };

  it.each(Object.entries(expectedAsc) as [SortKey, string[]][])(
    'sorts by %s in both directions',
    (sortKey, expected) => {
      expect(ids(queryMods(mods, query({ sortKey, sortDirection: 'asc' })))).toEqual(expected);
      expect(ids(queryMods(mods, query({ sortKey, sortDirection: 'desc' })))).toEqual(
        [...expected].reverse(),
      );
    },
  );

  it('sorts titles case-insensitively and numbers by value', () => {
    const list = [makeMod({ id: 'x10', title: 'Mod 10' }), makeMod({ id: 'x2', title: 'Mod 2' })];
    expect(ids(queryMods(list, defaultQuery))).toEqual(['x2', 'x10']);
  });

  it('breaks ties by title then id, ascending in both directions', () => {
    const tied = [
      makeMod({ id: 'b', title: 'Same', kind: 'vehicle' }),
      makeMod({ id: 'a', title: 'Same', kind: 'vehicle' }),
      makeMod({ id: 'c', title: 'Apple', kind: 'vehicle' }),
      makeMod({ id: 'y', title: 'Zed', kind: 'script' }),
    ];
    expect(ids(queryMods(tied, query({ sortKey: 'kind', sortDirection: 'asc' })))).toEqual([
      'y',
      'c',
      'a',
      'b',
    ]);
    expect(ids(queryMods(tied, query({ sortKey: 'kind', sortDirection: 'desc' })))).toEqual([
      'c',
      'a',
      'b',
      'y',
    ]);
  });

  it('does not mutate the input', () => {
    const input = Object.freeze([...mods]);
    const before = ids(input);
    queryMods(input, query({ sortKey: 'size', sortDirection: 'desc', search: 'a' }));
    expect(ids(input)).toEqual(before);
  });
});

describe('compareVersions', () => {
  it('compares numerically, not as text', () => {
    expect(compareVersions('1.10.0.0', '1.9.0.0')).toBeGreaterThan(0);
    expect(compareVersions('1.9.0.0', '1.10.0.0')).toBeLessThan(0);
    expect(compareVersions('2.0.0.0', '1.99.99.99')).toBeGreaterThan(0);
  });

  it('returns 0 for equal versions', () => {
    expect(compareVersions('1.0.0.0', '1.0.0.0')).toBe(0);
  });

  it('treats missing trailing parts as zero', () => {
    expect(compareVersions('1.0', '1.0.0.0')).toBe(0);
    expect(compareVersions('1.0.0.1', '1.0')).toBeGreaterThan(0);
    expect(compareVersions('1', '1.0.0.1')).toBeLessThan(0);
  });

  it('compares non-numeric parts as strings', () => {
    expect(compareVersions('1.0.0.a', '1.0.0.b')).toBeLessThan(0);
    expect(compareVersions('1.0.0.b', '1.0.0.a')).toBeGreaterThan(0);
    expect(compareVersions('1.0.0.beta', '1.0.0.beta')).toBe(0);
  });
});

describe('countByKind', () => {
  it('counts every kind, including empty ones', () => {
    expect(countByKind([])).toEqual({ map: 0, vehicle: 0, placeable: 0, script: 0, other: 0 });
    const mods = [
      makeMod({ kind: 'vehicle' }),
      makeMod({ kind: 'vehicle' }),
      makeMod({ kind: 'map' }),
      makeMod({ kind: 'other' }),
    ];
    expect(countByKind(mods)).toEqual({ map: 1, vehicle: 2, placeable: 0, script: 0, other: 1 });
  });
});

describe('worstSeverity', () => {
  const withIssues = (...severities: ('error' | 'warning' | 'info')[]) =>
    makeMod({
      issues: severities.map((severity) => ({ kind: 'nestedArchive', severity, detail: null })),
    });

  it('is null without issues', () => {
    expect(worstSeverity(makeMod())).toBeNull();
  });

  it('ranks error above warning above info', () => {
    expect(worstSeverity(withIssues('info'))).toBe('info');
    expect(worstSeverity(withIssues('info', 'warning'))).toBe('warning');
    expect(worstSeverity(withIssues('warning', 'error', 'info'))).toBe('error');
  });
});

describe('mock dataset', () => {
  const GIB = 1024 ** 3;
  const issueKinds: IssueKind[] = [
    'notAMod',
    'invalidFileName',
    'nestedArchive',
    'zipAndFolderTwin',
    'conflictCopy',
    'descVersionMismatch',
    'missingDependency',
  ];
  const api: Api = createMockApi({ delayMs: 0, updatesOn: true });
  const stressApi: Api = createMockApi({ delayMs: 0, stress: true, updatesOn: true });
  let profiles: Profile[] = [];
  let mods: Record<string, ModEntry[]> = {};
  let stressProfiles: Profile[] = [];
  let stressMods: ModEntry[] = [];

  beforeAll(async () => {
    profiles = await api.listProfiles();
    mods = Object.fromEntries(
      await Promise.all(profiles.map(async (p) => [p.id, await api.listMods(p.id)] as const)),
    );
    stressProfiles = await stressApi.listProfiles();
    stressMods = await stressApi.listMods('stress');
  });

  const main = () => mods['main'] ?? [];

  it('has the expected profiles and mod counts', () => {
    expect(profiles.map((p) => [p.id, p.name, p.modCount])).toEqual([
      ['main', 'Main Farm', 590],
      ['testing', 'Testing', 16],
      ['offline', 'Offline Testing', 6],
    ]);
    expect(profiles.map((p) => [p.isActive, p.isGameDefault])).toEqual([
      [false, true],
      [true, false],
      [false, false],
    ]);
    expect(stressProfiles.map((p) => p.id)).toEqual(['main', 'testing', 'offline', 'stress']);
    expect(stressProfiles.find((p) => p.id === 'stress')?.modCount).toBe(5000);
    expect(stressMods).toHaveLength(5000);
  });

  it('keeps profile summaries equal to the mods returned', () => {
    for (const profile of [...profiles, ...stressProfiles.filter((p) => p.id === 'stress')]) {
      const list = profile.id === 'stress' ? stressMods : (mods[profile.id] ?? []);
      expect(profile.modCount).toBe(list.length);
      expect(profile.totalBytes).toBe(list.reduce((sum, m) => sum + m.sizeBytes, 0));
      expect(profile.issueCount).toBe(list.filter((m) => m.issues.length > 0).length);
      expect(profile.updateCount).toBe(list.filter((m) => m.update.type === 'available').length);
    }
  });

  it('looks like a real collection in the main profile', () => {
    const total = main().reduce((sum, m) => sum + m.sizeBytes, 0);
    expect(total).toBeGreaterThan(22 * GIB);
    expect(total).toBeLessThan(25 * GIB);
    const counts = countByKind(main());
    expect(counts.map).toBe(6);
    expect(counts.vehicle).toBeGreaterThan(300);
    expect(main().filter((m) => m.format === 'folder').length).toBeGreaterThanOrEqual(4);
    const maps = main().filter((m) => m.kind === 'map');
    expect(maps.every((m) => m.sizeBytes >= 400 * 1024 ** 2 && m.sizeBytes <= 3.95 * GIB)).toBe(
      true,
    );
    expect(maps.filter((m) => m.sizeBytes > 3.8 * GIB)).toHaveLength(1);
    expect(maps.every((m) => m.dependencies.length >= 3 && m.dependencies.length <= 10)).toBe(true);
    for (const mod of main()) {
      expect(mod.modified >= '2024-11-12' && mod.modified <= '2026-11-16').toBe(true);
    }
  });

  it('includes every issue kind at least twice in the main profile', () => {
    for (const kind of issueKinds) {
      const count = main().filter((m) => m.issues.some((i) => i.kind === kind)).length;
      expect(count, kind).toBeGreaterThanOrEqual(2);
    }
  });

  it('gives each profile an issue and an update to show', () => {
    for (const profile of profiles) {
      expect(profile.issueCount, profile.id).toBeGreaterThan(0);
      expect(profile.updateCount, profile.id).toBeGreaterThan(0);
    }
  });

  it('has unique ids and mod names that match file names', () => {
    for (const list of [...Object.values(mods), stressMods]) {
      expect(new Set(list.map((m) => m.id)).size).toBe(list.length);
      for (const mod of list) {
        expect(mod.id).toBe(mod.fileName);
        expect(mod.modName).toBe(
          mod.format === 'zip' ? mod.fileName.replace(/\.zip$/, '') : mod.fileName,
        );
      }
    }
  });

  it('is deterministic', async () => {
    const other = createMockApi({ delayMs: 0, updatesOn: true });
    expect(await other.listProfiles()).toEqual(profiles);
    for (const profile of profiles) {
      expect(await other.listMods(profile.id)).toEqual(mods[profile.id]);
    }
    expect(await other.getGameStatus()).toEqual(await api.getGameStatus());
  });

  it('keeps alsoIn symmetric', () => {
    for (const [profileId, list] of Object.entries(mods)) {
      for (const mod of list) {
        expect(mod.alsoIn).not.toContain(profileId);
        for (const otherId of mod.alsoIn) {
          const twin = (mods[otherId] ?? []).find((m) => m.modName === mod.modName);
          expect(twin?.alsoIn, `${mod.modName} in ${otherId}`).toContain(profileId);
        }
      }
    }
    expect(main().filter((m) => m.alsoIn.includes('testing')).length).toBeGreaterThanOrEqual(10);
    expect(main().filter((m) => m.alsoIn.includes('offline')).length).toBeGreaterThanOrEqual(4);
    const shared = (mods['testing'] ?? []).filter((m) => m.alsoIn.includes('main'));
    expect(
      shared.some((m) => main().find((x) => x.modName === m.modName)?.version !== m.version),
    ).toBe(true);
  });

  it('only offers updates for tracked sources, with a higher version', () => {
    for (const mod of [...Object.values(mods).flat(), ...stressMods]) {
      if (mod.update.type !== 'available') continue;
      expect(['modHub', 'gitHub']).toContain(mod.source.type);
      expect(compareVersions(mod.update.version, mod.version)).toBeGreaterThan(0);
    }
  });

  it('keeps dependencies consistent with their profile', () => {
    for (const list of [...Object.values(mods), stressMods]) {
      const names = new Set(list.map((m) => m.modName));
      for (const mod of list) {
        for (const dep of mod.dependencies) {
          expect(names.has(dep.modName)).toBe(dep.satisfied);
        }
        const unsatisfied = mod.dependencies.filter((d) => !d.satisfied);
        const issue = mod.issues.find((i) => i.kind === 'missingDependency');
        expect(Boolean(issue)).toBe(unsatisfied.length > 0);
        for (const dep of unsatisfied) expect(issue?.detail).toContain(dep.modName);
      }
    }
  });

  it('makes not-a-mod entries empty and gives icons that are small data URIs', () => {
    for (const mod of main()) {
      if (mod.issues.some((i) => i.kind === 'notAMod')) {
        expect([mod.author, mod.version, mod.descVersion, mod.kind, mod.icon, mod.title]).toEqual([
          '',
          '',
          null,
          'other',
          null,
          mod.fileName,
        ]);
      }
      if (mod.icon) {
        expect(mod.icon.startsWith('data:image/svg+xml,')).toBe(true);
        expect(mod.icon.length).toBeLessThan(900);
      }
    }
    const withIcon = main().filter((m) => m.icon !== null).length;
    expect(withIcon / main().length).toBeGreaterThan(0.88);
  });

  it('pairs a zip with a folder of the same name for zipAndFolderTwin', () => {
    const twins = main().filter((m) => m.issues.some((i) => i.kind === 'zipAndFolderTwin'));
    for (const mod of twins) {
      const partner = twins.find((m) => m.modName === mod.modName && m.format !== mod.format);
      expect(partner, mod.fileName).toBeDefined();
    }
  });

  it('reports the game status and the testing path as the effective folder', async () => {
    const status = await api.getGameStatus();
    expect(status).toEqual({
      gameVersion: '1.25.1.0',
      gameRunning: false,
      serverRunning: false,
      settingsReadable: true,
      overrideActive: true,
      effectiveModsDir: profiles.find((p) => p.id === 'testing')?.path,
      loggedDescVersion: 106,
      store: 'steam',
      launchable: true,
      userDirSource: 'detected',
      userDir: 'C:\\Users\\Player\\Documents\\My Games\\FarmingSimulator2025',
    });
  });

  it('resolves to an empty list for an unknown profile and rejects when asked to fail', async () => {
    expect(await api.listMods('nope')).toEqual([]);
    await expect(createMockApi({ delayMs: 0, failMods: true }).listMods('main')).rejects.toThrow(
      Error,
    );
  });

  it('queries the 5,000-mod profile quickly', () => {
    const queries: LibraryQuery[] = [
      defaultQuery,
      query({ search: 'brannock tractor', sortKey: 'size', sortDirection: 'desc' }),
      query({ sortKey: 'version' }),
      query({ sortKey: 'author', status: 'issues' }),
    ];
    for (const q of queries) queryMods(stressMods, q); // warm-up
    for (const q of queries) {
      const start = performance.now();
      queryMods(stressMods, q);
      expect(performance.now() - start, JSON.stringify(q)).toBeLessThan(50);
    }
  });
});
