import { FolderX, RefreshCw, Save, SearchX } from 'lucide-react';
import { useMemo, useState } from 'react';
import type { GameStatus, Profile, SaveModRow } from '../../api';
import { api } from '../../api/client';
import { describeError } from '../../lib/errors';
import { useActionError } from '../../lib/useActionError';
import { useAsync } from '../../lib/useAsync';
import { strings } from '../../strings';
import { Button, IconButton } from '../../ui/Button';
import { Banner, EmptyState } from '../../ui/Feedback';
import { SegmentedControl } from '../../ui/SegmentedControl';
import { Select, type SelectOption } from '../../ui/Select';
import { ActionErrorBanner } from '../ActionErrorBanner';
import { fileOpBlockedReason } from '../files/blocked';
import { TransferDialog, type TransferPreset } from '../files/TransferDialog';
import type { FileOpsController } from '../files/useFileOps';
import { ExtrasDisclosure } from './ExtrasDisclosure';
import { HashNotice } from './HashNotice';
import { MissingActions } from './MissingActions';
import {
  copyGroups,
  countReport,
  filterReport,
  foundNowhere,
  idsByName,
  missingRequired,
  reportFilters,
  type CopyGroup,
  type ReportFilter,
} from './report';
import { ReportTable } from './ReportTable';
import { SaveList } from './SaveList';
import { useSavegameReport } from './useSavegameReport';

const t = strings.savegames;

export interface SavegamesScreenProps {
  profiles: readonly Profile[];
  /** The folder to check against until the user picks another: the active one. */
  defaultProfileId: string | null;
  game: GameStatus | null;
  fileOps: FileOpsController;
  /** Opens Settings, where the game folder is chosen. */
  onOpenSettings?: () => void;
}

const filterLabels: Record<ReportFilter, string> = {
  all: t.filterAll,
  missing: t.filterMissing,
  version: t.filterVersion,
  file: t.filterFile,
  present: t.filterPresent,
  checking: t.filterChecking,
};

const loadSaves = () => api.listSavegames();

export function SavegamesScreen({
  profiles,
  defaultProfileId,
  game,
  fileOps,
  onOpenSettings,
}: SavegamesScreenProps) {
  const { state, reload } = useAsync(loadSaves);
  const [chosenSaveId, setChosenSaveId] = useState<string | null>(null);
  const [chosenProfileId, setChosenProfileId] = useState<string | null>(null);

  const saves = state.status === 'ready' ? state.data : [];
  const save = saves.find((candidate) => candidate.id === chosenSaveId) ?? saves[0] ?? null;
  const profile =
    profiles.find((candidate) => candidate.id === chosenProfileId) ??
    profiles.find((candidate) => candidate.id === defaultProfileId) ??
    profiles[0] ??
    null;

  const report = useSavegameReport(save?.id ?? null, profile?.id ?? null, fileOps.revision);

  const folderOptions: SelectOption<string>[] = profiles.map((p) => ({
    value: p.id,
    label: p.name,
  }));

  return (
    <section className="sv-panel mm-saves" aria-label={t.title}>
      <div className="mm-library__head mm-saves__head">
        <div className="mm-library__title-row">
          <h2 className="mm-library__title">{t.title}</h2>
          <span className="mm-library__gap" />
          {profile && saves.length > 0 && (
            <Select
              className="mm-saves__folder"
              size="compact"
              label={t.folderLabel}
              prefix={t.checkAgainst}
              value={profile.id}
              options={folderOptions}
              onChange={setChosenProfileId}
            />
          )}
          <IconButton
            icon={RefreshCw}
            label={t.rescan}
            size="standard"
            onClick={() => {
              reload();
              report.recheck();
            }}
          />
        </div>
      </div>

      {state.status === 'loading' && (
        <div className="mm-loading" role="status" aria-label={t.loadingLabel} aria-busy="true" />
      )}
      {state.status === 'error' && (
        <EmptyState
          tone="error"
          icon={Save}
          title={t.loadFailedTitle}
          description={strings.states.loadFailedText(describeError(state.error))}
          action={<Button onClick={reload}>{strings.states.retry}</Button>}
        />
      )}
      {state.status === 'ready' && saves.length === 0 && game?.userDirSource === 'none' && (
        <EmptyState
          icon={FolderX}
          title={t.noGameFolderTitle}
          description={t.noGameFolderText}
          action={onOpenSettings && <Button onClick={onOpenSettings}>{t.openSettings}</Button>}
        />
      )}
      {state.status === 'ready' && saves.length === 0 && game?.userDirSource !== 'none' && (
        <EmptyState icon={Save} title={t.emptyTitle} description={t.emptyText} />
      )}
      {state.status === 'ready' && saves.length > 0 && profile === null && (
        <EmptyState icon={FolderX} title={t.noFolders} />
      )}
      {state.status === 'ready' && save !== null && profile !== null && (
        <>
          <SaveList saves={saves} selectedId={save.id} onSelect={setChosenSaveId} />
          <ReportSection
            saveName={save.name}
            profile={profile}
            profiles={profiles}
            controller={report}
            game={game}
            fileOps={fileOps}
          />
        </>
      )}
    </section>
  );
}

