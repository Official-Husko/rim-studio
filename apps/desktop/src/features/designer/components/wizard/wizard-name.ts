import { signal } from '@preact/signals';
import { suggestDefName } from './wizard-model';
import { readPrefix, writePrefix } from './wizard-prefs';

/**
 * The label, the def name prefix and the def name of the new weapon. The def name follows the
 * label until the user types one; clearing it makes it follow the label again.
 */
export function createNameState() {
  const label = signal('');
  const prefix = signal(readPrefix());
  const defName = signal('');
  const edited = signal(false);

  function setLabel(text: string): void {
    label.value = text;
    if (!edited.peek()) defName.value = suggestDefName(prefix.peek(), text);
  }

  function setPrefix(text: string): void {
    prefix.value = text;
    writePrefix(text);
    if (!edited.peek()) defName.value = suggestDefName(text, label.peek());
  }

  function setDefName(text: string): void {
    edited.value = text !== '';
    defName.value = text === '' ? suggestDefName(prefix.peek(), label.peek()) : text;
  }

  /** Forget the label and the def name; the prefix is kept. */
  function reset(): void {
    label.value = '';
    defName.value = '';
    edited.value = false;
  }

  return { label, prefix, defName, setLabel, setPrefix, setDefName, reset };
}
