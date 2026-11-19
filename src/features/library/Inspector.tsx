import {
  Boxes,
  CircleCheck,
  FolderOpen,
  MousePointerClick,
  PanelRightClose,
  TriangleAlert,
} from 'lucide-react';
import type { ReactNode } from 'react';
import type { GameStatus, ModEntry, ModIssue, Profile } from '../../api';
import { api } from '../../api/client';
import { formatBytes, formatDate } from '../../lib/format';
import { useActionError } from '../../lib/useActionError';
import { strings } from '../../strings';
import { ActionErrorBanner } from '../ActionErrorBanner';
import { IconButton } from '../../ui/Button';
import { cx } from '../../ui/cx';
import { EmptyState } from '../../ui/Feedback';
import { Icon } from '../../ui/Icon';
import { Pill } from '../../ui/Pill';
import { FileActions } from '../files/FileActions';
import type { FileOpsController } from '../files/useFileOps';
import { SourceSection } from '../updates/SourceSection';
import { UpdateBanner } from '../updates/UpdateBanner';
import { installableUpdates } from '../updates/choices';
import { UpdateSelection } from '../updates/UpdateSelection';
import { kindIcons, severityIcons } from './labels';
import { ModIcon } from './ModIcon';
import { NoteField } from './NoteField';
import { TagEditor } from './TagEditor';
import { addTags, removeTag, tagCounts, tagProblem } from './tags';
import type { ModMetaController } from './useModMeta';

export interface InspectorProps {
  /** The selected mods, in list order. */
  mods: readonly ModEntry[];
  profiles: readonly Profile[];
  game: GameStatus | null;
  fileOps: FileOpsController;
  /** Tags and notes: editing them, and the tags that exist. */
  meta: ModMetaController;
  /** Shown as a drawer over the list instead of beside it. */
  overlay: boolean;
  onClose: () => void;
  /** A source link was saved or removed: read the list again. */
  onLinksChanged: () => void;
}

const t = strings.inspector;

/** The issue's data worded for display; null when there is nothing to add to the help text. */
const detailText = (issue: ModIssue) => strings.issueDetail[issue.kind](issue.detail);

export function Inspector({
  mods,
  profiles,
  game,
  fileOps,
  meta,
  overlay,
  onClose,
  onLinksChanged,
}: InspectorProps) {
  const only = mods.length === 1 ? mods[0] : undefined;
  return (
    <aside
      className={cx('sv-panel', 'mm-inspector', overlay && 'mm-inspector--overlay')}
      aria-label={t.title}
    >
      <div className="sv-panel__header">
        <h2 className="sv-panel__title">{t.title}</h2>
        <IconButton icon={PanelRightClose} label={t.close} onClick={onClose} />
      </div>

      {only ? (
        <ModDetails
          mod={only}
          profiles={profiles}
          game={game}
          fileOps={fileOps}
          meta={meta}
          onLinksChanged={onLinksChanged}
        />
      ) : mods.length > 1 ? (
        <>
          <div className="mm-inspector__scroll">
            <EmptyState
              icon={Boxes}
              title={t.multiTitle(mods.length)}
              description={t.multiText(
                formatBytes(mods.reduce((sum, mod) => sum + mod.sizeBytes, 0)),
              )}
            />
            <div className="mm-inspector__sections">
              {installableUpdates(mods).length > 0 && (
                <Section title={t.multiUpdatesTitle}>
                  <UpdateSelection mods={mods} profiles={profiles} game={game} fileOps={fileOps} />
                </Section>
              )}
              <Section title={t.multiTagsTitle}>
                <TagsSection mods={mods} meta={meta} />
              </Section>
            </div>
          </div>
          <div className="sv-panel__footer">
            <FileActions mods={mods} profiles={profiles} game={game} fileOps={fileOps} />
          </div>
        </>
      ) : (
        <EmptyState icon={MousePointerClick} title={t.emptyTitle} description={t.emptyText} />
      )}
    </aside>
  );
}

