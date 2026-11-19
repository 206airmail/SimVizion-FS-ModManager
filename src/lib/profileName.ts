/** The longest display name a profile may have. */
export const PROFILE_NAME_MAX = 100;

export type ProfileNameProblem = 'empty' | 'tooLong';

/** Whether a name typed for a profile can be used; null when it can. Counts characters, not bytes. */
export function validateProfileName(name: string): ProfileNameProblem | null {
  const trimmed = name.trim();
  if (trimmed === '') return 'empty';
  if ([...trimmed].length > PROFILE_NAME_MAX) return 'tooLong';
  return null;
}
