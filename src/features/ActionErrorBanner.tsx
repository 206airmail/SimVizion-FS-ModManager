import { strings } from '../strings';
import { Button } from '../ui/Button';
import { Banner } from '../ui/Feedback';

export interface ActionErrorBannerProps {
  /** What the user tried to do, e.g. `strings.actionFailed.openFolder`. */
  title: string;
  /** The worded reason (from `describeError`). */
  message: string;
  onDismiss: () => void;
  className?: string;
}

/** A failed one-off action. The screen stays as it was; the user can read the reason and dismiss it. */
export function ActionErrorBanner({
  title,
  message,
  onDismiss,
  className,
}: ActionErrorBannerProps) {
  return (
    <Banner
      tone="error"
      title={title}
      className={className}
      actions={
        <Button size="compact" onClick={onDismiss}>
          {strings.actionFailed.dismiss}
        </Button>
      }
    >
      {message}
    </Banner>
  );
}
