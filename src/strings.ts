// All user-facing text lives here so the app can be translated later.

import type { ErrorCode, IssueKind, ModKind, ModSource, Severity } from './api';
import { formatCount } from './lib/format';

const plural = (count: number, one: string, many: string) => (count === 1 ? one : many);
const mods = (count: number) => `${formatCount(count)} ${plural(count, 'mod', 'mods')}`;

/** Reason codes the core puts in the detail of a `notAMod` issue. */
const notAModDetail: ReadonlyMap<string, string> = new Map([
  ['unreadableArchive', 'The archive could not be opened'],
  ['noModDesc', 'No modDesc.xml was found'],
  ['invalidModDesc', 'modDesc.xml could not be read'],
]);

export const strings = {
  app: {
    name: 'SimVizion',
    product: 'Mod Manager',
    subtitle: 'Farming Simulator 25',
    version: 'Version 0.1.0',
    sampleData: 'Sample data',
    sampleDataHint: 'This view shows generated sample mods, not your real folders.',
  },

  errors: {
    profileNotFound: 'That mod folder is no longer in your list.',
    folderMissing: 'The folder could not be found. It may have been moved or deleted.',
    folderUnreadable: 'The folder could not be read. You may not have permission to open it.',
    internal: 'Something went wrong inside the app.',
    gameRunning:
      'Farming Simulator 25 is running, so nothing was changed. Close the game and try again.',
    serverRunning:
      'The dedicated server is running, so nothing was changed. Stop the server and try again.',
    settingsMissing:
      'gameSettings.xml was not found, so nothing was changed. Start the game once so it creates the file.',
    settingsUnreadable: 'gameSettings.xml could not be read, so nothing was changed.',
    settingsChanged:
      'gameSettings.xml changed in the meantime, so nothing was written. Check which folder is active, then try again if you still want the change.',
    writeFailed:
      'gameSettings.xml could not be written, so the original file is unchanged. Check that the file is not read-only.',
    writesDisabled: 'This development build does not write game files, so nothing was changed.',
    nothingToUndo: 'There is nothing to undo any more, so nothing was changed.',
    noBackup: 'No backup of gameSettings.xml was found, so nothing was restored.',
    recycleUnavailable:
      'Windows cannot put this in the Recycle Bin (it may be too large, or the drive has no Recycle Bin), so it was left in place.',
    notEnoughSpace: 'The target drive does not have enough free space, so nothing was copied.',
    busy: 'Another file operation is still running. Wait for it to finish.',
    cancelled: 'Cancelled. Mods not yet finished were left as they were.',
    targetChanged:
      'The mod or the target folder changed since the plan was made, so it was skipped.',
    cannotRemoveProfile: 'The active folder and the game default folder stay in the list.',
    savegameMissing: 'That savegame could not be found. It may have been deleted in the game.',
    savegameUnreadable: 'That savegame could not be read. The game may still be saving it.',
  } satisfies Record<ErrorCode, string>,

  actionFailed: {
    addFolder: 'Could not add the folder',
    openFolder: 'Could not open the folder',
    showInFolder: 'Could not show the mod in its folder',
    setActive: 'Could not change the active folder',
    undoSwitch: 'Could not undo the switch',
    restoreBackup: 'Could not restore the backup',
    planTransfer: 'Could not check the transfer',
    copy: 'Could not copy the mods',
    move: 'Could not move the mods',
    remove: 'Could not remove the mods',
    cancelOperation: 'Could not cancel the operation',
    renameProfile: 'Could not rename the folder',
    removeProfile: 'Could not remove the folder from the list',
    dismiss: 'Dismiss',
  },

  switching: {
    busy: 'Switching…',
    blockedGame: 'Close Farming Simulator 25 to change the active folder.',
    blockedServer: 'Stop the dedicated server to change the active folder.',
    doneTitle: (name: string) => `${name} is now the active folder`,
    doneText:
      'The game loads its mods from this folder the next time it starts. gameSettings.xml was backed up first.',
    undo: 'Undo',
    undoing: 'Undoing…',
    undoneTitle: 'Switch undone',
    undoneText: (name: string | null) =>
      name === null
        ? 'The previous active folder is back in place.'
        : `${name} is the active folder again.`,
  },

  restore: {
    title: 'gameSettings.xml could not be read',
    text: 'The game cannot load its settings from this file. Restoring puts back the newest backup, and the unreadable file is kept as a backup too.',
    action: (when: string) => `Restore the backup from ${when}`,
    busy: 'Restoring…',
    doneTitle: 'Backup restored',
    doneText: 'gameSettings.xml was replaced with the backup and can be read again.',
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
    readingMods: (done: number, total: number) => `Reading ${formatCount(done)} of ${mods(total)}`,
    readingPercent: (fraction: number) => `${Math.floor(fraction * 100)}%`,
    loadFailedTitle: 'Could not read this folder',
    loadFailedText: (message: string) =>
      `${message} Nothing was changed, so it is safe to try again.`,
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
    moveTo: 'Move to…',
    copyHint: 'Copy the selected mods to another folder',
    moveHint: 'Move the selected mods to another folder',
    removeHint: 'Send the selected mods to the Windows Recycle Bin',
    remove: 'Remove',
    unknown: 'Unknown',
  },

  /** Copy, move and remove: the dialogs, the progress and result banners, and why they are blocked. */
  fileOps: {
    blockedGame: 'Close Farming Simulator 25 to copy, move or remove mods.',
    blockedServer: 'Stop the dedicated server to copy, move or remove mods.',
    blockedRunning: 'Another file operation is running. Wait for it to finish, or cancel it.',
    cancel: 'Cancel',

    transfer: {
      title: (mode: 'copy' | 'move', count: number) =>
        `${mode === 'copy' ? 'Copy' : 'Move'} ${mods(count)} to another folder`,
      targetLabel: 'Target folder',
      targetEmpty:
        'There is no other mod folder to send these to. Add one first with "Add mod folder".',
      targetMods: (count: number) => mods(count),
      modeLabel: 'Action',
      modeCopy: 'Copy',
      modeMove: 'Move',
      modeCopyHint: 'The originals stay in this folder.',
      modeMoveHint: 'The mods leave this folder once they are safely in the target.',
      next: 'Continue',
      back: 'Back',
      checking: 'Checking…',
      planMods: 'Mods',
      planSize: 'Total size',
      planFree: 'Free space on the target drive',
      sameDrive: 'Same drive: a move is instant and needs no extra space.',
      noSpaceTitle: 'Not enough free space',
      noSpaceText: (needed: string, free: string) =>
        `This needs ${needed}, but the target drive has ${free} free. Free up some space or choose another folder.`,
      clashTitle: (count: number, target: string) =>
        `${count === 1 ? '1 mod already exists' : `${formatCount(count)} mods already exist`} in ${target}`,
      clashText:
        'Choose what happens to each one. Replace sends the existing version to the Windows Recycle Bin. Skip leaves it alone and does not copy the incoming one.',
      clashTableLabel: 'Mods that already exist in the target folder',
      colMod: 'Mod',
      colIncoming: 'Incoming',
      colExisting: 'Already there',
      colChoice: 'Action',
      choiceReplace: 'Replace',
      choiceSkip: 'Skip',
      choiceLabel: (name: string) => `Action for ${name}`,
      applyAll: 'Apply to all',
      replaceAll: 'Replace all',
      skipAll: 'Skip all',
      noVersion: 'No version',
      confirm: (mode: 'copy' | 'move', count: number) =>
        count === 0 ? `Nothing to ${mode}` : `${mode === 'copy' ? 'Copy' : 'Move'} ${mods(count)}`,
      confirmNote: (count: number, skipped: number) =>
        count === 0
          ? 'Every mod is set to Skip, so there is nothing to do.'
          : skipped === 0
            ? `${mods(count)} will be transferred.`
            : `${mods(count)} will be transferred and ${formatCount(skipped)} skipped.`,
    },

    remove: {
      title: (count: number) => `Remove ${mods(count)}?`,
      lead: (count: number, size: string, profile: string) =>
        `${mods(count)} (${size}) will be removed from ${profile}.`,
      more: (count: number) => `and ${formatCount(count)} more`,
      recycle: 'They go to the Windows Recycle Bin, where you can restore them.',
      leftInPlace:
        'A mod that cannot be recycled is left where it is. Nothing is deleted outright.',
      synced: 'If this folder is synced, the removal syncs to your other devices too.',
      confirm: (count: number) => `Remove ${mods(count)}`,
    },

    progress: {
      label: 'File operation progress',
      title: (kind: 'copy' | 'move' | 'remove', target: string | null) =>
        kind === 'remove'
          ? 'Removing mods to the Recycle Bin'
          : `${kind === 'copy' ? 'Copying' : 'Moving'} mods${target === null ? '' : ` to ${target}`}`,
      preparing: 'Preparing…',
      bytes: (done: string, total: string) => `${done} of ${total}`,
      items: (done: number, total: number) => `${formatCount(done)} of ${mods(total)}`,
      cancel: 'Cancel',
      cancelling: 'Cancelling…',
      percent: (fraction: number) => `${Math.floor(fraction * 100)}%`,
    },

    result: {
      copied: (count: number, target: string) => `Copied ${mods(count)} to ${target}`,
      moved: (count: number, target: string) => `Moved ${mods(count)} to ${target}`,
      removed: (count: number) => `Removed ${mods(count)}`,
      nothing: {
        copy: 'Nothing was copied',
        move: 'Nothing was moved',
        remove: 'Nothing was removed',
      },
      cancelled: 'Cancelled',
      skipped: (count: number) => `${formatCount(count)} skipped`,
      failed: (count: number, kind: 'copy' | 'move' | 'remove') =>
        `${formatCount(count)} could not be ${kind === 'copy' ? 'copied' : kind === 'move' ? 'moved' : 'removed'}`,
      notDone: (count: number) => `${formatCount(count)} not done because it was cancelled`,
      allSkipped: 'Every mod was skipped.',
      recycleNote:
        'Removed mods are in the Windows Recycle Bin. Restore them from there if you need them back.',
      replacedNote: (count: number) =>
        `${count === 1 ? 'The replaced mod is' : `${formatCount(count)} replaced mods are`} in the Windows Recycle Bin.`,
      showDetails: (count: number) =>
        `Show ${formatCount(count)} ${plural(count, 'detail', 'details')}`,
      hideDetails: 'Hide details',
      detailsLabel: 'Mods that were not done',
    },
  },

  profileMenu: {
    actions: (name: string) => `Actions for ${name}`,
    rename: 'Rename…',
    remove: 'Remove from list…',
    blockedActive: 'The active folder stays in the list. Make another folder active first.',
    blockedDefault: 'The game default folder stays in the list.',
    blockedRunning: 'A file operation is running. Wait for it to finish, or cancel it.',
    renameTitle: 'Rename mod folder',
    renameLabel: 'Name in the list',
    renameHelp: 'Only the name here changes. The folder on disk keeps its name.',
    renameEmpty: 'Enter a name.',
    renameTooLong: (max: number) => `Use at most ${formatCount(max)} characters.`,
    renameAction: 'Rename',
    renaming: 'Renaming…',
    removeTitle: (name: string) => `Remove ${name} from the list?`,
    removeText:
      'The folder and its mods are not touched. You can add the folder again later with "Add mod folder".',
    removeAction: 'Remove from list',
    removing: 'Removing…',
    cancel: 'Cancel',
  },

  status: {
    gameRunning: 'Game is running',
    gameNotRunning: 'Game is not running',
    settingsOk: 'gameSettings.xml readable',
    settingsMissing: 'gameSettings.xml not readable',
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

  /**
   * One sentence from the data in `ModIssue.detail` (see "Issue details carry data, not
   * sentences" in docs/dev/decisions.md). An absent or unrecognised detail gives null.
   */
  issueDetail: {
    notAMod: (detail) => notAModDetail.get(detail ?? '') ?? null,
    invalidFileName: () => null,
    nestedArchive: (detail) => (detail ? `Contains ${detail} inside the archive` : null),
    zipAndFolderTwin: (detail) => (detail ? `Same name as ${detail}` : null),
    conflictCopy: (detail) => (detail ? `Copy of ${detail}` : null),
    descVersionMismatch: (detail) =>
      detail !== null && /^\d+$/.test(detail)
        ? `Newer than the game's descVersion ${detail}`
        : null,
    missingDependency: (detail) =>
      detail
        ? `Requires ${detail}, which ${detail.includes(',') ? 'are' : 'is'} not in this profile`
        : null,
  } satisfies Record<IssueKind, (detail: string | null) => string | null>,

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
