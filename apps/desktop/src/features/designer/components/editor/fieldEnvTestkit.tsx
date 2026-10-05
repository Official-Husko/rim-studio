import type { ComponentChildren } from 'preact';
import { vi, type Mock } from 'vitest';
import type { DesignSpecDto, DiagnosticDto, StatPoolDto, SuggestionDto } from 'rimstudio-ipc-types';
import { groupByField, suggestionsByField } from '../../model/draft';
import { newDraft } from '../../model/draft';
import { FieldEnvContext, type FieldEnv } from './fieldEnv';

export interface EnvOptions {
  spec?: DesignSpecDto;
  suggestions?: SuggestionDto[];
  pools?: StatPoolDto[];
  diagnostics?: DiagnosticDto[];
  roles?: string[];
  ownBusy?: boolean;
  projectileNotes?: string[];
}

/** A field environment for component tests; setField is a spy. */
type SetField = (pointer: string, value: unknown) => void;

export function makeEnv(
  options: EnvOptions = {},
): FieldEnv & { setField: Mock<SetField>; setOwnProjectile: Mock<(own: boolean) => void> } {
  return {
    spec: options.spec ?? newDraft('ranged', 'TM_Gun', 'gun').spec,
    suggestions: suggestionsByField(options.suggestions),
    pools: new Map((options.pools ?? []).map((p) => [p.stat, p])),
    diagnostics: groupByField(options.diagnostics ?? []),
    roles: options.roles ?? [],
    setField: vi.fn<SetField>(),
    setOwnProjectile: vi.fn<(own: boolean) => void>(),
    ownBusy: options.ownBusy ?? false,
    projectileNotes: options.projectileNotes ?? [],
  };
}

/** Provide an environment around a field under test. */
export function WithEnv({ env, children }: { env: FieldEnv; children: ComponentChildren }) {
  return <FieldEnvContext.Provider value={env}>{children}</FieldEnvContext.Provider>;
}
