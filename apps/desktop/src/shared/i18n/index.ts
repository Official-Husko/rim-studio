import { signal } from '@preact/signals';
import en from '~/locales/en.json';

/** Every key of the English catalog; a typo in a key fails to compile. */
export type MessageKey = keyof typeof en;

/** The active locale. Only English exists; other catalogs will load lazily. */
export const locale = signal<'en'>('en');

const catalogs: Record<'en', Record<string, string>> = { en };

/** Fill {name} placeholders. Unknown placeholders stay visible so a missing argument is noticed. */
function fill(template: string, args?: Record<string, string | number>): string {
  if (!args) return template;
  return template.replace(/\{(\w+)\}/g, (whole, name: string) => {
    const value = args[name];
    return value === undefined ? whole : String(value);
  });
}

/** Translate a key. Reading the locale signal here makes components update on a locale change. */
export function t(key: MessageKey, args?: Record<string, string | number>): string {
  const catalog = catalogs[locale.value];
  return fill(catalog[key] ?? catalogs.en[key] ?? key, args);
}

/** Plural form: looks up `<key>.one` or `<key>.other` by the count and fills {n}. */
export function tn(key: string, count: number, args: Record<string, string | number> = {}): string {
  const form = (count === 1 ? `${key}.one` : `${key}.other`) as MessageKey;
  return t(form, { ...args, n: count });
}
