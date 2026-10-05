/** The browser features the styles rely on. */
export interface ProbeEnv {
  css?: { supports(condition: string): boolean };
  hasLayerRule: boolean;
  hasPropertyRule: boolean;
}

function currentEnv(): ProbeEnv {
  return {
    css: typeof CSS === 'undefined' ? undefined : CSS,
    hasLayerRule: typeof CSSLayerBlockRule !== 'undefined',
    hasPropertyRule: typeof CSSPropertyRule !== 'undefined',
  };
}

/** Names of the missing features; an empty list means the webview is recent enough. */
export function probeFeatures(env: ProbeEnv = currentEnv()): string[] {
  const missing: string[] = [];
  const supports = (c: string): boolean => {
    try {
      return env.css?.supports(c) ?? false;
    } catch {
      return false;
    }
  };
  if (!env.hasLayerRule) missing.push('@layer');
  if (!env.hasPropertyRule) missing.push('@property');
  if (!supports('color: color-mix(in srgb, red, blue)')) missing.push('color-mix()');
  if (!supports('container-type: inline-size')) missing.push('container queries');
  if (!supports('selector(:has(*))')) missing.push(':has()');
  return missing;
}
