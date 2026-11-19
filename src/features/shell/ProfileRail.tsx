import {
  CircleArrowUp,
  Folder,
  FolderOpen,
  FolderPlus,
  HeartPulse,
  Save,
  Settings,
  TriangleAlert,
  type LucideIcon,
} from 'lucide-react';
import type { Profile } from '../../api';
import { formatBytes, formatCount } from '../../lib/format';
import { useMediaQuery } from '../../lib/useMediaQuery';
import { strings } from '../../strings';
import { IconButton } from '../../ui/Button';
import { cx } from '../../ui/cx';
import { Icon } from '../../ui/Icon';
import { Tooltip } from '../../ui/Tooltip';

export type Section = 'library' | 'health' | 'savegames' | 'settings';

/** Below this window width the rail shows icons only. Keep in step with shell.css. */
export const RAIL_COLLAPSE_QUERY = '(max-width: 1040px)';

export interface ProfileRailProps {
  profiles: readonly Profile[];
  selectedProfileId: string | null;
  section: Section;
  onSelectProfile: (id: string) => void;
  onSelectSection: (section: Section) => void;
  /** Opens the folder picker and adds the chosen folder as a profile. */
  onAddFolder: () => void;
}

export function ProfileRail({
  profiles,
  selectedProfileId,
  section,
  onSelectProfile,
  onSelectSection,
  onAddFolder,
}: ProfileRailProps) {
  const t = strings.rail;
  const collapsed = useMediaQuery(RAIL_COLLAPSE_QUERY);
  const totals = profiles.reduce(
    (sum, profile) => ({
      mods: sum.mods + profile.modCount,
      bytes: sum.bytes + profile.totalBytes,
      issues: sum.issues + profile.issueCount,
    }),
    { mods: 0, bytes: 0, issues: 0 },
  );

  return (
    <nav
      className={cx('sv-panel', 'mm-rail', collapsed && 'mm-rail--collapsed')}
      aria-label={t.profiles}
    >
      <div className="sv-panel__header mm-rail__header">
        <span className="sv-panel__eyebrow">{t.profiles}</span>
        <IconButton icon={FolderPlus} label={t.addFolder} onClick={onAddFolder} />
      </div>

      <div className="mm-rail__scroll">
        <ul className="mm-rail__list">
          {profiles.map((profile) => (
            <li key={profile.id}>
              <ProfileItem
                profile={profile}
                selected={section === 'library' && profile.id === selectedProfileId}
                collapsed={collapsed}
                onSelect={() => onSelectProfile(profile.id)}
              />
            </li>
          ))}
        </ul>

        <div className="mm-rail__group">{t.tools}</div>
        <ul className="mm-rail__list">
          <li>
            <NavItem
              icon={HeartPulse}
              label={t.health}
              count={totals.issues}
              selected={section === 'health'}
              collapsed={collapsed}
              onSelect={() => onSelectSection('health')}
            />
          </li>
          <li>
            <NavItem
              icon={Save}
              label={t.savegames}
              selected={section === 'savegames'}
              collapsed={collapsed}
              onSelect={() => onSelectSection('savegames')}
            />
          </li>
        </ul>
      </div>

      {!collapsed && (
        <div className="mm-rail__totals">
          <div className="sv-panel__eyebrow">{t.totals}</div>
          <dl>
            <div className="mm-rail__total">
              <dt>{t.totalProfiles}</dt>
              <dd className="sv-num">{formatCount(profiles.length)}</dd>
            </div>
            <div className="mm-rail__total">
              <dt>{t.totalMods}</dt>
              <dd className="sv-num">{formatCount(totals.mods)}</dd>
            </div>
            <div className="mm-rail__total">
              <dt>{t.totalSize}</dt>
              <dd className="sv-num">{formatBytes(totals.bytes)}</dd>
            </div>
            <div className="mm-rail__total">
              <dt>{t.totalIssues}</dt>
              <dd className="sv-num">{formatCount(totals.issues)}</dd>
            </div>
          </dl>
        </div>
      )}

      <div className="mm-rail__footer">
        <NavItem
          icon={Settings}
          label={t.settings}
          selected={section === 'settings'}
          collapsed={collapsed}
          onSelect={() => onSelectSection('settings')}
        />
      </div>
    </nav>
  );
}

interface ProfileItemProps {
  profile: Profile;
  selected: boolean;
  collapsed: boolean;
  onSelect: () => void;
}

function ProfileItem({ profile, selected, collapsed, onSelect }: ProfileItemProps) {
  const t = strings.rail;
  const summary = t.summary(profile.modCount, formatBytes(profile.totalBytes));

  const item = (
    <button
      type="button"
      className="mm-profile"
      aria-current={selected ? 'page' : undefined}
      aria-label={collapsed ? `${profile.name}, ${summary}` : undefined}
      onClick={onSelect}
    >
      <span className="mm-profile__icon">
        <Icon icon={selected ? FolderOpen : Folder} />
        {profile.isActive && <span className="sv-status-dot sv-status-dot--on mm-profile__dot" />}
      </span>
      {!collapsed && (
        <span className="mm-profile__body">
          <span className="mm-profile__line">
            <span className="mm-profile__name sv-truncate">{profile.name}</span>
            {profile.isActive && <span className="mm-active-mark">{t.active}</span>}
          </span>
          <span className="mm-profile__line">
            <span className="mm-profile__meta sv-truncate sv-num">{summary}</span>
            <span className="mm-profile__marks sv-num">
              {profile.issueCount > 0 && (
                <span className="mm-profile__mark" title={t.issues(profile.issueCount)}>
                  <Icon icon={TriangleAlert} size={12} className="sv-tone-warning" />
                  {formatCount(profile.issueCount)}
                </span>
              )}
              {profile.updateCount > 0 && (
                <span className="mm-profile__mark" title={t.updates(profile.updateCount)}>
                  <Icon icon={CircleArrowUp} size={12} className="sv-tone-info" />
                  {formatCount(profile.updateCount)}
                </span>
              )}
            </span>
          </span>
        </span>
      )}
    </button>
  );

  return collapsed ? <Tooltip label={`${profile.name} · ${summary}`}>{item}</Tooltip> : item;
}

interface NavItemProps {
  icon: LucideIcon;
  label: string;
  count?: number;
  selected: boolean;
  collapsed: boolean;
  onSelect: () => void;
}

function NavItem({ icon, label, count, selected, collapsed, onSelect }: NavItemProps) {
  const item = (
    <button
      type="button"
      className="sv-nav-item"
      aria-current={selected ? 'page' : undefined}
      aria-label={collapsed ? label : undefined}
      onClick={onSelect}
    >
      <Icon icon={icon} />
      {!collapsed && <span className="sv-nav-item__label sv-truncate">{label}</span>}
      {!collapsed && count !== undefined && count > 0 && (
        <span className="sv-count-chip sv-num">{formatCount(count)}</span>
      )}
    </button>
  );
  return collapsed ? <Tooltip label={label}>{item}</Tooltip> : item;
}
