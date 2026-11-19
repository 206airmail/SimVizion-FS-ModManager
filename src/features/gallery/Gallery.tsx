// Development-only page (open with #gallery): every primitive in its states, for design review.
// Its labels are review captions, not product text, so they are not in strings.ts.

import {
  Copy,
  FolderOpen,
  LayoutGrid,
  List,
  ListX,
  PackageOpen,
  Pencil,
  Play,
  RefreshCw,
  Trash2,
} from 'lucide-react';
import { useState, type ReactNode } from 'react';
import { strings } from '../../strings';
import { Button, IconButton } from '../../ui/Button';
import { Checkbox } from '../../ui/Checkbox';
import { Dialog } from '../../ui/Dialog';
import { Banner, EmptyState } from '../../ui/Feedback';
import { Menu, type MenuItem } from '../../ui/Menu';
import { Pill } from '../../ui/Pill';
import { SearchField } from '../../ui/SearchField';
import { SegmentedControl } from '../../ui/SegmentedControl';
import { Select } from '../../ui/Select';
import { Switch } from '../../ui/Switch';
import './gallery.css';

type DialogSample = 'confirm' | 'destructive' | 'busy' | 'scrolling' | null;

const menuItems: MenuItem[] = [
  { id: 'copy', label: 'Copy to…', icon: Copy, onSelect: () => undefined },
  { id: 'rename', label: 'Rename…', icon: Pencil, onSelect: () => undefined },
  {
    id: 'remove',
    label: 'Remove from list…',
    icon: ListX,
    disabledReason: 'The active folder stays in the list.',
    onSelect: () => undefined,
  },
  {
    id: 'delete',
    label: 'Move to the Recycle Bin',
    icon: Trash2,
    destructive: true,
    separatorBefore: true,
    onSelect: () => undefined,
  },
];

