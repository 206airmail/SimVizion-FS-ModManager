import { describe, expect, it } from 'vitest';
import type { HashProgress, SavegameReport } from '../../api';
import { ApiFailure } from '../../lib/errors';
import { createMockApi, type MockOptions } from '../../api/mock';
import { startReportRun, type HashPhase, type RunEvents } from './reportRunner';

const pending = (report: SavegameReport) =>
  report.rows.filter((row) => row.state === 'hashPending').length;

/** Records what a run delivers, in order. */
function recorder() {
  const log: string[] = [];
  const reports: SavegameReport[] = [];
  const progress: HashProgress[] = [];
  const phases: HashPhase[] = [];
  const failures: unknown[] = [];
  const events: RunEvents = {
    onReport: (report) => {
      reports.push(report);
      log.push(`report:${report.savegameId}:${report.profileId}`);
    },
    onReportFailed: (error) => {
      failures.push(error);
      log.push('reportFailed');
    },
    onHash: (phase) => {
      phases.push(phase);
      log.push(`hash:${phase}`);
    },
    onProgress: (p) => progress.push(p),
  };
  return { events, log, reports, progress, phases, failures };
}

const make = (options: MockOptions = {}) => createMockApi({ delayMs: 1, ...options });

describe('startReportRun', () => {
  it('delivers the first report, then the final one with the files checked', async () => {
    const api = make();
    const r = recorder();
    await startReportRun(api, 'savegame1', 'main', r.events).done;
    expect(r.log.slice(0, 3)).toEqual([
      'report:savegame1:main',
      'hash:running',
      'report:savegame1:main',
    ]);
    expect(r.log.at(-1)).toBe('hash:idle');
    expect(pending(r.reports[0] as SavegameReport)).toBeGreaterThan(0);
    expect(pending(r.reports[1] as SavegameReport)).toBe(0);
    expect(r.progress.length).toBeGreaterThan(1);
  });

  it('does not check the files again when none is pending', async () => {
    const api = make();
    await startReportRun(api, 'savegame1', 'main', recorder().events).done;
    const r = recorder();
    await startReportRun(api, 'savegame1', 'main', r.events).done;
    expect(r.log).toEqual(['report:savegame1:main', 'hash:idle']);
    expect(r.progress).toEqual([]);
  });

  it('reports a failure to read the savegame, and stays quiet otherwise', async () => {
    const r = recorder();
    await startReportRun(make({ brokenSave: true }), 'savegame9', 'main', r.events).done;
    expect(r.log).toEqual(['reportFailed']);
    expect((r.failures[0] as ApiFailure).code).toBe('savegameUnreadable');
  });

  it('keeps the first report when the check is cancelled', async () => {
    const api = make({ slowHash: true });
    const r = recorder();
    const run = startReportRun(api, 'savegame1', 'main', r.events);
    await new Promise((resolve) => setTimeout(resolve, 150));
    await api.cancelHashing();
    await run.done;
    expect(r.log).toEqual(['report:savegame1:main', 'hash:running', 'hash:cancelled']);
    expect(r.reports).toHaveLength(1);
  });

  it('never delivers anything after it was stopped, and frees the core for the next run', async () => {
    const api = make({ slowHash: true });
    const old = recorder();
    const first = startReportRun(api, 'savegame1', 'main', old.events);
    await new Promise((resolve) => setTimeout(resolve, 150));
    const beforeStop = old.log.length;
    first.stop();

    // The next run (another save and folder) starts at once; it must not hit `busy`.
    const next = recorder();
    const second = startReportRun(api, 'savegame4', 'testing', next.events);
    await new Promise((resolve) => setTimeout(resolve, 150));
    second.stop();
    await Promise.all([first.done, second.done]);

    expect(old.log.length).toBe(beforeStop);
    expect(old.reports.every((report) => report.savegameId === 'savegame1')).toBe(true);
    expect(next.log[0]).toBe('report:savegame4:testing');
    expect(next.log).not.toContain('hash:failed');
    expect(next.reports.every((report) => report.savegameId === 'savegame4')).toBe(true);
  });

  it('ignores a slow report that arrives after a newer run was started', async () => {
    // Two runs against the same instance: the first is stopped before its report arrives.
    const api = make();
    const old = recorder();
    const first = startReportRun(api, 'savegame1', 'main', old.events);
    first.stop();
    const next = recorder();
    await startReportRun(api, 'savegame7', 'testing', next.events).done;
    await first.done;
    expect(old.log).toEqual([]);
    expect(next.reports.at(-1)?.savegameId).toBe('savegame7');
  });
});
