import {
  FolderOpen,
  LayoutGrid,
  List,
  PackageOpen,
  PanelRightClose,
  PanelRightOpen,
  RefreshCw,
  SearchX,
} from 'lucide-react';
import { useCallback, useDeferredValue, useEffect, useMemo, useState } from 'react';
import type { GameStatus, ModEntry, Profile } from '../../api';
import { api } from '../../api/client';
import { formatCount, tailPath } from '../../lib/format';
import { useAsync } from '../../lib/useAsync';
import { useMediaQuery } from '../../lib/useMediaQuery';
import { strings } from '../../strings';
import { Button, IconButton } from '../../ui/Button';
import { cx } from '../../ui/cx';
import { EmptyState } from '../../ui/Feedback';
import { Pill } from '../../ui/Pill';
import { SearchField } from '../../ui/SearchField';
import { SegmentedControl } from '../../ui/SegmentedControl';
import { Select, type SelectOption } from '../../ui/Select';
import { Tooltip } from '../../ui/Tooltip';
import type { CollectionProps } from './collection';
import { Inspector } from './Inspector';
import { kindOrder } from './labels';
import { ModGrid } from './ModGrid';
import { ModTable } from './ModTable';
import {
  countByKind,
  defaultQuery,
  queryMods,
  type KindFilter,
  type LibraryQuery,
  type SortKey,
  type StatusFilter,
} from './query';
import {
  emptySelection,
  extendTo,
  navigate,
  restrictTo,
  selectAll,
  selectOnly,
  toggle,
  type Selection,
} from './selection';

/** Below this window width the inspector opens as a drawer over the list. Keep in step with library.css. */
const NARROW_QUERY = '(max-width: 1400px)';
const NO_MODS: readonly ModEntry[] = [];

type View = 'list' | 'grid';

export interface LibraryScreenProps {
  profile: Profile;
  profiles: readonly Profile[];
  game: GameStatus | null;
}

const t = strings.library;

