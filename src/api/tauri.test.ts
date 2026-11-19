import { beforeEach, describe, expect, it, vi } from 'vitest';

const { invoke, channels } = vi.hoisted(() => ({
  invoke: vi.fn(),
  channels: [] as { onmessage: (message: unknown) => void }[],
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke,
  Channel: class {
    onmessage: (message: unknown) => void = () => {};
    constructor() {
      channels.push(this);
    }
  },
  isTauri: () => true,
}));

import { ApiFailure, describeError } from '../lib/errors';
import { strings } from '../strings';
import { createTauriApi, toApiFailure } from './tauri';

beforeEach(() => {
  invoke.mockReset();
  channels.length = 0;
});

describe('toApiFailure', () => {
  it('turns an ApiError object into an ApiFailure with its code and detail', () => {
    const failure = toApiFailure({ code: 'folderMissing', detail: 'mods' });
    expect(failure).toBeInstanceOf(ApiFailure);
    expect(failure).toBeInstanceOf(Error);
    expect(failure.code).toBe('folderMissing');
    expect(failure.detail).toBe('mods');
  });

  it('accepts an ApiError without detail', () => {
    const failure = toApiFailure({ code: 'profileNotFound', detail: null });
    expect([failure.code, failure.detail]).toEqual(['profileNotFound', null]);
  });

  it('makes anything else an internal failure', () => {
    expect(toApiFailure(new Error('bridge down')).code).toBe('internal');
    expect(toApiFailure('plain string').code).toBe('internal');
    expect(toApiFailure(undefined).code).toBe('internal');
    expect(toApiFailure({ code: 'notARealCode', detail: null }).code).toBe('internal');
    expect(toApiFailure({ message: 'no code' }).code).toBe('internal');
  });

  it('returns an existing ApiFailure unchanged', () => {
    const failure = new ApiFailure('folderUnreadable');
    expect(toApiFailure(failure)).toBe(failure);
  });
});

describe('createTauriApi', () => {
  it('is not sample data', () => {
    expect(createTauriApi().sampleData).toBe(false);
  });

  it('rejects with a worded ApiFailure when a command fails', async () => {
    invoke.mockRejectedValue({ code: 'folderMissing', detail: null });
    const error = await createTauriApi()
      .listProfiles()
      .catch((e: unknown) => e);
    expect(error).toBeInstanceOf(ApiFailure);
    expect(describeError(error)).toBe(strings.errors.folderMissing);
  });

  it('calls each command with the documented name and arguments', async () => {
    invoke.mockResolvedValue(null);
    const api = createTauriApi();
    await api.getGameStatus();
    await api.listProfiles();
    await api.addProfile();
    await api.openProfileFolder('main');
    await api.showModInFolder('main', 'FS25_A.zip');
    expect(invoke.mock.calls).toEqual([
      ['get_game_status', undefined],
      ['list_profiles', undefined],
      ['add_profile', undefined],
      ['open_profile_folder', { profileId: 'main' }],
      ['show_mod_in_folder', { profileId: 'main', modId: 'FS25_A.zip' }],
    ]);
  });

  it('forwards scan progress from the channel to the caller', async () => {
    invoke.mockResolvedValue([]);
    const seen: unknown[] = [];
    await createTauriApi().listMods('main', (progress) => seen.push(progress));
    const [command, args] = invoke.mock.calls[0] ?? [];
    expect(command).toBe('list_mods');
    expect(args).toMatchObject({ profileId: 'main' });
    expect(args.onProgress).toBe(channels[0]);
    channels[0]?.onmessage({ profileId: 'main', done: 120, total: 590 });
    expect(seen).toEqual([{ profileId: 'main', done: 120, total: 590 }]);
  });

  it('accepts progress messages when no one is listening', async () => {
    invoke.mockResolvedValue([]);
    await createTauriApi().listMods('main');
    expect(() => channels[0]?.onmessage({ profileId: 'main', done: 1, total: 2 })).not.toThrow();
  });
});
