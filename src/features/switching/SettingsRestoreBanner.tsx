import { formatDateTime } from '../../lib/format';
import { strings } from '../../strings';
import { Button } from '../../ui/Button';
import { Banner } from '../../ui/Feedback';
import { ActionErrorBanner } from '../ActionErrorBanner';
import type { SettingsRestore } from './useSettingsRestore';

const t = strings.restore;

/**
 * App-level notices about gameSettings.xml: the offer to restore the newest backup while the file
 * is unreadable, the reason a restore failed, and a confirmation once it worked.
 */
export function SettingsRestoreBanner({ restore }: { restore: SettingsRestore }) {
  return (
    <>
      {restore.offerFrom !== null && (
        <Banner
          tone="error"
          title={t.title}
          actions={
            <Button
              variant="primary"
              size="compact"
              disabled={restore.busy}
              aria-busy={restore.busy || undefined}
              onClick={() => void restore.restore()}
            >
              {restore.busy ? t.busy : t.action(formatDateTime(restore.offerFrom))}
            </Button>
          }
        >
          {t.text}
        </Banner>
      )}
      {restore.error !== null && (
        <ActionErrorBanner
          title={strings.actionFailed.restoreBackup}
          message={restore.error}
          onDismiss={restore.dismissError}
        />
      )}
      {restore.restored && (
        <Banner
          tone="success"
          title={t.doneTitle}
          actions={
            <Button size="compact" onClick={restore.dismissRestored}>
              {strings.actionFailed.dismiss}
            </Button>
          }
        >
          {t.doneText}
        </Banner>
      )}
    </>
  );
}
