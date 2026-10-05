import { createContext } from 'preact';
import { useContext } from 'preact/hooks';
import type { DesignSpecDto, DiagnosticDto, StatPoolDto, SuggestionDto } from 'rimstudio-ipc-types';
import type { Pointer } from '../../model/pointer';

/** What every field of the form needs from the open draft and its live results. */
export interface FieldEnv {
  spec: DesignSpecDto;
  suggestions: ReadonlyMap<Pointer, SuggestionDto>;
  pools: ReadonlyMap<string, StatPoolDto>;
  diagnostics: ReadonlyMap<Pointer, DiagnosticDto[]>;
  /** Write a value into the spec; undefined removes it. */
  setField: (pointer: Pointer, value: unknown) => void;
  /** Give the weapon its own projectile (true) or point it back at the shared one (false). */
  setOwnProjectile: (own: boolean) => void;
  /** True while the backend builds the projectile. */
  ownBusy: boolean;
  /** What the backend said about the last change of the projectile. */
  projectileNotes: readonly string[];
  /** Roles seen in the reference list, for the role picker. */
  roles: readonly string[];
}

export const FieldEnvContext = createContext<FieldEnv | undefined>(undefined);

/** The environment of the surrounding editor; a field outside an editor is a programming error. */
export function useFieldEnv(): FieldEnv {
  const env = useContext(FieldEnvContext);
  if (!env) throw new Error('A designer field was rendered outside an editor.');
  return env;
}
