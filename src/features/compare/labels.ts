// Icons and tones for the compare states.

import {
  ArrowLeftRight,
  CircleCheck,
  FileDiff,
  PanelLeft,
  PanelRight,
  type LucideIcon,
} from 'lucide-react';
import type { CompareState } from '../../api';
import type { Tone } from '../../ui/Pill';

/** A mod that is only on one side is a fact (info); a difference is a warning; identical is fine. */
export const compareStateView: Record<CompareState, { tone: Tone; icon: LucideIcon }> = {
  onlyLeft: { tone: 'info', icon: PanelLeft },
  onlyRight: { tone: 'info', icon: PanelRight },
  versionDiffers: { tone: 'warning', icon: ArrowLeftRight },
  contentDiffers: { tone: 'warning', icon: FileDiff },
  identical: { tone: 'success', icon: CircleCheck },
};
