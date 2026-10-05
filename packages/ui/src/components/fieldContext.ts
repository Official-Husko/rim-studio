import { createContext } from 'preact';
import { useContext } from 'preact/hooks';

/** Set by FormField so a field inside it picks up the label, help and error wiring. */
export interface FieldContextValue {
  id?: string;
  describedBy?: string;
  invalid?: boolean;
  required?: boolean;
}

export const FieldContext = createContext<FieldContextValue>({});

/** Read the wiring of the surrounding FormField, if any. */
export function useFieldContext(): FieldContextValue {
  return useContext(FieldContext);
}
