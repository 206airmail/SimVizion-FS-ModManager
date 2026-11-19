// All user-facing text lives here so the app can be translated later.

import type { IssueKind, ModKind, ModSource, Severity } from './api';
import { formatCount } from './lib/format';

const plural = (count: number, one: string, many: string) => (count === 1 ? one : many);
const mods = (count: number) => `${formatCount(count)} ${plural(count, 'mod', 'mods')}`;

export const strings = {
  app: {
    name: 'SimVizion',
    product: 'Mod Manager',
    subtitle: 'Farming Simulator 25',
    version: 'Prototype 0.1.0',
    sampleData: 'Sample data',
    sampleDataHint: 'This prototype shows generated sample mods, not your real folders.',
  },

  header: {
    activeProfile: 'Active profile',
    noActiveProfile: 'Game default',
    game: 'Game',
    gameRunning: 'Running',
    gameNotRunning: 'Not running',
    gameVersion: (version: string) => `Version ${version}`,
    launch: 'Launch game',
  },

  rail: {
    profiles: 'Profiles',
    addFolder: 'Add mod folder',
    active: 'Active',
    summary: (count: number, size: string) => `${mods(count)} · ${size}`,
    issues: (count: number) => `${count} ${plural(count, 'mod has', 'mods have')} issues`,
    updates: (count: number) => `${count} ${plural(count, 'update', 'updates')} available`,
    tools: 'Tools',
    health: 'Health',
    savegames: 'Savegames',
    settings: 'Settings',
    totals: 'All profiles',
    totalProfiles: 'Profiles',
    totalMods: 'Mods',
    totalSize: 'Size on disk',
    totalIssues: 'With issues',
  },

  library: {
    active: 'Active',
    gameDefault: 'Game default folder',
    setActive: 'Set as active',
    openFolder: 'Open folder',
    rescan: 'Rescan folder',
    searchLabel: 'Search mods',
    searchPlaceholder: 'Search title, author or file name',
    clearSearch: 'Clear search',
    kindLabel: 'Filter by type',
    kindPrefix: 'Type',
    kindAll: 'All types',
    statusLabel: 'Filter by status',
    statusAll: 'All',
    statusIssues: 'Issues',
    statusUpdates: 'Updates',
    viewLabel: 'View',
    viewList: 'List view',
    viewGrid: 'Grid view',
    showDetails: 'Show details',
    hideDetails: 'Hide details',
    count: (shown: number, total: number) =>
      shown === total ? mods(total) : `${formatCount(shown)} of ${mods(total)}`,
    selected: (count: number) => `${formatCount(count)} selected`,
    clearSelection: 'Clear selection',
    tableLabel: (profile: string) => `Mods in ${profile}`,
    columns: {
      title: 'Title',
      author: 'Author',
      version: 'Version',
      kind: 'Type',
      size: 'Size',
      source: 'Source',
      status: 'Status',
    },
    folderFormat: 'Folder',
    update: 'Update',
    updateTo: (version: string) => `Update available: ${version}`,
    issueCount: (count: number) => `${count} ${plural(count, 'issue', 'issues')}`,
    noIcon: 'No icon',
  },

  states: {
    loadingMods: 'Loading mods',
    loadFailedTitle: 'Could not read this folder',
    loadFailedText: (message: string) =>
      `${message}. Nothing was changed. Check that the folder still exists, then try again.`,
    retry: 'Try again',
    emptyFolderTitle: 'This folder has no mods',
    emptyFolderText: 'Copy mods into the folder, or pick another profile from the list.',
    noMatchesTitle: 'No mods match',
    noMatchesText: 'Nothing in this profile fits the current search and filters.',
    clearFilters: 'Clear search and filters',
    notBuiltTitle: (section: string) => `${section} is not built yet`,
    notBuiltText:
      'This prototype covers the Library screen. Other sections arrive in later milestones.',
    backToLibrary: 'Back to the library',
    appFailedTitle: 'Could not load profiles',
  },

  inspector: {
    title: 'Details',
    close: 'Close details',
    emptyTitle: 'No mod selected',
    emptyText: 'Select a mod to see its details, issues and available update.',
    multiTitle: (count: number) => `${count} mods selected`,
    multiText: (size: string) => `${size} in total`,
    by: (author: string) => `by ${author}`,
    unknownAuthor: 'Unknown author',
    updateTitle: (version: string) => `Version ${version} is available`,
    updateText: (current: string, source: string) => `Installed: ${current}. Source: ${source}.`,
    updateAction: 'Update',
    issues: 'Issues',
    description: 'Description',
    noDescription: 'This mod has no description.',
    details: 'Details',
    fileName: 'File',
    format: 'Format',
    formatZip: 'ZIP archive',
    formatFolder: 'Unpacked folder',
    size: 'Size',
    modified: 'Modified',
    descVersion: 'descVersion',
    descVersionGame: (game: number) => `game reported ${game}`,
    multiplayer: 'Multiplayer',
    yes: 'Supported',
    no: 'Not supported',
    source: 'Source',
    linkSource: 'Link source',
    alsoIn: 'Also in',
    onlyHere: 'Only in this profile',
    dependencies: 'Requires',
    dependencyOk: 'Installed in this profile',
    dependencyMissing: 'Missing from this profile',
    showInFolder: 'Show in folder',
    copyTo: 'Copy to…',
    remove: 'Remove',
    unknown: 'Unknown',
  },

  status: {
    gameRunning: 'Game is running',
    gameNotRunning: 'Game is not running',
    settingsOk: 'gameSettings.xml readable',
    settingsMissing: 'gameSettings.xml not found',
    overrideOn: 'Mod folder override on',
    overrideOff: 'Using the game default folder',
  },

  gallery: {
    title: 'Component gallery',
    back: 'Back to the app',
  },

  kinds: {
    map: 'Map',
    vehicle: 'Vehicle',
    placeable: 'Placeable',
    script: 'Script',
    other: 'Other',
  } satisfies Record<ModKind, string>,

  kindsPlural: {
    map: 'Maps',
    vehicle: 'Vehicles',
    placeable: 'Placeables',
    script: 'Scripts',
    other: 'Other',
  } satisfies Record<ModKind, string>,

  issues: {
    notAMod: 'Not a mod',
    invalidFileName: 'Invalid file name',
    nestedArchive: 'Archive of archives',
    zipAndFolderTwin: 'ZIP and folder with the same name',
    conflictCopy: 'Duplicate copy',
    descVersionMismatch: 'descVersion differs from the game',
    missingDependency: 'Missing required mod',
  } satisfies Record<IssueKind, string>,

  issueHelp: {
    notAMod: 'No modDesc.xml at the top level, or the archive cannot be read. The game ignores it.',
    invalidFileName:
      'The game only loads names made of letters, digits and underscores that do not start with a digit.',
    nestedArchive: 'This archive contains other mod archives and needs unpacking once.',
    zipAndFolderTwin:
      'The game loads the folder and ignores the ZIP. Folder mods cannot be used in multiplayer.',
    conflictCopy: 'A sync conflict or renamed copy of another mod. The game ignores it.',
    descVersionMismatch: 'The mod may have been made for a different game version.',
    missingDependency: 'A mod this one requires is not in this profile.',
  } satisfies Record<IssueKind, string>,

  severity: {
    error: 'Error',
    warning: 'Warning',
    info: 'Note',
  } satisfies Record<Severity, string>,

  sources: {
    unknown: 'Not linked',
    modHub: 'ModHub',
    gitHub: 'GitHub',
    manual: 'Manual',
  } satisfies Record<ModSource['type'], string>,
} as const;