interface ReportSectionProps {
  saveName: string;
  profile: Profile;
  profiles: readonly Profile[];
  controller: ReturnType<typeof useSavegameReport>;
  game: GameStatus | null;
  fileOps: FileOpsController;
}

function ReportSection({
  saveName,
  profile,
  profiles,
  controller,
  game,
  fileOps,
}: ReportSectionProps) {
  const { report, status } = controller;
  const [filter, setFilter] = useState<ReportFilter>('all');
  const [preset, setPreset] = useState<TransferPreset | null>(null);
  const [preparing, setPreparing] = useState<string | null>(null);
  const prepareError = useActionError(profile.id);

  const rows: readonly SaveModRow[] = useMemo(() => report?.rows ?? [], [report]);
  const counts = useMemo(() => countReport(rows), [rows]);
  const required = useMemo(() => missingRequired(rows), [rows]);
  const groups = useMemo(() => copyGroups(rows), [rows]);
  const nowhere = useMemo(() => foundNowhere(rows), [rows]);
  const nameOf = (id: string) => profiles.find((p) => p.id === id)?.name ?? id;

  // A filter with no rows left (a file check ended, a copy finished) falls back to All.
  const activeFilter = filter !== 'all' && counts[filter] === 0 ? 'all' : filter;
  const shown = useMemo(() => filterReport(rows, activeFilter), [rows, activeFilter]);
  const requiredCount = rows.filter((row) => row.required).length;

  // The mod ids in the source folder are not in the report: ask the source folder for them.
  const copy = (group: CopyGroup) => {
    setPreparing(group.fromProfileId);
    void prepareError
      .run(async () => {
        const mods = await api.listMods(group.fromProfileId);
        const modIds = idsByName(mods, group.modNames);
        if (modIds.length > 0) {
          setPreset({
            fromProfileId: group.fromProfileId,
            toProfileId: profile.id,
            mode: 'copy',
            modIds,
          });
        }
      })
      .finally(() => setPreparing(null));
  };

  const blocked = fileOpBlockedReason(game, fileOps.running !== null);
  const visibleFilters = reportFilters.filter(
    (value) => value !== 'checking' || counts.checking > 0,
  );

  return (
    <>
      <div className="mm-library__toolbar mm-saves__reporthead">
        <div className="mm-saves__reporttitle">
          <h3 className="mm-saves__reportname sv-truncate">{saveName}</h3>
          {report && (
            <span className="mm-saves__summary sv-num sv-truncate">
              {t.summary(rows.length, requiredCount)}
            </span>
          )}
        </div>
        {report && (
          <SegmentedControl<ReportFilter>
            label={t.filterLabel}
            value={activeFilter}
            onChange={setFilter}
            segments={visibleFilters.map((value) => ({
              value,
              label: filterLabels[value],
              count: counts[value],
            }))}
          />
        )}
      </div>

      <HashNotice controller={controller} />

      <div className="mm-saves__scroll">
        {status === 'loading' && (
          <div className="mm-loading" role="status" aria-label={t.loadingLabel} aria-busy="true" />
        )}
        {status === 'error' && (
          <EmptyState
            tone="error"
            icon={Save}
            title={t.reportFailedTitle}
            description={strings.states.loadFailedText(describeError(controller.error))}
            action={<Button onClick={controller.recheck}>{strings.states.retry}</Button>}
          />
        )}
        {report && (
          <>
            <div className="mm-saves__notices">
              {required && (
                <Banner tone="error" title={t.requiredMissingTitle(required.title)}>
                  {required.foundIn.length > 0
                    ? t.requiredMissingFound(required.foundIn.map(nameOf).join(', '))
                    : t.requiredMissingNowhere}
                </Banner>
              )}
              {prepareError.message !== null && (
                <ActionErrorBanner
                  title={strings.actionFailed.prepareCopy}
                  message={prepareError.message}
                  onDismiss={prepareError.dismiss}
                />
              )}
              {(groups.length > 0 || nowhere.length > 0) && (
                <MissingActions
                  groups={groups}
                  nowhere={nowhere}
                  profiles={profiles}
                  targetName={profile.name}
                  blockedReason={blocked}
                  preparing={preparing}
                  onCopy={copy}
                />
              )}
            </div>
            {shown.length > 0 ? (
              <ReportTable
                rows={shown}
                label={t.tableLabel(saveName, profile.name)}
                profiles={profiles}
                checking={controller.hash.phase === 'running'}
              />
            ) : (
              <EmptyState
                icon={SearchX}
                title={t.rowsEmptyTitle}
                description={t.rowsEmptyText}
                action={<Button onClick={() => setFilter('all')}>{t.showAll}</Button>}
              />
            )}
            <ExtrasDisclosure ids={report.extras} />
          </>
        )}
      </div>

      {preset !== null && (
        <TransferDialog
          preset={preset}
          profiles={profiles}
          onClose={() => setPreset(null)}
          onConfirm={(plan, choices, targetName, replaced) => {
            setPreset(null);
            fileOps.startTransfer(plan, choices, targetName, replaced);
          }}
        />
      )}
    </>
  );
}
