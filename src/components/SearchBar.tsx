// ** import types
import type { HeaderAction } from '../lib/header-actions';
import type { MouseEvent } from 'react';

// ** import lib
import { useEffect, useRef, useState } from 'react';
import {
  Command,
  PanelRightClose,
  PanelRightOpen,
  Pin,
  Plus,
  Search,
  Settings2,
  SquareTerminal,
  X,
} from 'lucide-react';

import { IconButton } from './IconButton';
import { isSearchActive, visibleHeaderActions } from '../lib/header-actions';
import { useStore } from '../lib/store';
import { api } from '../lib/tauri';
import { toast } from '../lib/toast';
import { getPlatform, getShortcutLabel } from '../lib/platform';

export function SearchBar({ onAddDevice }: { onAddDevice?: () => void }) {
  const mode = useStore((s) => s.mode);
  const search = useStore((s) => s.search);
  const setSearch = useStore((s) => s.setSearch);
  const refresh = useStore((s) => s.refresh);
  const visibleCount = useStore((s) => s.items.length);
  const hasMore = useStore((s) => s.hasMore);
  const showPreview = useStore((s) => s.showPreview);
  const showCommands = useStore((s) => s.showCommands);
  const setShowPreview = useStore((s) => s.setShowPreview);
  const setShowCommands = useStore((s) => s.setShowCommands);
  const [pinned, setPinned] = useState(false);
  const [searchFocused, setSearchFocused] = useState(false);
  const ref = useRef<HTMLInputElement>(null);

  const context = { mode, searchFocused, hasSearchText: search.length > 0 };
  const actions = visibleHeaderActions(context);

  useEffect(() => {
    ref.current?.focus();
  }, []);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const target = e.target instanceof HTMLElement ? e.target : null;
      if (e.defaultPrevented || target?.matches('textarea, [contenteditable="true"]')) return;
      if (e.key === 'Escape') {
        if (showCommands) return;
        if (mode === 'quick') {
          e.preventDefault();
          void api.hideWindow();
          return;
        }
        if (search) {
          e.preventDefault();
          void setSearch('');
        } else if (document.activeElement === ref.current) {
          e.preventDefault();
          ref.current?.blur();
        }
      }
      if (e.key === 'F5') {
        e.preventDefault();
        void refresh();
      }
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'f') {
        e.preventDefault();
        ref.current?.focus();
        ref.current?.select();
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [mode, refresh, search, setSearch, showCommands]);

  useEffect(() => {
    const focusSearch = () => {
      ref.current?.focus();
      ref.current?.select();
      if (mode === 'quick') {
        window.setTimeout(() => {
          if (document.activeElement === ref.current) {
            void api.signalQuickSearchFocused().catch((error: unknown) => {
              console.error('Failed to confirm quick search focus', error);
            });
          }
        }, 0);
      }
    };
    window.addEventListener('clipmo:focus-search', focusSearch);
    if (mode === 'quick') window.addEventListener('focus', focusSearch);
    return () => {
      window.removeEventListener('clipmo:focus-search', focusSearch);
      window.removeEventListener('focus', focusSearch);
    };
  }, [mode]);

  const focusFromHeader = (event: MouseEvent<HTMLElement>) => {
    if ((event.target as HTMLElement).closest('button')) return;
    event.preventDefault();
    ref.current?.focus();
  };

  const renderAction = (action: HeaderAction) => {
    switch (action) {
      case 'clearSearch':
        return (
          <IconButton
            key={action}
            label="Clear search"
            className="search-clear-button"
            onClick={() => {
              void setSearch('');
              ref.current?.focus();
            }}
          >
            <X size={15} aria-hidden />
          </IconButton>
        );
      case 'preview':
        return (
          <IconButton
            key={action}
            label={showPreview ? 'Hide preview pane' : 'Show preview pane'}
            active={showPreview}
            onClick={() => void setShowPreview(!showPreview)}
          >
            {showPreview ? (
              <PanelRightClose size={16} aria-hidden />
            ) : (
              <PanelRightOpen size={16} aria-hidden />
            )}
          </IconButton>
        );
      case 'pin':
        return (
          <IconButton
            key={action}
            label={pinned ? 'Unpin window' : 'Keep window on top'}
            active={pinned}
            onClick={() => {
              const next = !pinned;
              setPinned(next);
              void api.setAlwaysOnTop(next).catch((error: unknown) => {
                setPinned(!next);
                toast(`The pin state could not be changed: ${String(error)}`, 'error');
              });
            }}
          >
            <Pin size={16} aria-hidden />
          </IconButton>
        );
      case 'commands':
        return (
          <IconButton
            key={action}
            label={`Commands (${getShortcutLabel('commands')})`}
            onClick={() => setShowCommands(true)}
          >
            {getPlatform() === 'macos' ? (
              <Command size={16} aria-hidden />
            ) : (
              <SquareTerminal size={16} aria-hidden />
            )}
          </IconButton>
        );
      case 'settings':
        return (
          <IconButton
            key={action}
            className="search-settings-button"
            label={`Settings (${getShortcutLabel('settings')})`}
            onClick={() => void api.openSettingsWindow().catch((error: unknown) => {
              toast(`Settings could not be opened: ${String(error)}`, 'error');
            })}
          >
            <Settings2 size={16} aria-hidden />
          </IconButton>
        );
    }
  };

  return (
    <header
      className={`search-header ${isSearchActive(context) ? 'is-search-active' : ''}`}
      onMouseDown={focusFromHeader}
    >
      <Search className="search-glyph" size={15} strokeWidth={1.9} aria-hidden />
      <input
        ref={ref}
        type="search"
        placeholder={mode === 'quick' ? 'Search clipboard…' : 'Search content, tags, or application…'}
        value={search}
        onChange={(e) => void setSearch(e.target.value)}
        onFocus={() => setSearchFocused(true)}
        onBlur={() => setSearchFocused(false)}
        aria-label="Search clipboard history"
        aria-describedby="search-results-status"
        autoComplete="off"
        spellCheck={false}
      />
      <span id="search-results-status" className="sr-only" aria-live="polite">
        {search
          ? `${hasMore ? 'At least ' : ''}${visibleCount} search ${visibleCount === 1 ? 'result' : 'results'}`
          : `${hasMore ? 'At least ' : ''}${visibleCount} clipboard ${visibleCount === 1 ? 'item' : 'items'} visible`}
      </span>
      {mode === 'full' && onAddDevice && (
        <IconButton label="Pair another device" onClick={onAddDevice}>
          <Plus size={17} aria-hidden />
        </IconButton>
      )}
      {actions.map(renderAction)}
    </header>
  );
}
