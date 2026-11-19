import { ExternalLink } from 'lucide-react';
import type { ModEntry } from '../../api';
import { api } from '../../api/client';
import { useActionError } from '../../lib/useActionError';
import { strings } from '../../strings';
import { Button } from '../../ui/Button';
import { ActionErrorBanner } from '../ActionErrorBanner';

/**
 * One button per page the mod can be opened on ("Open on ModHub", "Open on GitHub", "Open
 * <host>"). The app only names the mod and the kind; the core knows the address.
 */
export function PageLinks({ mod }: { mod: ModEntry }) {
  const open = useActionError(`${mod.profileId}/${mod.id}`);
  if (mod.links.length === 0) return null;
  return (
    <>
      <div className="mm-pagelinks">
        {mod.links.map((link) => (
          <Button
            key={link.kind}
            size="compact"
            icon={ExternalLink}
            onClick={() => void open.run(() => api.openModLink(mod.modName, link.kind))}
          >
            <span className="sv-truncate">{strings.inspector.openPage(link.kind, link.label)}</span>
          </Button>
        ))}
      </div>
      {open.message !== null && (
        <ActionErrorBanner
          title={strings.actionFailed.openLink}
          message={open.message}
          onDismiss={open.dismiss}
        />
      )}
    </>
  );
}
