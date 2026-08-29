// ** import lib
import { FolderPlus, FolderX, Star, StarOff, Trash2 } from 'lucide-react';

import { useStore } from '../lib/store';

interface Props {
  x: number;
  y: number;
  onClose: () => void;
}

/** Context controls for the current multi-selection. */
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

  const run = (action: () => Promise<void>) => {
    onClose();
    void action();
  };

  return (
    <div className="selection-context-menu" role="menu" style={{ left: x, top: y }}>
      <div className="selection-context-title">{selectedIds.length} selected</div>
      <details>
        <summary><FolderPlus size={15} aria-hidden /> Add to collection</summary>
        <div className="selection-context-submenu">
          {collections.length ? collections.map((collection) => (
            <button key={collection.name} type="button" role="menuitem" onClick={() => run(() => addSelectedToCollection(collection.name))}>
              {collection.name}
            </button>
          )) : <span>Create a collection first</span>}
        </div>
      </details>
      <details>
        <summary><FolderX size={15} aria-hidden /> Remove from collection</summary>
        <div className="selection-context-submenu">
          {collections.map((collection) => (
            <button key={collection.name} type="button" role="menuitem" onClick={() => run(() => removeSelectedFromCollection(collection.name))}>
              {collection.name}
            </button>
          ))}
        </div>
      </details>
      <button type="button" role="menuitem" onClick={() => run(() => setSelectedFavorites(!allStarred))}>
        {allStarred ? <StarOff size={15} aria-hidden /> : <Star size={15} aria-hidden />}
        {allStarred ? 'Remove star from all' : 'Star all'}
      </button>
      <button type="button" role="menuitem" className="is-danger" onClick={() => run(deleteSelected)}>
        <Trash2 size={15} aria-hidden /> Delete selected
      </button>
    </div>
  );
}
