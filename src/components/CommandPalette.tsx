// ** import types
import type { LucideIcon } from 'lucide-react';

// ** import lib
import { useEffect, useRef } from 'react';
import {
  ArrowDownUp,
  ClipboardCopy,
  Copy,
  Eraser,
  Keyboard,
  PanelRight,
  Pencil,
  Search,
  Settings2,
  Star,
  Trash2,
  X,
} from 'lucide-react';

import { IconButton } from './IconButton';
import { APP_SHORTCUTS, shortcutKeys } from '../lib/shortcuts';
import { getPlatform } from '../lib/platform';
import { useStore } from '../lib/store';
import { api, type ClearScope } from '../lib/tauri';

const ICONS: Record<string, LucideIcon> = {
  navigate: ArrowDownUp,
  paste: ClipboardCopy,
  copy: Copy,
  edit: Pencil,
  favorite: Star,
  delete: Trash2,
  search: Search,
  commands: Keyboard,
  settings: Settings2,
  preview: PanelRight,
  clear: Eraser,
  hide: X,
};

const FOCUSABLE_SELECTOR = [
  'button:not(:disabled)',
  '[href]',
  'input:not(:disabled)',
  'select:not(:disabled)',
  'textarea:not(:disabled)',
  '[tabindex]:not([tabindex="-1"])',
].join(',');

export function CommandPalette() {
  const visible = useStore((state) => state.showCommands);
  const setVisible = useStore((state) => state.setShowCommands);
  const selectedId = useStore((state) => state.selectedId);
  const items = useStore((state) => state.items);
  const toggleFavorite = useStore((state) => state.toggleFavorite);
  const deleteItem = useStore((state) => state.deleteItem);
  const clearHistory = useStore((state) => state.clearHistory);
  const showPreview = useStore((state) => state.showPreview);
  const setShowPreview = useStore((state) => state.setShowPreview);
  const selected = items.find((item) => item.id === selectedId);
  const paletteRef = useRef<HTMLElement>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (!visible) return;
    previousFocusRef.current = document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;
    const frame = window.requestAnimationFrame(() => {
      paletteRef.current?.querySelector<HTMLButtonElement>('.command-row:not(:disabled)')?.focus();
    });
    return () => {
      window.cancelAnimationFrame(frame);
      previousFocusRef.current?.focus();
    };
  }, [visible]);

  if (!visible) return null;

  const close = () => setVisible(false);
  const run = async (id: string) => {
    switch (id) {
      case 'paste':
        if (selectedId) await api.pasteActive(selectedId, 'original');
        break;
      case 'copy':
        if (selectedId) await api.copyToClipboard(selectedId, 'original');
        break;
      case 'edit':
        if (!showPreview) {
          void setShowPreview(true);
          window.setTimeout(() => window.dispatchEvent(new CustomEvent('clipmo:edit-selected')), 0);
        } else {
          window.dispatchEvent(new CustomEvent('clipmo:edit-selected'));
        }
        break;
      case 'favorite':
        if (selectedId) await toggleFavorite(selectedId);
        break;
      case 'delete':
        if (selectedId) await deleteItem(selectedId);
        break;
      case 'search':
        window.dispatchEvent(new CustomEvent('clipmo:focus-search'));
        break;
      case 'settings':
        await api.openSettingsWindow();
        break;
      case 'preview':
        void setShowPreview(!showPreview);
        break;
      case 'clear': {
        let approved: ClearScope | null;
        try {
          approved = await api.confirmClear(
            'Clear all non-favorite history items? Favorites will stay pinned.',
            'Clear history',
          );
        } catch (error) {
          console.error('Failed to confirm clearing clipboard history', error);
          return;
        }
        if (approved) await clearHistory(false, approved);
        break;
      }
      case 'hide':
        await api.hideWindow();
        break;
      default:
        break;
    }
    close();
  };

  return (
    <div className="command-backdrop" role="presentation" onMouseDown={close}>
      <section
        ref={paletteRef}
        className="command-palette"
        role="dialog"
        aria-modal="true"
        aria-labelledby="command-title"
        aria-describedby="command-description"
        onMouseDown={(event) => event.stopPropagation()}
        onKeyDown={(event) => {
          if (event.key === 'Tab') {
            const focusable = Array.from(
              event.currentTarget.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR),
            ).filter((element) => !element.hasAttribute('hidden'));
            if (focusable.length === 0) {
              event.preventDefault();
              return;
            }

            const index = focusable.indexOf(document.activeElement as HTMLElement);
            const nextIndex = event.shiftKey
              ? (index <= 0 ? focusable.length - 1 : index - 1)
              : (index < 0 || index === focusable.length - 1 ? 0 : index + 1);
            event.preventDefault();
            focusable[nextIndex]?.focus();
            return;
          }

          if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return;
          const buttons = Array.from(
            event.currentTarget.querySelectorAll<HTMLButtonElement>('.command-row:not(:disabled)'),
          );
          if (buttons.length === 0) return;
          event.preventDefault();
          const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
          if (event.key === 'Home') buttons[0]?.focus();
          else if (event.key === 'End') buttons.at(-1)?.focus();
          else {
            const delta = event.key === 'ArrowDown' ? 1 : -1;
            const nextIndex = index < 0
              ? (delta > 0 ? 0 : buttons.length - 1)
              : (index + delta + buttons.length) % buttons.length;
            buttons[nextIndex]?.focus();
          }
        }}
      >
        <header>
          <span className="command-heading-icon"><Keyboard size={20} aria-hidden /></span>
          <div>
            <h2 id="command-title">Commands and shortcuts</h2>
            <p id="command-description">
              {getPlatform() === 'macos' ? 'macOS' : 'Windows'} shortcuts are shown for this build.
            </p>
          </div>
          <IconButton label="Close commands" onClick={close}>
            <X size={18} aria-hidden />
          </IconButton>
        </header>
        <div className="command-list">
          {APP_SHORTCUTS.map((shortcut) => {
            const Glyph = ICONS[shortcut.id] ?? Keyboard;
            const disabled = ['paste', 'copy', 'favorite', 'delete'].includes(shortcut.id) && !selected;
            const editDisabled = shortcut.id === 'edit' && (!selected || ['image', 'files'].includes(selected.kind));
            return (
              <button
                type="button"
                className="command-row"
                key={shortcut.id}
                disabled={disabled || editDisabled || shortcut.id === 'navigate' || shortcut.id === 'commands'}
                onClick={() => {
                  void run(shortcut.id).catch((error: unknown) => {
                    console.error(`Failed to run ${shortcut.id} command`, error);
                  });
                }}
              >
                <span className="command-icon"><Glyph size={17} aria-hidden /></span>
                <span className="command-copy">
                  <strong>{shortcut.label}</strong>
                </span>
                <span className="shortcut-keys" aria-label={shortcutKeys(shortcut).join(' plus ')}>
                  {shortcutKeys(shortcut).map((key) => <kbd key={key}>{key}</kbd>)}
                </span>
              </button>
            );
          })}
        </div>
      </section>
    </div>
  );
}
