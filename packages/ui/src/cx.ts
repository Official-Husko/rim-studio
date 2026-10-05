/** Join class names, skipping falsy values. Class names stay static strings at call sites. */
export function cx(...parts: Array<string | false | null | undefined>): string {
  return parts.filter((p): p is string => typeof p === 'string' && p.length > 0).join(' ');
}
