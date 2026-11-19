import { useCallback, useState } from 'react';
import { describeError } from './errors';

/**
 * Runs one-off user actions (open a folder, add a profile) and keeps the worded error of the
 * latest failure, so the screen can show a dismissible banner instead of breaking.
 * `scope` names what the screen is showing (a profile, a mod): an error is only reported while
 * the scope is the one it happened in, so it does not follow the user to another profile.
 */
export function useActionError(scope = '') {
  const [failure, setFailure] = useState<{ scope: string; message: string } | null>(null);

  const run = useCallback(
    async <T>(action: () => Promise<T>): Promise<T | undefined> => {
      setFailure(null);
      try {
        return await action();
      } catch (error) {
        setFailure({ scope, message: describeError(error) });
        return undefined;
      }
    },
    [scope],
  );
  const dismiss = useCallback(() => setFailure(null), []);

  return { message: failure?.scope === scope ? failure.message : null, run, dismiss };
}
