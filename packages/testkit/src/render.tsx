import { fireEvent, render, type RenderResult } from '@testing-library/preact';
import type { ComponentChildren, ComponentType, VNode } from 'preact';

type Provider = ComponentType<{ children?: ComponentChildren }>;

export interface RenderOptions {
  /** Wrappers applied outermost first. */
  providers?: Provider[];
  /** Sets data-density on the root element for the test. */
  density?: 'compact' | 'comfortable' | 'roomy';
}

/** Render with the dark theme attributes set and optional providers around the tree. */
export function renderWithProviders(ui: VNode, options: RenderOptions = {}): RenderResult {
  const root = document.documentElement;
  root.setAttribute('data-theme', 'dark');
  root.setAttribute('data-density', options.density ?? 'comfortable');
  const providers = options.providers ?? [];
  const wrapped = providers.reduceRight<VNode>((child, Wrapper) => <Wrapper>{child}</Wrapper>, ui);
  return render(wrapped);
}

/** Dispatch a keydown (and keyup) for a key such as "ArrowDown" or "Escape". */
export function press(element: Element, key: string, init: KeyboardEventInit = {}): void {
  fireEvent.keyDown(element, { key, ...init });
  fireEvent.keyUp(element, { key, ...init });
}
