export type PlatformKind = 'windows' | 'macos' | 'linux';

export type ShortcutAction =
  | 'clearHistory'
  | 'commands'
  | 'copy'
  | 'deleteItem'
  | 'edit'
  | 'favorite'
  | 'open'
  | 'paste'
  | 'search'
  | 'settings'
  | 'preview';

const WINDOWS_SHORTCUTS: Record<ShortcutAction, string[]> = {
  clearHistory: ['Ctrl', 'Shift', 'Delete'],
  commands: ['Ctrl', 'K'],
  copy: ['Ctrl', 'C'],
  deleteItem: ['Delete'],
  edit: ['Ctrl', 'E'],
  favorite: ['Ctrl', 'D'],
  open: ['Ctrl', 'Shift', 'V'],
  paste: ['Enter'],
  search: ['Ctrl', 'F'],
  settings: ['Ctrl', ','],
  preview: ['Ctrl', 'Shift', 'P'],
};

const MACOS_SHORTCUTS: Record<ShortcutAction, string[]> = {
  clearHistory: ['⇧', '⌘', '⌫'],
  commands: ['⌘', 'K'],
  copy: ['⌘', 'C'],
  deleteItem: ['⌘', '⌫'],
  edit: ['⌘', 'E'],
  favorite: ['⌘', 'D'],
  open: ['⇧', '⌘', 'V'],
  paste: ['↵'],
  search: ['⌘', 'F'],
  settings: ['⌘', ','],
  preview: ['⇧', '⌘', 'P'],
};

export const getPlatform = (): PlatformKind => {
  if (typeof navigator === 'undefined') return 'windows';
  const value = `${navigator.platform} ${navigator.userAgent}`.toLowerCase();
  if (value.includes('mac')) return 'macos';
  if (value.includes('linux')) return 'linux';
  return 'windows';
};

export const getShortcutKeys = (action: ShortcutAction): string[] => {
  return getPlatform() === 'macos' ? MACOS_SHORTCUTS[action] : WINDOWS_SHORTCUTS[action];
};

export const getShortcutLabel = (action: ShortcutAction): string => {
  const keys = getShortcutKeys(action);
  return getPlatform() === 'macos' ? keys.join('') : keys.join('+');
};

export const isDeleteShortcutKey = (event: Pick<KeyboardEvent, 'key' | 'metaKey'>): boolean =>
  event.key === 'Delete' || (getPlatform() === 'macos' && event.metaKey && event.key === 'Backspace');