function ModDetails({
  mod,
  profiles,
  game,
  fileOps,
  meta,
  onLinksChanged,
}: {
  mod: ModEntry;
  profiles: readonly Profile[];
  game: GameStatus | null;
  fileOps: FileOpsController;
  meta: ModMetaController;
  onLinksChanged: () => void;
}) {
  const showInFolder = useActionError(`${mod.profileId}/${mod.id}`);
  const otherProfiles = profiles.filter((profile) => mod.alsoIn.includes(profile.id));

  return (
    <>
      <div className="mm-inspector__scroll">
        <div className="mm-inspector__identity">
          <ModIcon mod={mod} size="detail" />
          <div className="mm-inspector__heading">
            <h3 className="mm-inspector__title sv-selectable">{mod.title}</h3>
            <p className="mm-inspector__author">
              {mod.author ? t.by(mod.author) : t.unknownAuthor}
            </p>
            <div className="mm-inspector__tags">
              <Pill icon={kindIcons[mod.kind]}>{strings.kinds[mod.kind]}</Pill>
              {mod.version && <span className="mm-inspector__version sv-num">{mod.version}</span>}
            </div>
          </div>
        </div>

        <div className="mm-inspector__sections">
          <UpdateBanner mod={mod} profiles={profiles} game={game} fileOps={fileOps} />

          {mod.issues.length > 0 && (
            <Section title={t.issues}>
              <ul className="mm-issues">
                {mod.issues.map((issue, index) => (
                  <li key={index} className="mm-issue">
                    <Icon
                      icon={severityIcons[issue.severity]}
                      className={`sv-tone-${issue.severity}`}
                    />
                    <div className="mm-issue__body">
                      <div className="mm-issue__title">
                        <span className="sv-visually-hidden">
                          {strings.severity[issue.severity]}:{' '}
                        </span>
                        {strings.issues[issue.kind]}
                      </div>
                      <div className="mm-issue__text">{strings.issueHelp[issue.kind]}</div>
                      {detailText(issue) && (
                        <div className="mm-issue__detail sv-selectable">{detailText(issue)}</div>
                      )}
                    </div>
                  </li>
                ))}
              </ul>
            </Section>
          )}

          <Section title={t.sourceTitle}>
            <SourceSection mod={mod} onChanged={onLinksChanged} />
          </Section>

          <Section title={strings.tags.title}>
            <TagsSection mods={[mod]} meta={meta} />
          </Section>

          <Section title={strings.tags.noteTitle}>
            <NoteField key={`${mod.profileId}/${mod.id}`} mod={mod} meta={meta} />
          </Section>

          <Section title={t.description}>
            <p className={cx('mm-inspector__description', mod.description && 'sv-selectable')}>
              {mod.description || t.noDescription}
            </p>
          </Section>

          <Section title={t.details}>
            <dl className="mm-props">
              <Property label={t.fileName}>
                <span className="sv-inline-code sv-selectable">{mod.fileName}</span>
              </Property>
              <Property label={t.format}>
                {mod.format === 'zip' ? t.formatZip : t.formatFolder}
              </Property>
              <Property label={t.size}>
                <span className="sv-num">{formatBytes(mod.sizeBytes)}</span>
              </Property>
              <Property label={t.modified}>
                <span className="sv-num">{formatDate(mod.modified)}</span>
              </Property>
              <Property label={t.descVersion}>
                <span className="sv-num">{mod.descVersion ?? t.unknown}</span>
                {game?.loggedDescVersion != null && (
                  <span className="mm-props__quiet sv-num">
                    {t.descVersionGame(game.loggedDescVersion)}
                  </span>
                )}
              </Property>
              <Property label={t.multiplayer}>{mod.multiplayer ? t.yes : t.no}</Property>
              <Property label={t.alsoIn}>
                {otherProfiles.length > 0 ? (
                  otherProfiles.map((profile) => (
                    <Pill key={profile.id} size="compact">
                      {profile.name}
                    </Pill>
                  ))
                ) : (
                  <span className="mm-props__quiet">{t.onlyHere}</span>
                )}
              </Property>
            </dl>
          </Section>

          {mod.dependencies.length > 0 && (
            <Section title={t.dependencies}>
              <ul className="mm-deps">
                {mod.dependencies.map((dependency) => (
                  <li key={dependency.modName} className="mm-dep">
                    <Icon
                      icon={dependency.satisfied ? CircleCheck : TriangleAlert}
                      size={16}
                      className={dependency.satisfied ? 'sv-tone-success' : 'sv-tone-warning'}
                    />
                    <span className="mm-dep__name sv-truncate sv-selectable">
                      {dependency.modName}
                    </span>
                    <span className="mm-dep__state">
                      {dependency.satisfied ? t.dependencyOk : t.dependencyMissing}
                    </span>
                  </li>
                ))}
              </ul>
            </Section>
          )}
        </div>
      </div>

      {showInFolder.message !== null && (
        <ActionErrorBanner
          className="mm-inspector__notice"
          title={strings.actionFailed.showInFolder}
          message={showInFolder.message}
          onDismiss={showInFolder.dismiss}
        />
      )}
      <div className="sv-panel__footer">
        <IconButton
          icon={FolderOpen}
          label={t.showInFolder}
          onClick={() => void showInFolder.run(() => api.showModInFolder(mod.profileId, mod.id))}
        />
        <FileActions mods={[mod]} profiles={profiles} game={game} fileOps={fileOps} />
      </div>
    </>
  );
}

/** The tag editor for one mod or for a selection, with the failure of the last edit. */
function TagsSection({ mods, meta }: { mods: readonly ModEntry[]; meta: ModMetaController }) {
  return (
    <>
      <TagEditor
        chips={tagCounts(mods)}
        total={mods.length}
        known={meta.knownTags}
        problemFor={(tag) => tagProblem(mods, tag)}
        onAdd={(tags) => void meta.setTags(mods, (current) => addTags(current, tags))}
        onRemove={(tag) => void meta.setTags(mods, (current) => removeTag(current, tag))}
      />
      {meta.failure !== null && (
        <ActionErrorBanner
          className="mm-inspector__meta-failure"
          title={meta.failure.title}
          message={meta.failure.message}
          onDismiss={meta.dismissFailure}
        />
      )}
    </>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="mm-inspector__section">
      <h4 className="mm-inspector__section-title">{title}</h4>
      {children}
    </section>
  );
}

function Property({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="mm-props__row">
      <dt>{label}</dt>
      <dd>{children}</dd>
    </div>
  );
}
