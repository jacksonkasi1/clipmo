// ** import types
import type { ItemKind } from './types';

export type FilterShortcutTarget = 'navigation' | 'all' | 'favorites' | ItemKind;

export interface FilterShortcutDefinition {
  target: FilterShortcutTarget;
  label: string;
  description: string;
  defaultShortcut: string;
}

export const FILTER_SHORTCUTS: FilterShortcutDefinition[] = [
  { target: 'navigation', label: 'Toggle navigation', description: 'Open or close the filter sidebar.', defaultShortcut: 'Ctrl+B' },
  { target: 'all', label: 'All history', description: 'Show every clipboard category.', defaultShortcut: 'Alt+1' },
  { target: 'favorites', label: 'Favorites', description: 'Show starred clipboard items.', defaultShortcut: 'Alt+2' },
  { target: 'text', label: 'Text filter', description: 'Show text clipboard items.', defaultShortcut: 'Alt+3' },
  { target: 'image', label: 'Image filter', description: 'Show image clipboard items.', defaultShortcut: 'Alt+4' },
  { target: 'link', label: 'Link filter', description: 'Show URL clipboard items.', defaultShortcut: 'Alt+5' },
  { target: 'files', label: 'File filter', description: 'Show file clipboard items.', defaultShortcut: 'Alt+6' },
  { target: 'email', label: 'Email filter', description: 'Show detected email addresses.', defaultShortcut: 'Alt+7' },
  { target: 'color', label: 'Color filter', description: 'Show detected color values.', defaultShortcut: 'Alt+8' },
];

export const DEFAULT_FILTER_SHORTCUTS = FILTER_SHORTCUTS.map((item) => item.defaultShortcut);

export function resolvedFilterShortcuts(configured?: string[]): string[] {
  return FILTER_SHORTCUTS.map((definition, index) => configured?.[index] ?? definition.defaultShortcut);
}

export function matchesShortcut(event: KeyboardEvent, shortcut: string): boolean {
  const tokens = shortcut.split('+').map((token) => token.trim().toLowerCase()).filter(Boolean);
  const key = tokens.find((token) => !['ctrl', 'control', 'shift', 'alt', 'win', 'super', 'meta'].includes(token));
  if (!key) return false;
  const eventKey = event.key.toLowerCase();
  return event.ctrlKey === (tokens.includes('ctrl') || tokens.includes('control'))
    && event.shiftKey === tokens.includes('shift')
    && event.altKey === tokens.includes('alt')
    && event.metaKey === (tokens.includes('win') || tokens.includes('super') || tokens.includes('meta'))
    && (eventKey === key || eventKey === key.replace(/^arrow/, ''));
}
