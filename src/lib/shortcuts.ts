// ** import types
import type { ShortcutAction } from './platform';

// ** import utils
import { getShortcutKeys } from './platform';

export interface ShortcutDefinition {
  id: string;
  label: string;
  description: string;
  action?: ShortcutAction;
  keys?: string[];
}

export const APP_SHORTCUTS: ShortcutDefinition[] = [
  { id: 'navigate', label: 'Navigate history', description: 'Select the previous or next item.', keys: ['↑', '↓'] },
  { id: 'paste', label: 'Paste selected item', description: 'Paste into the previously active app.', action: 'paste' },
  { id: 'copy', label: 'Copy selected item', description: 'Place the item back on the clipboard.', action: 'copy' },
  { id: 'edit', label: 'Edit selected item', description: 'Edit text, links, colors, and email addresses.', action: 'edit' },
  { id: 'favorite', label: 'Toggle favorite', description: 'Pin or unpin the selected item.', action: 'favorite' },
  { id: 'delete', label: 'Delete selected item', description: 'Remove one history item.', action: 'deleteItem' },
  { id: 'search', label: 'Focus search', description: 'Search all captured text and metadata.', action: 'search' },
  { id: 'commands', label: 'Show commands', description: 'Open the shortcut and action palette.', action: 'commands' },
  { id: 'settings', label: 'Open settings', description: 'Open Clipmo settings.', action: 'settings' },
  { id: 'preview', label: 'Toggle preview', description: 'Show or hide the preview pane.', action: 'preview' },
  { id: 'clear', label: 'Clear non-favorites', description: 'Clear history while keeping favorites.', action: 'clearHistory' },
  { id: 'all-devices', label: 'Show all devices', description: 'Clear the source-device filter.', keys: ['Ctrl', '0'] },
  { id: 'switch-device', label: 'Switch source device', description: 'Select one of the first nine source devices.', keys: ['Ctrl', '1–9'] },
  { id: 'hide', label: 'Hide Clipmo', description: 'Close the popup without quitting.', keys: ['Esc'] },
];

export function shortcutKeys(shortcut: ShortcutDefinition): string[] {
  return shortcut.action ? getShortcutKeys(shortcut.action) : (shortcut.keys ?? []);
}