export function Gallery() {
  const [sampleOn, setSampleOn] = useState(true);
  const [search, setSearch] = useState('tractor');
  const [kind, setKind] = useState<'all' | 'vehicle' | 'map'>('all');
  const [status, setStatus] = useState<'all' | 'issues' | 'updates'>('issues');
  const [view, setView] = useState<'list' | 'grid'>('list');
  const [dialog, setDialog] = useState<DialogSample>(null);
  const closeDialog = () => setDialog(null);

  return (
    <div className="mm-gallery">
      <header className="mm-gallery__head">
        <h1>{strings.gallery.title}</h1>
        <a className="sv-link" href="./">
          {strings.gallery.back}
        </a>
      </header>

      <Group title="Buttons, standard (32px)">
        <Button variant="primary" icon={Play} illuminated>
          Primary
        </Button>
        <Button variant="primary">Primary plain</Button>
        <Button icon={FolderOpen}>Secondary</Button>
        <Button variant="destructive" icon={Trash2}>
          Destructive
        </Button>
        <Button variant="primary" disabled>
          Disabled
        </Button>
        <Button disabled>Disabled</Button>
        <IconButton icon={RefreshCw} label="Icon button" size="standard" />
      </Group>

      <Group title="Buttons, compact (28px)">
        <Button size="compact" variant="primary">
          Primary
        </Button>
        <Button size="compact" icon={FolderOpen}>
          Secondary
        </Button>
        <Button size="compact" variant="destructive" icon={Trash2}>
          Destructive
        </Button>
        <IconButton icon={RefreshCw} label="Icon button" />
        <IconButton icon={LayoutGrid} label="Pressed toggle" pressed />
        <IconButton icon={RefreshCw} label="Disabled icon button" disabled />
      </Group>

      <Group title="Fields">
        <div style={{ width: 260 }}>
          <SearchField label="Search" clearLabel="Clear" value={search} onChange={setSearch} />
        </div>
        <div style={{ width: 220 }}>
          <SearchField
            size="compact"
            label="Search"
            clearLabel="Clear"
            placeholder="Compact, empty"
            value=""
            onChange={() => undefined}
          />
        </div>
        <div style={{ width: 180 }}>
          <Select
            label="Type"
            prefix="Type"
            value={kind}
            onChange={setKind}
            options={[
              { value: 'all', label: 'All types', hint: '590' },
              { value: 'vehicle', label: 'Vehicles', hint: '325' },
              { value: 'map', label: 'Maps', hint: '6' },
            ]}
          />
        </div>
      </Group>

      <Group title="Segmented controls">
        <SegmentedControl
          label="Status"
          value={status}
          onChange={setStatus}
          segments={[
            { value: 'all', label: 'All' },
            { value: 'issues', label: 'Issues', count: 34 },
            { value: 'updates', label: 'Updates', count: 38 },
          ]}
        />
        <SegmentedControl
          label="View"
          value={view}
          onChange={setView}
          segments={[
            { value: 'list', label: 'List view', icon: List },
            { value: 'grid', label: 'Grid view', icon: LayoutGrid },
          ]}
        />
      </Group>

      <Group title="Pills">
        <Pill>Vehicle</Pill>
        <Pill size="compact">Compact</Pill>
        <Pill tone="info">Update</Pill>
        <Pill tone="warning">Duplicate</Pill>
        <Pill tone="error">Not a mod</Pill>
        <Pill tone="success">Verified</Pill>
        <span className="mm-active-mark">Active</span>
      </Group>

      <Group title="Checkbox and switch (lime when on; Space toggles; teal border on focus)">
        <Checkbox checked={sampleOn} onChange={setSampleOn}>
          Main Farm
        </Checkbox>
        <Checkbox checked={false} onChange={() => undefined}>
          Unticked
        </Checkbox>
        <Checkbox checked disabled onChange={() => undefined}>
          Disabled
        </Checkbox>
        <Switch checked={sampleOn} onChange={setSampleOn}>
          Check for mod updates online
        </Switch>
        <Switch checked={false} disabled onChange={() => undefined}>
          Disabled
        </Switch>
      </Group>

      <Group title="Banners" column>
        <Banner
          tone="info"
          title="Version 1.2.0.0 is available"
          actions={<Button size="compact">Update</Button>}
        >
          Installed: 1.1.0.0. Source: ModHub 284193.
        </Banner>
        <Banner tone="warning" title="The game is running">
          Switching profiles is paused until Farming Simulator closes.
        </Banner>
        <Banner tone="error" title="gameSettings.xml could not be written">
          Nothing was changed. The previous file is still in place.
        </Banner>
        <Banner tone="success" title="Profile switched" />
      </Group>

      <Group title="Menu (arrows, Home, End, Enter, Escape; a disabled item keeps its reason)">
        <Menu label="Actions" items={menuItems} />
        <Menu label="Actions" items={menuItems} text="Actions" size="standard" />
      </Group>

      <Group title="Dialog (modal: focus trapped, Escape closes unless busy, focus returns here)">
        <Button onClick={() => setDialog('confirm')}>Confirmation</Button>
        <Button variant="destructive" icon={Trash2} onClick={() => setDialog('destructive')}>
          Destructive confirmation
        </Button>
        <Button onClick={() => setDialog('busy')}>Busy (Escape ignored)</Button>
        <Button onClick={() => setDialog('scrolling')}>Long body (scrolls)</Button>
      </Group>
      {dialog === 'confirm' && (
        <Dialog
          title="Rename mod folder"
          size="small"
          onClose={closeDialog}
          footer={
            <>
              <Button onClick={closeDialog}>Cancel</Button>
              <Button variant="primary" onClick={closeDialog}>
                Rename
              </Button>
            </>
          }
        >
          <p>Only the name in the list changes. The folder on disk keeps its name.</p>
        </Dialog>
      )}
      {dialog === 'destructive' && (
        <Dialog
          title="Remove 3 mods?"
          size="small"
          onClose={closeDialog}
          footer={
            <>
              <Button data-autofocus onClick={closeDialog}>
                Cancel
              </Button>
              <Button variant="destructive" onClick={closeDialog}>
                Remove 3 mods
              </Button>
            </>
          }
        >
          <p>They go to the Windows Recycle Bin. Focus starts on Cancel.</p>
        </Dialog>
      )}
      {dialog === 'busy' && (
        <Dialog
          title="Checking the transfer"
          size="small"
          busy
          onClose={closeDialog}
          footer={
            <>
              <Button disabled>Cancel</Button>
              <Button variant="primary" disabled aria-busy="true">
                Checking…
              </Button>
            </>
          }
        >
          <p>Escape does nothing while an action runs.</p>
          <Button onClick={closeDialog}>Finish</Button>
        </Dialog>
      )}
      {dialog === 'scrolling' && (
        <Dialog
          title="A body taller than the window"
          size="medium"
          onClose={closeDialog}
          footer={<Button onClick={closeDialog}>Close</Button>}
        >
          {Array.from({ length: 24 }, (_, index) => (
            <p key={index}>Line {index + 1}. The header and footer stay fixed; the body scrolls.</p>
          ))}
        </Dialog>
      )}

      <Group title="Empty and error states">
        <div className="sv-panel" style={{ width: 380, height: 220 }}>
          <EmptyState
            icon={PackageOpen}
            title="This folder has no mods"
            description="Copy mods into the folder, or pick another profile from the list."
          />
        </div>
        <div className="sv-panel" style={{ width: 380, height: 220 }}>
          <EmptyState
            tone="error"
            icon={PackageOpen}
            title="Could not read this folder"
            description="Nothing was changed. Check that the folder still exists, then try again."
            action={<Button>Try again</Button>}
          />
        </div>
      </Group>
    </div>
  );
}

function Group({
  title,
  column,
  children,
}: {
  title: string;
  column?: boolean;
  children: ReactNode;
}) {
  return (
    <section className="mm-gallery__group">
      <h2 className="sv-panel__eyebrow">{title}</h2>
      <div className={column ? 'mm-gallery__column' : 'mm-gallery__row'}>{children}</div>
    </section>
  );
}
