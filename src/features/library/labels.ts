// Icons and tones for the domain values shown on the Library screen.

import {
  CircleX,
  FileCode,
  GitBranch,
  Info,
  Map,
  Package,
  Pencil,
  Store,
  Tractor,
  TriangleAlert,
  Warehouse,
  type LucideIcon,
} from 'lucide-react';
import type { ModKind, ModSource, Severity } from '../../api';
import { strings } from '../../strings';
import type { Tone } from '../../ui/Pill';

export const kindIcons: Record<ModKind, LucideIcon> = {
  map: Map,
  vehicle: Tractor,
  placeable: Warehouse,
  script: FileCode,
  other: Package,
};

/** Order used wherever kinds are listed. */
export const kindOrder: readonly ModKind[] = ['vehicle', 'placeable', 'script', 'map', 'other'];

export const severityIcons: Record<Severity, LucideIcon> = {
  error: CircleX,
  warning: TriangleAlert,
  info: Info,
};

export const severityTones: Record<Severity, Exclude<Tone, 'neutral' | 'success'>> = {
  error: 'error',
  warning: 'warning',
  info: 'info',
};

export const sourceIcons: Record<ModSource['type'], LucideIcon | null> = {
  unknown: null,
  modHub: Store,
  gitHub: GitBranch,
  manual: Pencil,
};

/** Full text for a source, such as "ModHub 284193" or "GitHub owner/repo". */
export function describeSource(source: ModSource): string {
  switch (source.type) {
    case 'modHub':
      return `${strings.sources.modHub} ${source.id}`;
    case 'gitHub':
      return `${strings.sources.gitHub} ${source.owner}/${source.repo}`;
    case 'manual':
      return strings.sources.manual;
    case 'unknown':
      return strings.sources.unknown;
  }
}
