// Icons and tones for a savegame's mod states.

import {
  ArrowLeftRight,
  CircleCheck,
  CircleX,
  FileDiff,
  LoaderCircle,
  type LucideIcon,
} from 'lucide-react';
import type { SaveModState } from '../../api';
import type { Tone } from '../../ui/Pill';

export const saveStateView: Record<SaveModState, { tone: Tone; icon: LucideIcon }> = {
  present: { tone: 'success', icon: CircleCheck },
  versionDiffers: { tone: 'warning', icon: ArrowLeftRight },
  fileDiffers: { tone: 'warning', icon: FileDiff },
  hashPending: { tone: 'neutral', icon: LoaderCircle },
  missing: { tone: 'error', icon: CircleX },
  dlcInstalled: { tone: 'success', icon: CircleCheck },
  dlcMissing: { tone: 'error', icon: CircleX },
};