export function LibraryScreen({ profile, profiles, game }: LibraryScreenProps) {
  const [query, setQuery] = useState<LibraryQuery>(defaultQuery);
  const [view, setView] = useState<View>('list');
  // The docked preference and the temporary drawer are separate, so a narrow window
  // never overwrites what the user chose for a wide one.
  const [docked, setDocked] = useState(true);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const narrow = useMediaQuery(NARROW_QUERY);
  const inspectorOpen = narrow ? drawerOpen : docked;

  const load = useCallback(() => api.listMods(profile.id), [profile.id]);
  const { state, reload } = useAsync(load);
  const allMods = state.status === 'ready' ? state.data : NO_MODS;

  const deferredQuery = useDeferredValue(query);
  const mods = useMemo(() => queryMods(allMods, deferredQuery), [allMods, deferredQuery]);
  const order = useMemo(() => mods.map((mod) => mod.id), [mods]);
  const kindCounts = useMemo(() => countByKind(allMods), [allMods]);
  const statusCounts = useMemo(
    () => ({
      issues: allMods.filter((mod) => mod.issues.length > 0).length,
      updates: allMods.filter((mod) => mod.update.type === 'available').length,
    }),
    [allMods],
  );

  // Selection belongs to one profile: switching profiles starts fresh without an effect.
  const [stored, setStored] = useState({ profileId: profile.id, selection: emptySelection });
  const selection = useMemo(
    () => restrictTo(stored.profileId === profile.id ? stored.selection : emptySelection, order),
    [stored, profile.id, order],
  );
  const select = useCallback(
    (next: Selection) => setStored({ profileId: profile.id, selection: next }),
    [profile.id],
  );
  const selectedMods = useMemo(
    () => mods.filter((mod) => selection.ids.has(mod.id)),
    [mods, selection],
  );

  const openInspector = useCallback(() => {
    if (narrow) setDrawerOpen(true);
    else setDocked(true);
  }, [narrow]);
  const closeInspector = useCallback(() => {
    if (narrow) setDrawerOpen(false);
    else setDocked(false);
  }, [narrow]);

  useEffect(() => {
    if (!(narrow && drawerOpen)) return;
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setDrawerOpen(false);
    };
    document.addEventListener('keydown', closeOnEscape);
    return () => document.removeEventListener('keydown', closeOnEscape);
  }, [narrow, drawerOpen]);

  const collection: CollectionProps = {
    mods,
    selection,
    label: t.tableLabel(profile.name),
    onPress: (id, modifiers) => {
      if (modifiers.extend) select(extendTo(selection, order, id));
      else if (modifiers.toggle) select(toggle(selection, id));
      else select(selectOnly(id));
    },
    onOpen: (id) => {
      select(selectOnly(id));
      openInspector();
    },
    onNavigate: (key, extend, layout) => {
      const next = navigate(selection, order, key, extend, layout);
      if (next) select(next);
      return next !== null;
    },
    onSelectAll: () => select(selectAll(selection, order)),
    onClear: () => select(emptySelection),
  };

  const onSort = (key: SortKey) =>
    setQuery((current) => ({
      ...current,
      sortKey: key,
      sortDirection: current.sortKey === key && current.sortDirection === 'asc' ? 'desc' : 'asc',
    }));

  const kindOptions: SelectOption<KindFilter>[] = [
    { value: 'all', label: t.kindAll, hint: formatCount(allMods.length) },
    ...kindOrder.map((kind) => ({
      value: kind,
      label: strings.kindsPlural[kind],
      hint: formatCount(kindCounts[kind]),
    })),
  ];
  const filtered = query.search.trim() !== '' || query.kind !== 'all' || query.status !== 'all';
  const ready = state.status === 'ready';

  return (
    <div className={cx('mm-library-layout', inspectorOpen && !narrow && 'has-inspector')}>
      <section className="sv-panel mm-library" aria-label={profile.name}>
        <div className="mm-library__head">
          <div className="mm-library__title-row">
            <h2 className="mm-library__title sv-truncate">{profile.name}</h2>
            {profile.isActive && <span className="mm-active-mark">{t.active}</span>}
            {profile.isGameDefault && <Pill>{t.gameDefault}</Pill>}
            <Tooltip label={profile.path}>
              <span className="mm-library__path sv-truncate">{tailPath(profile.path)}</span>
            </Tooltip>
            <span className="mm-library__gap" />
            {!profile.isActive && <Button variant="primary">{t.setActive}</Button>}
            <IconButton icon={FolderOpen} label={t.openFolder} size="standard" />
            <IconButton icon={RefreshCw} label={t.rescan} size="standard" onClick={reload} />
          </div>

          <div className="mm-library__toolbar">
            <div className="mm-library__filters">
              <SearchField
                className="mm-library__search"
                size="compact"
                label={t.searchLabel}
                placeholder={t.searchPlaceholder}
                clearLabel={t.clearSearch}
                value={query.search}
                onChange={(search) => setQuery((current) => ({ ...current, search }))}
              />
              <Select
                className="mm-library__kind"
                size="compact"
                label={t.kindLabel}
                prefix={t.kindPrefix}
                value={query.kind}
                options={kindOptions}
                onChange={(kind) => setQuery((current) => ({ ...current, kind }))}
              />
              <SegmentedControl<StatusFilter>
                label={t.statusLabel}
                value={query.status}
                onChange={(status) => setQuery((current) => ({ ...current, status }))}
                segments={[
                  { value: 'all', label: t.statusAll },
                  {
                    value: 'issues',
                    label: t.statusIssues,
                    count: ready ? statusCounts.issues : undefined,
                  },
                  {
                    value: 'updates',
                    label: t.statusUpdates,
                    count: ready ? statusCounts.updates : undefined,
                  },
                ]}
              />
            </div>
            <div className="mm-library__views">
              {ready && (
                <span className="mm-library__count sv-num" role="status">
                  {selection.ids.size > 1 ? (
                    <strong>{t.selected(selection.ids.size)}</strong>
                  ) : (
                    t.count(mods.length, allMods.length)
                  )}
                </span>
              )}
              <SegmentedControl<View>
                label={t.viewLabel}
                value={view}
                onChange={setView}
                segments={[
                  { value: 'list', label: t.viewList, icon: List },
                  { value: 'grid', label: t.viewGrid, icon: LayoutGrid },
                ]}
              />
              <IconButton
                icon={inspectorOpen ? PanelRightClose : PanelRightOpen}
                label={inspectorOpen ? t.hideDetails : t.showDetails}
                pressed={inspectorOpen}
                onClick={inspectorOpen ? closeInspector : openInspector}
              />
            </div>
          </div>
        </div>

        {state.status === 'loading' && <LoadingRows />}
        {state.status === 'error' && (
          <EmptyState
            tone="error"
            icon={FolderOpen}
            title={strings.states.loadFailedTitle}
            description={strings.states.loadFailedText(state.message.replace(/\.$/, ''))}
            action={<Button onClick={reload}>{strings.states.retry}</Button>}
          />
        )}
        {ready && allMods.length === 0 && (
          <EmptyState
            icon={PackageOpen}
            title={strings.states.emptyFolderTitle}
            description={strings.states.emptyFolderText}
          />
        )}
        {ready && allMods.length > 0 && mods.length === 0 && (
          <EmptyState
            icon={SearchX}
            title={strings.states.noMatchesTitle}
            description={strings.states.noMatchesText}
            action={
              filtered && (
                <Button
                  onClick={() =>
                    setQuery((current) => ({ ...current, search: '', kind: 'all', status: 'all' }))
                  }
                >
                  {strings.states.clearFilters}
                </Button>
              )
            }
          />
        )}
        {ready &&
          mods.length > 0 &&
          (view === 'list' ? (
            <ModTable
              {...collection}
              sortKey={query.sortKey}
              sortDirection={query.sortDirection}
              onSort={onSort}
            />
          ) : (
            <ModGrid {...collection} />
          ))}
      </section>

      {inspectorOpen && narrow && (
        <div className="mm-drawer-backdrop" onClick={() => setDrawerOpen(false)} />
      )}
      {inspectorOpen && (
        <Inspector
          mods={selectedMods}
          profiles={profiles}
          game={game}
          overlay={narrow}
          onClose={closeInspector}
        />
      )}
    </div>
  );
}

const SKELETON_WIDTHS = [62, 48, 71, 39, 56, 66, 44, 59, 52, 68, 41, 63, 50, 57];

function LoadingRows() {
  return (
    <div className="mm-loading" role="status" aria-label={strings.states.loadingMods}>
      {SKELETON_WIDTHS.map((width, index) => (
        <div key={index} className="mm-loading__row">
          <span className="sv-skeleton mm-loading__icon" />
          <span className="sv-skeleton" style={{ width: `${width}%` }} />
        </div>
      ))}
    </div>
  );
}
