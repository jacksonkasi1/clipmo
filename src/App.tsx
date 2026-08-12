// ** import types
import type { Backdrop } from './lib/types';

// ** import lib
import { useEffect, useRef, useState } from 'react';

import { CommandPalette } from './components/CommandPalette';
import { ClipboardSidebar } from './components/ClipboardSidebar';
import { DetailsTable } from './components/DetailsTable';
import { Footer } from './components/Footer';
import { ItemList } from './components/ItemList';
import { PairDeviceDialog } from './components/PairDeviceDialog';
import { PreviewPane } from './components/PreviewPane';
import { SearchBar } from './components/SearchBar';
import { SidebarExplorerDialog } from './components/SidebarExplorerDialog';
import { getListKeyboardAction } from './lib/list-navigation';
import { FILTER_SHORTCUTS, matchesShortcut, resolvedFilterShortcuts } from './lib/filter-shortcuts';
import { useStore } from './lib/store';
import { api, on } from './lib/tauri';
import { applyTheme } from './lib/theme';
import { ToastSurface } from './lib/toast';

export default function App() {
  const mode = useStore((s) => s.mode);
  const appearance = useStore((s) => s.appearance);
  const settings = useStore((s) => s.settings);
  const bootstrapped = useStore((s) => s.bootstrapped);
  const showPreview = useStore((s) => s.showPreview);
  const showDetails = useStore((s) => s.showDetails);
  const showCommands = useStore((s) => s.showCommands);
  const setShowCommands = useStore((s) => s.setShowCommands);
  const setShowPreview = useStore((s) => s.setShowPreview);
  const selectedId = useStore((s) => s.selectedId);
  const selectedIds = useStore((s) => s.selectedIds);
  const items = useStore((s) => s.items);
  const select = useStore((s) => s.select);
  const selectOnly = useStore((s) => s.selectOnly);
  const selectToggle = useStore((s) => s.selectToggle);
  const selectRange = useStore((s) => s.selectRange);
  const selectAll = useStore((s) => s.selectAll);
  const toggleFavorite = useStore((s) => s.toggleFavorite);
  const deleteItem = useStore((s) => s.deleteItem);
  const deleteSelected = useStore((s) => s.deleteSelected);
  const clearHistory = useStore((s) => s.clearHistory);
  const devices = useStore((s) => s.devices);
  const sidebarOpen = useStore((s) => s.sidebarOpen);
  const toggleSidebar = useStore((s) => s.toggleSidebar);
  const setCategory = useStore((s) => s.setCategory);
  const showFavorites = useStore((s) => s.showFavorites);
  const setDevice = useStore((s) => s.setDevice);
  const readinessSignaled = useRef(false);
  const [quickEntering, setQuickEntering] = useState(false);
  const [pairDeviceOpen, setPairDeviceOpen] = useState(false);
  const [filterExplorerOpen, setFilterExplorerOpen] = useState(false);

  useEffect(() => {
    document.documentElement.dataset.mode = mode;
    document.title = mode === 'quick' ? 'Clipmo quick clipboard' : 'Clipmo';
  }, [mode]);

  // Native event names remain stable for in-place upgrades; all visible copy
  // and browser-only events use the Clipmo name.
  useEffect(() => {
    if (mode !== 'quick') return;
    let fallback: number | undefined;
    const replayOpen = () => {
      window.clearTimeout(fallback);
      setQuickEntering(false);
      window.setTimeout(() => setQuickEntering(true), 0);
      fallback = window.setTimeout(() => setQuickEntering(false), 180);
      // The native reveal already waits for both `frontend_ready` and the
      // first SQLite read, so by the time this listener fires the store is
      // already hydrated. We still re-fetch on every open because the user
      // may have copied items while the palette was hidden, and the
      // visibility-resync listener (in `bootStore`) covers the same case
      // for a window that was merely occluded rather than closed. Both
      // paths share `requestResync`, so a near-simultaneous open + visible
      // event only triggers one SQLite read.
      void useStore.getState().requestResync('open').catch((error: unknown) => {
        console.error('Failed to resync quick clipboard on open', error);
      });
      window.dispatchEvent(new CustomEvent('clipmo:focus-search'));
    };
    const unlisten = on<void>('clipdeck:quick-opened', replayOpen);
    return () => {
      window.clearTimeout(fallback);
      void unlisten.then((fn) => fn());
    };
  }, [mode]);

  useEffect(() => {
    applyTheme(settings?.theme ?? 'system', appearance);
    document.documentElement.dataset.backdrop = settings?.backdrop ?? 'acrylic';
  }, [settings?.theme, settings?.backdrop, appearance]);

  useEffect(() => {
    const unlisten = on<Backdrop>('clipdeck:backdrop', (effective) => {
      document.documentElement.dataset.backdrop = effective.toLowerCase();
    });

    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (event.defaultPrevented) return;
      const modifier = event.ctrlKey || event.metaKey;
      const key = event.key.toLowerCase();
      const target = event.target as HTMLElement | null;
      const editing =
        target?.matches('input, textarea, select, [contenteditable="true"]') ?? false;

      if (!editing && modifier && /^\d$/.test(event.key)) {
        const deviceIndex = Number(event.key) - 1;
        const device = devices[deviceIndex];
        if (event.key === '0' || device) {
          event.preventDefault();
          void setDevice(event.key === '0' ? null : device?.id ?? null);
          return;
        }
      }
      const filterShortcutIndex = editing
        ? -1
        : resolvedFilterShortcuts(settings?.filterShortcuts)
            .findIndex((shortcut) => matchesShortcut(event, shortcut));
      if (filterShortcutIndex >= 0) {
        event.preventDefault();
        const target = FILTER_SHORTCUTS[filterShortcutIndex]?.target;
        if (target === 'navigation') {
          toggleSidebar();
        } else if (target === 'favorites') {
          void showFavorites();
        } else if (target === 'all') {
          void setCategory(null);
        } else if (target) {
          void setCategory(target);
        }
        return;
      }

      if (showCommands && event.key === 'Escape') {
        event.preventDefault();
        setShowCommands(false);
        return;
      }
      if (modifier && key === 'k') {
        event.preventDefault();
        setShowCommands(!showCommands);
        return;
      }

      const selectedIndex = items.findIndex((item) => item.id === selectedId);
      const searchHasFocus = target?.matches('input[type="search"]') ?? false;
      const listAction = (!editing || searchHasFocus)
        ? getListKeyboardAction(
            event.key,
            selectedIndex,
            items.length,
            settings?.pasteOnEnter ?? true,
          )
        : null;
      if (listAction) {
        event.preventDefault();
        if (listAction.type === 'select') {
          const next = items[listAction.index];
          if (next) {
            if (event.shiftKey) selectRange(next.id);
            else if (modifier) selectToggle(next.id);
            else selectOnly(next.id);
          }
        } else if (selectedId !== null) {
          if (listAction.type === 'paste') {
            void api.pasteActive(selectedId, 'original');
          } else {
            void api.copyToClipboard(selectedId, 'original');
          }
        }
        return;
      }
      if (editing) return;

      if (modifier && key === 'a') {
        event.preventDefault();
        selectAll();
        return;
      }
      if (modifier && key === 'c' && selectedId !== null && !window.getSelection()?.toString()) {
        event.preventDefault();
        // A clipboard can contain one logical payload. Ctrl+C therefore
        // copies the focused row even when a range is selected.
        void api.copyToClipboard(selectedId, 'original');
      } else if (modifier && key === 'e' && selectedId) {
        event.preventDefault();
        if (!showPreview) {
          void setShowPreview(true);
          window.setTimeout(() => window.dispatchEvent(new CustomEvent('clipmo:edit-selected')), 0);
        } else {
          window.dispatchEvent(new CustomEvent('clipmo:edit-selected'));
        }
      } else if (modifier && key === 'd' && selectedId) {
        event.preventDefault();
        const target = selectedIds.length > 1 ? selectedIds : [selectedId];
        for (const id of target) void toggleFavorite(id);
      } else if (event.key === 'Delete' && !(modifier && event.shiftKey)) {
        event.preventDefault();
        if (selectedIds.length > 1) {
          void deleteSelected();
        } else if (selectedId !== null) {
          void deleteItem(selectedId);
        }
      } else if (event.key === 'Escape' && selectedIds.length > 0) {
        event.preventDefault();
        select(null);
      } else if (modifier && key === ',') {
        event.preventDefault();
        void api.openSettingsWindow().catch((error: unknown) => {
          console.error('Settings could not be opened', error);
        });
      } else if (modifier && event.shiftKey && key === 'p') {
        event.preventDefault();
        void setShowPreview(!showPreview);
      } else if (modifier && event.shiftKey && event.key === 'Delete') {
        event.preventDefault();
        void api.confirm(
          'Clear all non-favorite history items? Favorites will stay pinned.',
          'Clear history',
        ).then((approved) => {
          if (approved) return clearHistory(false);
        }).catch((error: unknown) => {
          console.error('Failed to confirm clearing clipboard history', error);
        });
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [
    clearHistory,
    deleteItem,
    deleteSelected,
    items,
    select,
    selectAll,
    selectOnly,
    selectRange,
    selectToggle,
    selectedId,
    selectedIds,
    setShowCommands,
    setShowPreview,
    settings?.pasteOnEnter,
    showCommands,
    showPreview,
    toggleFavorite,
    devices,
    setCategory,
    setDevice,
    showFavorites,
    toggleSidebar,
  ]);

  useEffect(() => {
    if (!bootstrapped || readinessSignaled.current) return;

    let cancelled = false;
    let retry: number | undefined;
    const deadline = Date.now() + 30_000;

    function scheduleRetry(delay: number) {
      if (cancelled || readinessSignaled.current || Date.now() >= deadline) return;
      retry = window.setTimeout(signalWhenReady, delay);
    }

    function signalWhenReady() {
      if (cancelled || readinessSignaled.current) return;
      const search = document.querySelector<HTMLInputElement>('.search-header input[type="search"]');
      const layout = document.querySelector<HTMLElement>('.history-pane');
      const searchVisible = Boolean(search && search.getBoundingClientRect().height > 0);
      const layoutVisible = Boolean(layout && layout.getBoundingClientRect().height > 0);
      if (!searchVisible || !layoutVisible) {
        scheduleRetry(100);
        return;
      }
      // Quick View refuses to reveal until both the layout AND the first
      // SQLite read have landed. The full window has no such constraint, so
      // we only block on hydration when the current webview is the Quick
      // palette.
      const hydrated = useStore.getState().hydrated;
      if (mode === 'quick' && !hydrated) {
        scheduleRetry(80);
        return;
      }

      void api.signalFrontendReady(searchVisible, layoutVisible).then(() => {
        if (!cancelled) readinessSignaled.current = true;
      }).catch((error: unknown) => {
        if (cancelled) return;
        console.error(`Failed to signal ${mode} frontend readiness`, error);
        scheduleRetry(250);
      });
    }

    signalWhenReady();
    return () => {
      cancelled = true;
      window.clearTimeout(retry);
    };
  }, [bootstrapped, mode]);

  const frameClasses = [
    'app-frame',
    `is-${mode}`,
    showPreview ? '' : 'preview-is-hidden',
  ].filter(Boolean).join(' ');

  const clipboardLayout = (
    <>
      <aside
        className={`history-pane ${sidebarOpen ? 'sidebar-is-open' : ''}`}
        aria-label="Clipboard history"
      >
        <ClipboardSidebar
          onAddDevice={mode === 'full' ? () => setPairDeviceOpen(true) : undefined}
          onExploreFilters={mode === 'full' ? () => setFilterExplorerOpen(true) : undefined}
        />
        <div className="history-content">
          <SearchBar onAddDevice={mode === 'full' ? () => setPairDeviceOpen(true) : undefined} />
          <ItemList />
          <Footer />
        </div>
      </aside>
      {showPreview && (
        <main className="content-pane">
          <PreviewPane />
          {mode === 'full' && showDetails && <DetailsTable />}
        </main>
      )}
    </>
  );

  return (
    <div
      className={frameClasses}
      role="application"
      aria-label={mode === 'quick' ? 'Clipmo quick clipboard' : 'Clipmo clipboard history'}
    >
      {mode === 'quick' ? (
        <div
          className={`quick-content ${quickEntering ? 'quick-entering' : ''}`}
          onAnimationEnd={() => setQuickEntering(false)}
        >
          {clipboardLayout}
        </div>
      ) : clipboardLayout}
      {mode === 'full' && <CommandPalette />}
      {mode === 'full' && (
        <PairDeviceDialog open={pairDeviceOpen} onClose={() => setPairDeviceOpen(false)} />
      )}
      {mode === 'full' && (
        <SidebarExplorerDialog open={filterExplorerOpen} onClose={() => setFilterExplorerOpen(false)} />
      )}
      <ToastSurface />
    </div>
  );
}
