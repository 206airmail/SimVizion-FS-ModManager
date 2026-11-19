// All user-facing text lives here so the app can be translated later.

import type {
  ErrorCode,
  GameStore,
  IssueKind,
  ModKind,
  ModSource,
  Severity,
  UserDirSource,
} from './api';
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
    offline:
      'The update service could not be reached. Check your internet connection and try again.',
    rateLimited:
      'The update service asked the app to wait a little. It will be asked again later; nothing was changed.',
    updatesDisabled:
      'Online update checks are switched off. Turn them on in Settings to check for updates.',
    verifyFailed:
      'The downloaded file was not what the update promised, so it was discarded. The installed mod was not changed.',
    notInstallable:
      'This update cannot be installed by the app. Open its page to download it yourself.',
    invalidLink:
      'That is not a web address the app can use. Paste a link that starts with https:// or http://.',
    notLaunchable:
      'No Farming Simulator 25 installation was found that the app can start. Start the game from its launcher.',
    invalidGameFolder:
      'That folder is not a Farming Simulator 25 user folder. Choose the folder that holds gameSettings.xml and the mods folder.',
    appUpdateFailed: 'The app update could not be checked or installed. Try again later.',
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
    update: 'Could not update the mod',
    cancelOperation: 'Could not cancel the operation',
    openLink: 'Could not open the page',
    saveLink: 'Could not save the link',
    setUpdates: 'Could not change the update setting',
    checkUpdates: 'Could not check for updates',
    saveTags: 'Could not save the tags',
    saveNote: 'Could not save the note',
    loadComparison: 'Could not compare the folders',
    prepareCopy: 'Could not prepare the copy',
    renameProfile: 'Could not rename the folder',
    removeProfile: 'Could not remove the folder from the list',
    launch: 'Could not start the game',
    chooseGameFolder: 'Could not use that folder',
    clearGameFolder: 'Could not go back to the detected folder',
    setAppUpdates: 'Could not change the app update setting',
    checkAppUpdate: 'Could not check for an app update',
    installAppUpdate: 'The update could not be installed',
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
    launchHint: 'Start Farming Simulator 25',
    launchStarting: 'Starting…',
    launchStartingReason: 'The game is starting.',
    launchBlockedRunning: 'Farming Simulator 25 is already running.',
    launchBlockedNone:
      'No Farming Simulator 25 installation was found, so the app has nothing to start.',
    launchChecking: 'Looking for the game…',
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
    compare: 'Compare',
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
    compare: 'Compare with another folder',
    searchLabel: 'Search mods',
    searchPlaceholder: 'Search title, author or file name',
    clearSearch: 'Clear search',
    kindLabel: 'Filter by type',
    kindPrefix: 'Type',
    kindAll: 'All types',
    tagLabel: 'Filter by tag',
    tagPrefix: 'Tag',
    tagAll: 'All tags',
    tagMarks: (tags: readonly string[], hasNote: boolean) =>
      [tags.length > 0 ? `Tags: ${tags.join(', ')}` : '', hasNote ? 'Has a note' : '']
        .filter(Boolean)
        .join('. '),
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

  /** The tag editor and the note field in the inspector. */
  tags: {
    title: 'Tags',
    inputLabel: 'Add a tag',
    inputLabelMany: (count: number) => `Add a tag to ${formatCount(count)} mods`,
    inputPlaceholder: 'Add a tag',
    add: 'Add tag',
    remove: (tag: string) => `Remove the tag ${tag}`,
    removeFromAll: (tag: string) => `Remove the tag ${tag} from all selected mods`,
    partial: (count: number, total: number) => `${formatCount(count)}/${formatCount(total)}`,
    partialHint: (tag: string, count: number, total: number) =>
      `${tag}: on ${formatCount(count)} of ${formatCount(total)} selected mods`,
    suggestions: 'Existing tags',
    suggestionLabel: (tag: string) => `Add the tag ${tag}`,
    none: 'No tags yet.',
    noneMany: 'None of the selected mods has a tag yet.',
    tooLong: (max: number) => `A tag can have at most ${formatCount(max)} characters.`,
    tooMany: (max: number) => `A mod can have at most ${formatCount(max)} tags.`,
    sharedHint: 'Tags and notes follow the mod into every folder.',
    noteTitle: 'Notes',
    noteLabel: 'Notes about this mod',
    notePlaceholder: 'Write a note for yourself. It saves when you leave the field.',
    noteSaving: 'Saving…',
    noteSaved: 'Saved',
    noteHint: 'Ctrl+Enter saves',
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
    appFailedTitle: 'Could not load profiles',
    loadingProfiles: 'Loading mod folders',
  },

  /** The screen that replaces the library when there is nothing to show yet (first run). */
  welcome: {
    noGameTitle: 'Farming Simulator 25 was not found',
    noGameText:
      'The app could not find the game’s user folder, the one that holds gameSettings.xml, your mods and your savegames. It looked in:',
    searched: [
      'Documents\\My Games\\FarmingSimulator2025',
      'The Microsoft Store app folder of Farming Simulator 25',
    ],
    noGameNote:
      'You can still add any mod folder and look through it. Switching the active folder and checking savegames need the game folder.',
    chooseGameFolder: 'Choose game folder…',
    noFoldersTitle: 'No mod folders yet',
    noFoldersText:
      'Add the folder where your mods are kept to see what is in it. Nothing in it is changed or moved.',
    addFolder: 'Add mod folder',
  },

  gameFolder: {
    title: 'Game folder',
    lead: 'The folder that holds gameSettings.xml, the mods folder and your savegames.',
    pathLabel: 'User folder',
    sourceLabel: 'Source',
    sources: {
      detected: 'Detected automatically',
      chosen: 'Chosen by you',
      none: 'Not found',
    } satisfies Record<UserDirSource, string>,
    storeLabel: 'Installed from',
    stores: {
      steam: 'Steam',
      giants: 'GIANTS Software',
      epic: 'Epic Games',
      microsoftStore: 'Microsoft Store',
    } satisfies Record<GameStore, string>,
    noStore: 'No installation found',
    notFound:
      'No Farming Simulator 25 user folder was found. Choose the folder that holds gameSettings.xml and the mods folder.',
    choose: 'Choose…',
    useDetected: 'Use detected folder',
    chosenNote: 'This folder is used instead of the one the app would find itself.',
  },

  health: {
    title: 'Health',
    summary: (modCount: number, folderCount: number) =>
      `${mods(modCount)} with issues in ${formatCount(folderCount)} ${plural(folderCount, 'folder', 'folders')}`,
    summaryClean: 'No mod has issues',
    rescan: 'Check again',
    folderLabel: 'Show only one mod folder',
    folderPrefix: 'Folder',
    folderAll: 'All folders',
    kindLabel: 'Kind of issue',
    kindAll: 'All issues',
    tableLabel: 'Mods with issues',
    columns: { title: 'Mod', modName: 'File name', folder: 'Folder', issues: 'Issues' },
    more: (count: number) => `+${formatCount(count)}`,
    open: (title: string, folder: string) => `Show ${title} in ${folder}`,
    emptyTitle: 'No problems found',
    emptyText: 'None of the mods in your folders have issues.',
    noMatchesTitle: 'No mods match',
    noMatchesText: 'No mod has this kind of issue in this folder.',
    clearFilters: 'Show all issues',
    loadingLabel: 'Checking your folders',
    failedTitle: 'Could not check your folders',
  },

  inspector: {
    title: 'Details',
    close: 'Close details',
    emptyTitle: 'No mod selected',
    emptyText: 'Select a mod to see its details, issues and available update.',
    multiTagsTitle: 'Tags for the selection',
    multiTitle: (count: number) => `${count} mods selected`,
    multiText: (size: string) => `${size} in total`,
    by: (author: string) => `by ${author}`,
    unknownAuthor: 'Unknown author',
    updateTitle: (version: string) => `Version ${version} is available`,
    updateText: (current: string) => `Installed: ${current}.`,
    updateAction: 'Update',
    updateHint: (version: string) => `Download version ${version} and install it`,
    updateNotInstallable:
      'The app cannot download this update itself. Get it from the mod’s page instead.',
    updateNotInstallableNoPage: 'The app cannot download this update itself.',
    multiUpdatesTitle: 'Updates',
    multiUpdatesText: (installable: number, selected: number) =>
      installable === selected
        ? `${formatCount(installable)} of the selected mods can be updated here.`
        : `${formatCount(installable)} of the ${formatCount(selected)} selected mods can be updated here. The others have no update, or one the app cannot download.`,
    updateMany: (count: number) => `Update ${count}`,
    updateManyHint: 'Update the selected mods that have an update, each in its own folder',
    sourceTitle: 'Source',
    sourceNone: 'Not linked to a page',
    openPage: (kind: 'modHub' | 'gitHub' | 'manual', label: string) =>
      kind === 'manual' ? `Open ${label}` : `Open on ${label}`,
    linkSource: 'Link source…',
    editLink: 'Edit link…',
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
    blockedGame: 'Close Farming Simulator 25 to copy, move, remove or update mods.',
    blockedServer: 'Stop the dedicated server to copy, move, remove or update mods.',
    blockedRunning: 'Another file operation is running. Wait for it to finish, or cancel it.',
    cancel: 'Cancel',

    transfer: {
      title: (mode: 'copy' | 'move', count: number) =>
        `${mode === 'copy' ? 'Copy' : 'Move'} ${mods(count)} to another folder`,
      titleTo: (mode: 'copy' | 'move', count: number, target: string) =>
        `${mode === 'copy' ? 'Copy' : 'Move'} ${mods(count)} to ${target}`,
      planning: 'Working out what the transfer needs…',
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
      title: (kind: 'copy' | 'move' | 'remove' | 'update', target: string | null) =>
        kind === 'remove'
          ? 'Removing mods to the Recycle Bin'
          : kind === 'update'
            ? target === null
              ? 'Updating mods'
              : `Updating ${target}`
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
      /** One mod in several folders (`modName` given), or several mods each in one folder. */
      updated: (count: number, modName: string | null) =>
        modName === null
          ? `Updated ${mods(count)}`
          : `Updated ${modName} in ${formatCount(count)} ${plural(count, 'folder', 'folders')}`,
      nothing: {
        copy: 'Nothing was copied',
        move: 'Nothing was moved',
        remove: 'Nothing was removed',
        update: 'Nothing was updated',
      },
      cancelled: 'Cancelled',
      skipped: (count: number) => `${formatCount(count)} skipped`,
      failed: (count: number, kind: 'copy' | 'move' | 'remove' | 'update', folders = false) =>
        `${formatCount(count)}${folders ? ` ${plural(count, 'folder', 'folders')}` : ''} could not be ${kind === 'copy' ? 'copied' : kind === 'move' ? 'moved' : kind === 'update' ? 'updated' : 'removed'}`,
      updatedNote: 'The previous versions are in the Windows Recycle Bin.',
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

  /** Compare mode: two folders in one merged table. */
  compare: {
    title: 'Compare folders',
    leftLabel: 'Left folder',
    rightLabel: 'Right folder',
    leftPrefix: 'Left',
    rightPrefix: 'Right',
    swap: 'Swap the two folders',
    rescan: 'Compare again',
    filterLabel: 'Show',
    filterDifferences: 'Differences',
    filterOnlyLeft: 'Only left',
    filterOnlyRight: 'Only right',
    filterDifferent: 'Different',
    filterAll: 'All',
    searchLabel: 'Search the comparison',
    searchPlaceholder: 'Search title or mod name',
    tableLabel: (left: string, right: string) => `${left} compared with ${right}`,
    columns: {
      mod: 'Mod',
      version: 'Version',
      size: 'Size',
      modified: 'Modified',
      state: 'State',
    },
    states: {
      onlyLeft: 'Only left',
      onlyRight: 'Only right',
      versionDiffers: 'Version differs',
      contentDiffers: 'Content differs',
      identical: 'Identical',
    },
    stateHints: {
      onlyLeft: 'This mod is only in the left folder.',
      onlyRight: 'This mod is only in the right folder.',
      versionDiffers: 'The two folders hold different versions of this mod.',
      contentDiffers: 'Same version, but the files are not the same.',
      identical: 'The same version and the same content.',
    },
    absent: 'Not in this folder',
    count: (shown: number, total: number) =>
      shown === total ? mods(total) : `${formatCount(shown)} of ${mods(total)}`,
    selected: (count: number) => `${formatCount(count)} selected`,
    nothingSelected: 'Select mods to copy or move them',
    copyRight: 'Copy →',
    copyLeft: 'Copy ←',
    moveRight: 'Move →',
    moveLeft: 'Move ←',
    transferHint: (mode: 'copy' | 'move', from: string, to: string) =>
      `${mode === 'copy' ? 'Copy' : 'Move'} the selected mods from ${from} to ${to}`,
    blockedNoSelection: 'Select one or more mods first.',
    blockedNotOnSide: (folder: string) => `None of the selected mods is in ${folder}.`,
    needTwoTitle: 'Comparing needs two folders',
    needTwoText: 'Add another mod folder with "Add mod folder", then compare the two.',
    loadingLabel: 'Comparing the folders',
    noRowsTitle: 'Nothing to show here',
    noRowsText: 'No mod fits this filter and search.',
    identicalTitle: 'The two folders match',
    identicalText: 'Every mod is in both folders, at the same version and with the same content.',
    showAll: 'Show all mods',
  },

  /** Savegames: what a save needs against one folder. */
  savegames: {
    title: 'Savegames',
    checkAgainst: 'Check against',
    folderLabel: 'Mod folder to check the savegame against',
    rescan: 'Check again',
    listLabel: 'Savegames',
    listColumns: {
      name: 'Savegame',
      map: 'Map',
      saved: 'Saved',
      played: 'Play time',
      money: 'Money',
      mods: 'Mods',
    },
    slot: (slot: number) => `Slot ${slot}`,
    unknownDate: 'Unknown',
    playTime: (minutes: number | null) => {
      if (minutes === null) return '–';
      const hours = Math.floor(minutes / 60);
      const rest = minutes % 60;
      return hours === 0 ? `${rest} min` : `${formatCount(hours)} h ${rest} min`;
    },
    money: (value: number | null) => (value === null ? '–' : formatCount(value)),
    summary: (count: number, required: number) =>
      required === 0 ? mods(count) : `${mods(count)}, ${formatCount(required)} required`,
    filterLabel: 'Show',
    filterAll: 'All',
    filterMissing: 'Missing',
    filterVersion: 'Different version',
    filterFile: 'Different file',
    filterPresent: 'Present',
    filterChecking: 'Checking',
    tableLabel: (save: string, folder: string) => `Mods of ${save} in ${folder}`,
    columns: {
      mod: 'Mod',
      saved: 'In the save',
      installed: 'In this folder',
      state: 'State',
      foundIn: 'Found in',
    },
    required: 'Required',
    states: {
      present: 'Present',
      versionDiffers: 'Different version',
      fileDiffers: 'Different file',
      hashPending: 'Checking…',
      notChecked: 'Not checked',
      missing: 'Missing',
      dlcInstalled: 'DLC installed',
      dlcMissing: 'DLC not installed',
    },
    stateHints: {
      present: 'The folder has this mod at the same version.',
      versionDiffers: 'The folder has this mod, but not the version the save was made with.',
      fileDiffers: 'The version is the same, but the file is not the one the save was made with.',
      hashPending: 'The file is being compared with the save.',
      notChecked: 'The version matches, but the file was not compared with the save.',
      missing: 'The folder does not have this mod.',
      dlcInstalled: 'This DLC is installed in the game.',
      dlcMissing: 'This DLC is not installed in the game. Get it from the game store.',
    },
    rowsEmptyTitle: 'No mods in this state',
    rowsEmptyText: 'Nothing in this savegame fits that filter.',
    showAll: 'Show all mods',
    loadingLabel: 'Reading the savegame',

    requiredMissingTitle: (title: string) => `The map mod ${title} is missing`,
    requiredMissingFound: (folders: string) =>
      `The game will not load this savegame without it. You have it in ${folders}: copy it from there with the button below.`,
    requiredMissingNowhere:
      'The game will not load this savegame without it, and it is not in any of your folders. Download the map again, then add it to this folder.',

    copyMissingTitle: 'Copy the missing mods',
    copyMissingText: 'These mods are missing here, but you have them in other folders.',
    copyFrom: (count: number, folder: string) => `Copy ${formatCount(count)} from ${folder}`,
    copyFromHint: (count: number, folder: string, target: string) =>
      `Copy ${mods(count)} from ${folder} to ${target}`,
    copyPreparing: 'Preparing…',
    notAnywhereTitle: 'Not in any of your folders',
    notAnywhereCount: (count: number) => `${mods(count)}`,
    notAnywhereShow: 'Show names',
    notAnywhereHide: 'Hide names',
    notAnywhereLabel: 'Mods that are in none of your folders',

    extrasTitle: 'Extra mods in this folder',
    extrasText: 'The savegame does not use these mods. The game still loads them.',
    extrasLabel: 'Mods in this folder that the savegame does not use',
    extrasNone: 'Every mod in this folder is used by the savegame.',

    hash: {
      label: 'File check progress',
      title: 'Checking the mod files against the save',
      preparing: 'Preparing…',
      files: (done: number, total: number) =>
        `${formatCount(done)} of ${formatCount(total)} ${plural(total, 'file', 'files')}`,
      percent: (fraction: number) => `${Math.floor(fraction * 100)}%`,
      cancel: 'Cancel',
      cancelling: 'Cancelling…',
      cancelledTitle: 'File check cancelled',
      cancelledText:
        'The versions are checked, but the files were not compared with the save. Rows marked "Checking…" are unconfirmed.',
      checkAgain: 'Check the files',
      failedTitle: 'Could not check the files',
    },

    emptyTitle: 'No savegames found',
    emptyText:
      'The game has not saved a game yet, or its user folder could not be found. Savegames appear here once the game has written one.',
    loadFailedTitle: 'Could not read the savegames',
    reportFailedTitle: 'Could not check this savegame',
    noFolders: 'Add a mod folder first. A savegame is checked against one of your folders.',
    noGameFolderTitle: 'Game folder not found',
    noGameFolderText:
      'Savegames are read from the game’s user folder, which the app could not find. Choose it in Settings.',
    openSettings: 'Open Settings',
  },

  profileMenu: {
    actions: (name: string) => `Actions for ${name}`,
    compareWith: 'Compare with…',
    compareNeedsTwo: 'Add another mod folder to compare with.',
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

  /** Online updates: Settings, the first-run prompt, the status item and the dialogs. */
  updates: {
    settings: {
      title: 'Settings',
      onlineTitle: 'Online',
      modsGroupTitle: 'Mod updates',
      switchLabel: 'Check for mod updates online',
      sent: 'When this is on, the app sends the file names of your mods to SiloAPI, a service by HLLMR Ventures, to find newer versions. For a zip whose name the service does not know, it also sends a fingerprint of the file (its SHA-256). Nothing else is sent, and nothing at all leaves your computer while this is off.',
      downloads:
        'Updates are downloaded from GitHub or ModHub only when you press Update. Your mod folders are never changed without your confirmation.',
      lastCheck: 'Last check',
      neverChecked: 'Not yet',
      retryAfter: 'Next check allowed',
      result: 'Result',
      resultText: (checked: number, byName: number, byHash: number, updates: number) =>
        `${mods(checked)} checked: ${formatCount(byName)} found by name, ${formatCount(byHash)} by file fingerprint, ${formatCount(updates)} ${plural(updates, 'has an update', 'have updates')}.`,
      fromCache: 'Nothing was sent: these are the results of the last check.',
      checkNow: 'Check now',
      checking: 'Checking…',
      off: 'Switched off. Nothing is sent and no updates are shown.',
      failedTitle: 'The update check did not finish',
      pausedText: (time: string) =>
        `The update service asked the app to wait. It will be asked again after ${time}.`,
    },
    prompt: {
      title: 'Turn on online update checks?',
      lead: 'Both are off until you turn them on. Nothing is sent while they are off.',
      mods: {
        label: 'Check for mod updates',
        detail:
          'Sends your mod file names (and a fingerprint of any zip the service does not know) to SiloAPI.',
      },
      app: {
        label: 'Check for app updates',
        detail: 'Asks GitHub once a day whether a newer version of this app is out.',
      },
      turnOn: 'Turn on',
      notNow: 'Not now',
    },
    status: {
      label: 'Update checks',
      hint: 'Open the update settings',
      off: 'Updates: Off',
      checking: 'Updates: checking…',
      notChecked: 'Updates: not checked yet',
      checked: (time: string) => `Updates: checked ${time}`,
      offline: 'Updates: Offline',
      paused: (time: string) => `Updates: paused until ${time}`,
    },
    dialog: {
      title: (modTitle: string) => `Update ${modTitle}`,
      planning: 'Working out what the update needs…',
      /** The plan's source is GitHub or ModHub; any other kind is worded as the page it is. */
      from: (source: 'modHub' | 'gitHub' | 'manual') =>
        source === 'gitHub' ? 'from GitHub' : source === 'modHub' ? 'from ModHub' : 'from its page',
      foldersLabel: 'Update in these folders',
      folderVersion: (version: string) => `Version ${version}`,
      current: 'Up to date',
      active: 'Active',
      note: 'The new file is downloaded and checked before anything is replaced. The old version goes to the Windows Recycle Bin, where you can restore it.',
      none: 'Tick at least one folder.',
      confirm: (count: number) =>
        count <= 1 ? 'Update' : `Update in ${formatCount(count)} folders`,
      cancel: 'Cancel',
      failedTitle: 'This update cannot be installed by the app',
      hint: (version: string) => `Download version ${version} and install it`,
    },
    many: {
      title: (count: number) => `Update ${mods(count)}?`,
      lead: 'Each mod is updated in the folder you are looking at. Copies in other folders stay as they are; open a mod on its own to update those too.',
      to: (from: string, to: string) => `${from} → ${to}`,
      more: (count: number) => `and ${formatCount(count)} more`,
      note: 'Each download is checked before it replaces anything. The old versions go to the Windows Recycle Bin.',
      confirm: (count: number) => `Update ${mods(count)}`,
    },
    link: {
      title: 'Link a source',
      editTitle: 'Edit the source link',
      lead: 'Paste the address of the mod’s page, for example on KingMods or itch.io. The app keeps it so you can open the page from here. It does not check that page for updates.',
      label: 'Page address',
      placeholder: 'https://',
      host: (host: string) => `Opens a page on ${host}`,
      current: (host: string) => `Currently linked to ${host}. Saving replaces it.`,
      invalid: 'Starts with https:// or http://',
      save: 'Save link',
      saving: 'Saving…',
      remove: 'Remove link',
      removing: 'Removing…',
      cancel: 'Cancel',
    },
  },

  appUpdates: {
    settings: {
      groupTitle: 'App updates',
      switchLabel: 'Check for app updates',
      sent: 'When this is on, the app asks GitHub Releases whether a newer version of SimVizion Mod Manager is out: a plain download of a small file, at most once a day. Your mods and folders are not sent.',
      downloads:
        'An update is downloaded only when you press Install and restart, and its signature is checked before it runs.',
      lastCheck: 'Last check',
      neverChecked: 'Not yet',
      result: 'Result',
      upToDate: 'You have the latest version.',
      available: (version: string) => `Version ${version} is available.`,
      checkNow: 'Check now',
      checking: 'Checking…',
      off: 'Switched off. Nothing is sent.',
      failedTitle: 'The app update did not finish',
    },
    banner: {
      title: (version: string) => `Version ${version} is available`,
      text: 'Installing closes the app and starts the new version. Your mods and settings are kept.',
      notesLabel: 'What is new',
      showNotes: 'Show notes',
      hideNotes: 'Hide notes',
      install: 'Install and restart',
      installing: 'Installing…',
      restarting: 'Restarting…',
      retry: 'Try again',
      later: 'Later',
      progress: (done: string, total: string) => `${done} of ${total}`,
      percent: (fraction: number) => `${Math.floor(fraction * 100)}%`,
      preparing: 'Downloading the update',
      progressLabel: 'Installing the update',
    },
  },

  status: {
    gameRunning: 'Game is running',
    gameNotRunning: 'Game is not running',
    settingsOk: 'gameSettings.xml readable',
    settingsMissing: 'gameSettings.xml not readable',
    overrideOn: 'Mod folder override on',
    overrideOff: 'Using the game default folder',
    store: (store: string) => `Game: ${store}`,
    noGameFolder: 'Game folder not found',
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
