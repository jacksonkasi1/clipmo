// ** import types
import type { CSSProperties } from 'react';

// ** import lib
import { useEffect, useRef, useState } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { AlertCircle, Clipboard, LoaderCircle, SearchX } from 'lucide-react';

import { useStore } from '../lib/store';
import { getShortcutLabel } from '../lib/platform';
import { ItemRow } from './ItemRow';
import { SelectionContextMenu } from './SelectionContextMenu';

/**
 * Row heights, in px, shared by the virtualizer and the stylesheet.
 *
 * The value is published to CSS as `--row-height` on the scroll container so
 * the measured height and the painted height cannot drift apart; `.item-row`
 * in app.css reads it instead of hard-coding a second number.
 */
export const ROW_HEIGHT = { quick: 32, full: 40 } as const;

export function ItemList() {
  const mode = useStore((s) => s.mode);
  const items = useStore((s) => s.items);
  const selectedId = useStore((s) => s.selectedId);
  const selectedIds = useStore((s) => s.selectedIds);
  const selectOnly = useStore((s) => s.selectOnly);
  const selectToggle = useStore((s) => s.selectToggle);
  const selectRange = useStore((s) => s.selectRange);
  const search = useStore((s) => s.search);
  const loading = useStore((s) => s.loading);
  const bootError = useStore((s) => s.bootError);
  const loadingMore = useStore((s) => s.loadingMore);
  const hasMore = useStore((s) => s.hasMore);
  const loadMore = useStore((s) => s.loadMore);
  const refresh = useStore((s) => s.refresh);
  const parentRef = useRef<HTMLDivElement>(null);
  const startupRecoveryStarted = useRef(false);
  // Tracks whether the list owns keyboard focus so the active row can show a
  // slightly stronger neutral fill. This replaces the old accent focus ring,
  // which drew a blue rectangle around the entire scrolling container.
  const [listFocused, setListFocused] = useState(false);
  const [startupRetrying, setStartupRetrying] = useState(false);
  const [startupRecovered, setStartupRecovered] = useState(false);
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number } | null>(null);

  const rowHeight = ROW_HEIGHT[mode];

  const virtualizer = useVirtualizer({
    count: items.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => rowHeight,
    overscan: 8,
  });

  useEffect(() => {
    const index = items.findIndex((item) => item.id === selectedId);
    if (index >= 0) virtualizer.scrollToIndex(index, { align: 'auto' });
  }, [items, selectedId, virtualizer]);

  useEffect(() => {
    const scrollElement = parentRef.current;
    if (!scrollElement || !hasMore) return;
    const loadNearEnd = () => {
      const remaining = scrollElement.scrollHeight
        - scrollElement.scrollTop
        - scrollElement.clientHeight;
      if (remaining <= 600) {
        void loadMore().catch((error: unknown) => {
          console.error('Failed to load more clipboard history', error);
        });
      }
    };
    scrollElement.addEventListener('scroll', loadNearEnd, { passive: true });
    loadNearEnd();
    return () => scrollElement.removeEventListener('scroll', loadNearEnd);
  }, [hasMore, items.length, loadMore, loadingMore]);

  useEffect(() => {
    if (
      mode !== 'quick'
      || !bootError
      || items.length > 0
      || startupRecoveryStarted.current
    ) {
      return;
    }

    startupRecoveryStarted.current = true;
    let cancelled = false;

    const recover = async () => {
      setStartupRetrying(true);
      for (let attempt = 0; attempt < 5; attempt += 1) {
        try {
          await refresh();
          if (!cancelled) {
            setStartupRecovered(true);
            setStartupRetrying(false);
          }
          return;
        } catch (error) {
          if (attempt === 4) {
            console.error('Quick clipboard startup recovery failed', error);
            break;
          }
          await new Promise<void>((resolve) => {
            window.setTimeout(resolve, 120 * (2 ** attempt));
          });
        }
      }
      if (!cancelled) setStartupRetrying(false);
    };

    void recover();
    return () => {
      cancelled = true;
    };
  }, [bootError, items.length, mode, refresh]);

  const selectedSet = new Set(selectedIds);
  const showStartupLoading = items.length === 0 && (loading || startupRetrying);
  const showStartupError = items.length === 0 && Boolean(bootError) && !startupRecovered;

  return (
    <div
      ref={parentRef}
      className={`item-list ${selectedIds.length > 1 ? 'is-multiselect' : ''}`}
      style={{ '--row-height': `${rowHeight}px` } as CSSProperties}
      role="listbox"
      tabIndex={0}
      aria-label="Clipboard entries"
      aria-multiselectable="true"
      aria-busy={loading || loadingMore || startupRetrying}
      aria-activedescendant={selectedId !== null ? `clip-item-${selectedId}` : undefined}
      onFocus={() => setListFocused(true)}
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget as Node | null)) {
          setListFocused(false);
        }
      }}
      onKeyDown={(event) => {
        // Ctrl+Space toggles the active row without a mouse, replacing the
        // per-row checkbox that used to provide keyboard multi-select.
        if (event.key === ' ' && (event.ctrlKey || event.metaKey) && selectedId !== null) {
          event.preventDefault();
          selectToggle(selectedId);
        }
      }}
      onContextMenu={(event) => {
        const row = (event.target as HTMLElement).closest<HTMLElement>('[data-clip-id]');
        if (!row) return;
        event.preventDefault();
        const id = Number(row.dataset.clipId);
        if (!selectedIds.includes(id)) selectOnly(id);
        setContextMenu({ x: event.clientX, y: event.clientY });
      }}
    >
      {showStartupLoading ? (
        <div className="empty-state is-loading" role="status">
          <span className="empty-state-icon"><LoaderCircle className="is-spinning" size={24} aria-hidden /></span>
          <strong>Loading clipboard history…</strong>
          <span>Search is ready while Clipmo connects to your history.</span>
        </div>
      ) : showStartupError ? (
        <div className="empty-state" role="status">
          <span className="empty-state-icon"><AlertCircle size={24} aria-hidden /></span>
          <strong>Clipboard history could not be loaded</strong>
          <span>Clipmo is still usable. Press F5 to try again.</span>
        </div>
      ) : items.length === 0 ? (
        <EmptyState search={search} />
      ) : (
        <div
          role="presentation"
          style={{
            height: `${virtualizer.getTotalSize()}px`,
            position: 'relative',
            width: '100%',
          }}
        >
          {virtualizer.getVirtualItems().map((row) => {
            const item = items[row.index];
            if (!item) return null;
            return (
              <div
                key={item.id}
                role="presentation"
                style={{
                  position: 'absolute',
                  top: 0,
                  left: 0,
                  width: '100%',
                  transform: `translateY(${row.start}px)`,
                }}
              >
                <ItemRow
                  item={item}
                  selected={item.id === selectedId}
                  multiSelected={selectedIds.length > 1 && selectedSet.has(item.id)}
                  focused={listFocused && item.id === selectedId}
                  mode={mode}
                  position={row.index + 1}
                  total={hasMore ? -1 : items.length}
                  onSelect={(event) => {
                    setContextMenu(null);
                    if (event.shiftKey) selectRange(item.id);
                    else if (event.ctrlKey || event.metaKey) selectToggle(item.id);
                    else selectOnly(item.id);
                    parentRef.current?.focus({ preventScroll: true });
                  }}
                />
              </div>
            );
          })}
        </div>
      )}
      {loadingMore && (
        <span className="sr-only" role="status" aria-live="polite">
          Loading more clipboard history
        </span>
      )}
      {contextMenu && <SelectionContextMenu {...contextMenu} onClose={() => setContextMenu(null)} />}
    </div>
  );
}

function EmptyState({ search }: { search: string }) {
  const setSearch = useStore((s) => s.setSearch);

  if (search) {
    return (
      <div className="empty-state">
        <span className="empty-state-icon"><SearchX size={24} aria-hidden /></span>
        <strong>No matches for “{search}”</strong>
        <span>Try another phrase or clear your search.</span>
        <button type="button" className="text-button" onClick={() => void setSearch('')}>
          Clear search
        </button>
      </div>
    );
  }

  return (
    <div className="empty-state">
      <span className="empty-state-icon"><Clipboard size={25} aria-hidden /></span>
      <strong>Your clipboard history is empty</strong>
      <span>Copy text, images, or files and they’ll appear here instantly.</span>
      <kbd>{getShortcutLabel('open')}</kbd>
    </div>
  );
}
