import {
  CircleArrowUp,
  Folder,
  FolderOpen,
  FolderPlus,
  HeartPulse,
  ListX,
  Pencil,
  Save,
  Settings,
  TriangleAlert,
  type LucideIcon,
} from 'lucide-react';
import { useRef, useState } from 'react';
import type { Profile } from '../../api';
import { formatBytes, formatCount } from '../../lib/format';
import { useMediaQuery } from '../../lib/useMediaQuery';
import { strings } from '../../strings';
import { IconButton } from '../../ui/Button';
import { cx } from '../../ui/cx';
import { Icon } from '../../ui/Icon';
import { Menu, MenuPopup, type MenuAnchor, type MenuItem } from '../../ui/Menu';
import { Tooltip } from '../../ui/Tooltip';
import { RemoveProfileDialog, RenameProfileDialog } from '../files/ProfileDialogs';

export type Section = 'library' | 'health' | 'savegames' | 'settings';

/** Below this window width the rail shows icons only. Keep in step with shell.css. */
export const RAIL_COLLAPSE_QUERY = '(max-width: 1040px)';

/** What the profile menu does; the screen that owns the profile list supplies it. */
export interface ProfileActions {
  /** Changes the display name and reloads the list. Rejects with the Api's error. */
  rename: (profile: Profile, name: string) => Promise<void>;
  /** Removes the folder from the list and reloads. Rejects with the Api's error. */
  remove: (profile: Profile) => Promise<void>;
  /** Why "Remove from list" is unavailable for this profile right now, or null. */
  removeBlockedReason: (profile: Profile) => string | null;
}

export interface ProfileRailProps {
  profiles: readonly Profile[];
  selectedProfileId: string | null;
  section: Section;
  onSelectProfile: (id: string) => void;
  onSelectSection: (section: Section) => void;
  /** Opens the folder picker and adds the chosen folder as a profile. */
  onAddFolder: () => void;
  profileActions: ProfileActions;
}

export function ProfileRail({
  profiles,
  selectedProfileId,
  section,
  onSelectProfile,
  onSelectSection,
  onAddFolder,
  profileActions,
}: ProfileRailProps) {
  const t = strings.rail;
  const collapsed = useMediaQuery(RAIL_COLLAPSE_QUERY);
  const [dialog, setDialog] = useState<{ kind: 'rename' | 'remove'; profile: Profile } | null>(
    null,
  );
  const menuItems = (profile: Profile): MenuItem[] => {
    const blocked = profileActions.removeBlockedReason(profile);
    return [
      {
        id: 'rename',
        label: strings.profileMenu.rename,
        icon: Pencil,
        onSelect: () => setDialog({ kind: 'rename', profile }),
      },
      {
        id: 'remove',
        label: strings.profileMenu.remove,
        icon: ListX,
        disabledReason: blocked ?? undefined,
        onSelect: () => setDialog({ kind: 'remove', profile }),
      },
    ];
  };
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
                menuItems={menuItems(profile)}
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

      {dialog?.kind === 'rename' && (
        <RenameProfileDialog
          profile={dialog.profile}
          onClose={() => setDialog(null)}
          onRename={profileActions.rename}
        />
      )}
      {dialog?.kind === 'remove' && (
        <RemoveProfileDialog
          profile={dialog.profile}
          onClose={() => setDialog(null)}
          onRemove={profileActions.remove}
        />
      )}
    </nav>
  );
}

interface ProfileItemProps {
  profile: Profile;
  selected: boolean;
  collapsed: boolean;
  menuItems: MenuItem[];
  onSelect: () => void;
}

/**
 * One folder in the rail. The profile menu is a small button at the right, visible on hover and
 * keyboard focus. The collapsed rail has no room for it, so there (and as a shortcut everywhere)
 * the same menu opens from the context menu key, Shift+F10 or a right click.
 */
function ProfileItem({ profile, selected, collapsed, menuItems, onSelect }: ProfileItemProps) {
  const t = strings.rail;
  const summary = t.summary(profile.modCount, formatBytes(profile.totalBytes));
  const [menuOpen, setMenuOpen] = useState(false);
  const [contextAnchor, setContextAnchor] = useState<MenuAnchor | null>(null);
  const itemRef = useRef<HTMLButtonElement>(null);

  const item = (
    <button
      type="button"
      className="mm-profile"
      aria-current={selected ? 'page' : undefined}
      aria-label={collapsed ? `${profile.name}, ${summary}` : undefined}
      ref={itemRef}
      onClick={onSelect}
      onContextMenu={(event) => {
        event.preventDefault();
        setContextAnchor({ x: event.clientX, y: event.clientY });
      }}
      onKeyDown={(event) => {
        if (event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10')) {
          event.preventDefault();
          setContextAnchor(event.currentTarget);
        }
      }}
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

  const label = strings.profileMenu.actions(profile.name);
  return (
    <div className="mm-profile-row" data-menu-open={menuOpen || undefined}>
      {collapsed ? <Tooltip label={`${profile.name} · ${summary}`}>{item}</Tooltip> : item}
      {!collapsed && (
        <div className="mm-profile-row__menu">
          <Menu label={label} items={menuItems} onOpenChange={setMenuOpen} />
        </div>
      )}
      {contextAnchor && (
        <MenuPopup
          label={label}
          items={menuItems}
          anchor={contextAnchor}
          onClose={(restoreFocus) => {
            setContextAnchor(null);
            if (restoreFocus) itemRef.current?.focus();
          }}
        />
      )}
    </div>
  );
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
