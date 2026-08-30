// ** import types
import type { CSSProperties, KeyboardEvent } from 'react';

// ** import lib
import { ArrowLeft, ChevronRight, Folder } from 'lucide-react';
import { useEffect, useRef } from 'react';

import { useStore } from '../lib/store';
import { tagColorKey } from '../lib/tag-color';

interface QuickCollectionsViewProps {
  onBack: () => void;
}

/** A compact folder picker; choosing a folder keeps the user in Quick View. */
export function QuickCollectionsView({ onBack }: QuickCollectionsViewProps) {
  const collections = useStore((state) => state.collections);
  const activeTag = useStore((state) => state.activeTag);
  const tagColors = useStore((state) => state.tagColors);
  const setTag = useStore((state) => state.setTag);
  const backRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    backRef.current?.focus();
    const onKeyDown = (event: globalThis.KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      event.preventDefault();
      onBack();
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [onBack]);

  const navigateFolders = (event: KeyboardEvent<HTMLDivElement>) => {
    if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) return;
    const buttons = Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>('button'));
    if (!buttons.length) return;
    event.preventDefault();
    const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
    const next = event.key === 'Home' ? 0 : event.key === 'End' ? buttons.length - 1
      : Math.max(0, Math.min(buttons.length - 1, index + (event.key === 'ArrowDown' ? 1 : -1)));
    buttons[next]?.focus();
  };

  return (
    <section className="quick-collections" aria-label="Quick collections">
      <header className="quick-collections-header">
        <button ref={backRef} type="button" className="collection-header-button" onClick={onBack} aria-label="Back to history" title="Back to history (Esc)">
          <ArrowLeft size={17} aria-hidden />
        </button>
        <h1>Collections</h1>
        <span>{collections.length}</span>
      </header>
      <div className="quick-collections-list" onKeyDown={navigateFolders}>
        {collections.map((collection, index) => {
          const color = tagColors[tagColorKey(collection.name)];
          return (
            <button
              key={collection.name}
              type="button"
              className={`quick-collection-row collection-tone-${index % 6}`}
              style={color ? { '--folder-color': color } as CSSProperties : undefined}
              aria-pressed={activeTag === collection.name}
              aria-label={`${collection.name} ${collection.itemCount} ${collection.itemCount === 1 ? 'item' : 'items'}`}
              onClick={() => { void setTag(collection.name); onBack(); }}
            >
              <Folder size={24} className="quick-collection-icon" aria-hidden />
              <span><strong>{collection.name}</strong><small>{collection.itemCount} {collection.itemCount === 1 ? 'item' : 'items'}</small></span>
              <ChevronRight size={16} aria-hidden />
            </button>
          );
        })}
        {!collections.length && <p className="quick-collections-empty">No collections yet. Create a collection in the main Clipmo window.</p>}
      </div>
      <footer className="quick-collections-footer">Choose a folder to view its clips <span>Esc to go back</span></footer>
    </section>
  );
}
