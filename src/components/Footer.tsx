// ** import lib
import { CornerDownLeft, PanelLeftOpen, Trash2, X } from 'lucide-react';

import { IconButton } from './IconButton';
import { useStore } from '../lib/store';
import { resolvedFilterShortcuts } from '../lib/filter-shortcuts';

/** A compact keyboard-hint strip shared by the quick and full windows. */
export function Footer() {
  const mode = useStore((s) => s.mode);
  const selectedId = useStore((s) => s.selectedId);
  const items = useStore((s) => s.items);
  const select = useStore((s) => s.select);
  const selectedIds = useStore((s) => s.selectedIds);
  const deleteSelected = useStore((s) => s.deleteSelected);
  const pasteOnEnter = useStore((s) => s.settings?.pasteOnEnter ?? true);
  const sidebarOpen = useStore((s) => s.sidebarOpen);
  const toggleSidebar = useStore((s) => s.toggleSidebar);
  const settings = useStore((s) => s.settings);
  const navigationShortcut = resolvedFilterShortcuts(settings?.filterShortcuts)[0];
  const hasSelection = items.some((item) => item.id === selectedId);
  const primaryVerb = pasteOnEnter ? 'Paste' : 'Copy';

  if (selectedIds.length > 1) {
    return (
      <footer className={`history-footer is-${mode} selection-footer`} aria-label="Selection actions">
        <span>{selectedIds.length} selected</span>
        <div className="footer-spacer" />
        <IconButton label="Clear selection" onClick={() => select(null)}>
          <X size={15} aria-hidden />
        </IconButton>
        <IconButton
          label={`Delete ${selectedIds.length} selected items`}
          tone="danger"
          onClick={() => void deleteSelected()}
        >
          <Trash2 size={15} aria-hidden />
        </IconButton>
      </footer>
    );
  }

  return (
    <footer className={`history-footer is-${mode}`} aria-label="Keyboard actions">
      {!sidebarOpen && (
        <button
          type="button"
          className="footer-menu-button"
          title={`Open navigation (${navigationShortcut})`}
          aria-label={`Open navigation, ${navigationShortcut}`}
          onClick={toggleSidebar}
        >
          <PanelLeftOpen size={14} aria-hidden />
        </button>
      )}
      <span className="footer-hint">
        <kbd aria-label="Up and down arrows">↑↓</kbd>
        <span>Navigate</span>
      </span>
      <span className="footer-hint footer-primary-action">
        <kbd aria-label="Enter"><CornerDownLeft size={12} aria-hidden /></kbd>
        <span>{hasSelection ? primaryVerb : 'Select'}</span>
      </span>
      {mode === 'quick' && (
        <span className="footer-hint">
          <kbd>Esc</kbd>
          <span>Close</span>
        </span>
      )}
    </footer>
  );
}
