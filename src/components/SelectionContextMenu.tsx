// ** import lib
import { FolderPlus, FolderX, Star, StarOff, Trash2 } from 'lucide-react';
import { useEffect, useMemo, useRef } from 'react';
import { createPortal } from 'react-dom';

import { useStore } from '../lib/store';

interface Props {
  x: number;
  y: number;
  onClose: () => void;
}

/** Compact actions for either one clipboard item or the current multi-selection. */
export function SelectionContextMenu({ x, y, onClose }: Props) {
  const collections = useStore((state) => state.collections);
  const selectedIds = useStore((state) => state.selectedIds);
  const items = useStore((state) => state.items);
  const addSelectedToCollection = useStore((state) => state.addSelectedToCollection);
  const removeSelectedFromCollection = useStore((state) => state.removeSelectedFromCollection);
  const setSelectedFavorites = useStore((state) => state.setSelectedFavorites);
  const deleteSelected = useStore((state) => state.deleteSelected);
  const selected = items.filter((item) => selectedIds.includes(item.id));
  const allStarred = selected.length > 0 && selected.every((item) => item.favorite);
  const isMultiple = selected.length > 1;
  const menuRef = useRef<HTMLDivElement>(null);
  const relevantCollections = useMemo(
    () => collections.filter((collection) => selected.some((item) => item.tags.includes(collection.name))),
    [collections, selected],
  );

  useEffect(() => {
    const close = () => onClose();
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') close();
    };
    window.addEventListener('pointerdown', close);
    window.addEventListener('blur', close);
    window.addEventListener('resize', close);
    window.addEventListener('scroll', close, true);
    window.addEventListener('keydown', handleKeyDown);
    menuRef.current?.querySelector<HTMLElement>('button, summary')?.focus();
    return () => {
      window.removeEventListener('pointerdown', close);
      window.removeEventListener('blur', close);
      window.removeEventListener('resize', close);
      window.removeEventListener('scroll', close, true);
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [onClose]);

  const run = (action: () => Promise<void>) => {
    onClose();
    void action();
  };

  const menuX = Math.max(8, Math.min(x, window.innerWidth - 238));
  const menuY = Math.max(8, Math.min(y, window.innerHeight - 300));

  return createPortal(
    <div
      ref={menuRef}
      className="selection-context-menu"
      role="menu"
      aria-label={isMultiple ? `Actions for ${selected.length} selected items` : 'Item actions'}
      style={{ left: menuX, top: menuY }}
      onPointerDown={(event) => event.stopPropagation()}
    >
      {isMultiple && <div className="selection-context-title">{selected.length} selected</div>}
      <details>
        <summary><FolderPlus size={15} aria-hidden /> Add to collection</summary>
        <div className="selection-context-submenu">
          {collections.length ? collections.map((collection) => (
            <button key={collection.name} type="button" role="menuitem" onClick={() => run(() => addSelectedToCollection(collection.name))}>
              {collection.name}
            </button>
          )) : <span>No collections yet</span>}
        </div>
      </details>
      <details>
        <summary><FolderX size={15} aria-hidden /> Remove from collection</summary>
        <div className="selection-context-submenu">
          {relevantCollections.length ? relevantCollections.map((collection) => (
            <button key={collection.name} type="button" role="menuitem" onClick={() => run(() => removeSelectedFromCollection(collection.name))}>
              {collection.name}
            </button>
          )) : <span>Not in a collection</span>}
        </div>
      </details>
      <button type="button" role="menuitem" onClick={() => run(() => setSelectedFavorites(!allStarred))}>
        {allStarred ? <StarOff size={15} aria-hidden /> : <Star size={15} aria-hidden />}
        {allStarred ? 'Unstar' : 'Star'}
      </button>
      <button type="button" role="menuitem" className="is-danger" onClick={() => run(deleteSelected)}>
        <Trash2 size={15} aria-hidden /> {isMultiple ? `Delete ${selected.length} items` : 'Delete'}
      </button>
    </div>,
    document.body,
  );
}
