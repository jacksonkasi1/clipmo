import {
  ArrowLeft,
  ArrowRight,
  Check,
  Folder,
  FolderOpen,
  Pipette,
  Plus,
  Search,
  Trash2,
  X,
} from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';

import { useStore } from '../lib/store';
import { tagColorClass, tagColorKey } from '../lib/tag-color';
import type { ClipItem } from '../lib/types';
import { KindIcon } from './KindIcon';
import { PopoutFolderCard } from './PopoutFolderCard';

const PRESET_COLORS = [
  '#39b9e8', // Accent Cyan/Blue
  '#62c68b', // Emerald Green
  '#89d329', // Lime Green
  '#f1a13b', // Amber Orange
  '#a98df0', // Purple
  '#34c8b2', // Teal
  '#ef68b2', // Pink
  '#ef7777', // Coral Red
];

interface Props {
  onBack: () => void;
}

/** Windows Fluent Collections Master-Detail View with 3D Pop-out Folders */
export function CollectionsView({ onBack }: Props) {
  const collections = useStore((state) => state.collections);
  const items = useStore((state) => state.items);
  const tagColors = useStore((state) => state.tagColors);
  const createCollection = useStore((state) => state.createCollection);
  const deleteCollection = useStore((state) => state.deleteCollection);
  const setTagColor = useStore((state) => state.setTagColor);
  const setTag = useStore((state) => state.setTag);

  const [selectedName, setSelectedName] = useState<string | null>(null);
  const [filterQuery, setFilterQuery] = useState('');
  const [name, setName] = useState('');
  const [creating, setCreating] = useState(false);
  const [collectionToDelete, setCollectionToDelete] = useState<string | null>(null);

  const inputRef = useRef<HTMLInputElement>(null);
  const confirmDeleteBtnRef = useRef<HTMLButtonElement>(null);

  // Auto-select first collection if none selected or if previous was deleted
  useEffect(() => {
    if (collections.length > 0) {
      if (!selectedName || !collections.some((c) => c.name === selectedName)) {
        const firstCollection = collections[0];
        if (firstCollection) {
          setSelectedName(firstCollection.name);
        }
      }
    } else {
      setSelectedName(null);
    }
  }, [collections, selectedName]);

  useEffect(() => {
    if (creating) inputRef.current?.focus();
  }, [creating]);

  useEffect(() => {
    if (collectionToDelete) {
      confirmDeleteBtnRef.current?.focus();
    }
  }, [collectionToDelete]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        if (collectionToDelete) {
          setCollectionToDelete(null);
        } else if (creating) {
          setCreating(false);
          setName('');
        } else {
          onBack();
        }
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [collectionToDelete, creating, onBack]);

  const create = async () => {
    const trimmed = name.trim();
    if (!trimmed) return;
    await createCollection(trimmed);
    setSelectedName(trimmed);
    setName('');
    setCreating(false);
  };

  const handleConfirmDelete = async () => {
    if (!collectionToDelete) return;
    const target = collectionToDelete;
    setCollectionToDelete(null);
    await deleteCollection(target);
  };

  const filteredCollections = useMemo(() => {
    if (!filterQuery.trim()) return collections;
    return collections.filter((c) =>
      c.name.toLowerCase().includes(filterQuery.trim().toLowerCase()),
    );
  }, [collections, filterQuery]);

  const activeCollection = useMemo(() => {
    return collections.find((c) => c.name === selectedName) || collections[0] || null;
  }, [collections, selectedName]);

  const activeCollectionItems = useMemo(() => {
    if (!activeCollection) return [];
    return items.filter((item: ClipItem) =>
      item.tags.some((t) => t.trim().toLowerCase() === activeCollection.name.trim().toLowerCase()),
    );
  }, [items, activeCollection]);

  const activeCustomColor = activeCollection ? tagColors[tagColorKey(activeCollection.name)] : undefined;
  const activeToneIndex = activeCollection
    ? collections.findIndex((c) => c.name === activeCollection.name) % 6
    : 0;

  const openActiveInHistory = (colName?: string) => {
    const target = colName || activeCollection?.name;
    if (target) {
      void setTag(target);
      onBack();
    }
  };

  return (
    <main className="collections-view" aria-label="Collections">
      {/* Top Header */}
      <header className="collections-header">
        <button
          type="button"
          className="collection-header-button"
          onClick={onBack}
          aria-label="Back to history"
          title="Back to history (Esc)"
        >
          <ArrowLeft size={18} aria-hidden />
        </button>
        <div className="collections-title">
          <h1>Collections</h1>
          <span>{collections.length} {collections.length === 1 ? 'folder' : 'folders'}</span>
        </div>

        {/* Search / Filter */}
        {collections.length > 3 && (
          <div className="collections-search">
            <Search size={14} className="collections-search-icon" aria-hidden />
            <input
              type="search"
              placeholder="Filter folders..."
              value={filterQuery}
              onChange={(e) => setFilterQuery(e.target.value)}
              aria-label="Filter collections"
            />
            {filterQuery && (
              <button
                type="button"
                onClick={() => setFilterQuery('')}
                aria-label="Clear filter"
                className="collections-search-clear"
              >
                <X size={12} aria-hidden />
              </button>
            )}
          </div>
        )}

        {creating ? (
          <div className="collection-create">
            <input
              ref={inputRef}
              value={name}
              onChange={(event) => setName(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === 'Enter') void create();
              }}
              placeholder="Collection name"
              aria-label="Collection name"
              maxLength={40}
            />
            <button type="button" onClick={() => void create()} aria-label="Save collection" disabled={!name.trim()}>
              <Check size={16} aria-hidden />
            </button>
            <button type="button" onClick={() => { setCreating(false); setName(''); }} aria-label="Cancel">
              <X size={16} aria-hidden />
            </button>
          </div>
        ) : (
          <button
            type="button"
            className="collection-header-button is-primary"
            onClick={() => setCreating(true)}
            aria-label="New collection"
            title="New collection"
          >
            <Plus size={18} aria-hidden />
          </button>
        )}
      </header>

      {/* Main Master-Detail Layout */}
      {collections.length > 0 ? (
        <div className="collections-layout">
          {/* Left: Folder Gallery */}
          <section className="collections-gallery" aria-label="Folder Gallery">
            <div className="collections-popout-grid">
              {filteredCollections.map((col, idx) => {
                const customCol = tagColors[tagColorKey(col.name)];
                const isSelected = activeCollection?.name === col.name;
                return (
                  <PopoutFolderCard
                    key={col.name}
                    collection={col}
                    items={items}
                    color={customCol}
                    toneIndex={idx % 6}
                    isSelected={isSelected}
                    onClick={() => setSelectedName(col.name)}
                    onOpenInHistory={() => openActiveInHistory(col.name)}
                  />
                );
              })}
            </div>
          </section>

          {/* Right: Simplified, Clean Collection Inspector */}
          {activeCollection && (
            <aside
              className={`collection-inspector ${activeCustomColor ? 'has-custom-color' : `collection-tone-${activeToneIndex} ${tagColorClass(activeCollection.name)}`}`}
              style={activeCustomColor ? ({ '--folder-color': activeCustomColor } as React.CSSProperties) : undefined}
              aria-label="Collection Details"
            >
              <div className="inspector-header">
                <div className="inspector-title-wrap">
                  <h2>{activeCollection.name}</h2>
                  <span className="inspector-badge">
                    {activeCollection.itemCount} {activeCollection.itemCount === 1 ? 'item' : 'items'}
                  </span>
                </div>
                <button
                  type="button"
                  className="inspector-delete-btn"
                  aria-label={`Delete ${activeCollection.name}`}
                  title="Delete collection"
                  onClick={() => setCollectionToDelete(activeCollection.name)}
                >
                  <Trash2 size={15} aria-hidden />
                </button>
              </div>

              {/* Refined Rounded Color Theme Card */}
              <div className="inspector-color-card">
                <div className="inspector-color-card-header">
                  <span className="inspector-color-card-title">Folder Color</span>
                  <button
                    type="button"
                    className={`inspector-auto-btn ${!activeCustomColor ? 'is-active' : ''}`}
                    onClick={() => setTagColor(activeCollection.name, null)}
                    title="Restore automatic folder color"
                  >
                    Auto
                  </button>
                </div>
                <div className="inspector-color-chips">
                  {PRESET_COLORS.map((preset) => {
                    const isSelected = activeCustomColor?.toLowerCase() === preset.toLowerCase();
                    return (
                      <button
                        key={preset}
                        type="button"
                        className={`inspector-color-chip ${isSelected ? 'is-selected' : ''}`}
                        style={{ backgroundColor: preset }}
                        title={`Color ${preset}`}
                        aria-label={`Set color ${preset}`}
                        onClick={() => setTagColor(activeCollection.name, preset)}
                      >
                        {isSelected && <Check size={12} strokeWidth={3} className="chip-check" aria-hidden />}
                      </button>
                    );
                  })}
                  {/* Styled Custom Color Picker Button */}
                  <label
                    className={`inspector-custom-chip ${activeCustomColor && !PRESET_COLORS.some((p) => p.toLowerCase() === activeCustomColor.toLowerCase()) ? 'is-selected' : ''}`}
                    title="Pick custom color"
                    style={
                      activeCustomColor && !PRESET_COLORS.some((p) => p.toLowerCase() === activeCustomColor.toLowerCase())
                        ? { backgroundColor: activeCustomColor }
                        : undefined
                    }
                  >
                    <Pipette size={12} strokeWidth={2.5} className="custom-pipette-icon" aria-hidden />
                    <input
                      type="color"
                      className="custom-color-hidden-input"
                      value={activeCustomColor || '#39b9e8'}
                      onChange={(e) => setTagColor(activeCollection.name, e.target.value)}
                      aria-label="Custom color picker"
                    />
                  </label>
                </div>
              </div>

              {/* Action Button */}
              <button
                type="button"
                className="inspector-open-btn"
                onClick={() => openActiveInHistory()}
                aria-label={`Open ${activeCollection.name} in history`}
              >
                <FolderOpen size={16} aria-hidden />
                <span>Open in History</span>
                <ArrowRight size={15} aria-hidden />
              </button>

              {/* Recent Items Preview in This Collection */}
              <div className="inspector-items-section">
                <div className="inspector-section-header">
                  <span>Recent clips</span>
                  <span className="inspector-section-count">{activeCollectionItems.length}</span>
                </div>

                {activeCollectionItems.length > 0 ? (
                  <div className="inspector-items-list">
                    {activeCollectionItems.slice(0, 6).map((clip) => (
                      <div
                        key={clip.id}
                        className="inspector-item-row"
                        onClick={() => openActiveInHistory()}
                        title="Click to open collection"
                      >
                        <span className="inspector-item-icon">
                          <KindIcon item={clip} size={14} />
                        </span>
                        <span className="inspector-item-text">
                          {clip.preview.trim() || 'Untitled clip'}
                        </span>
                        {clip.source?.name && (
                          <span className="inspector-item-app">{clip.source.name}</span>
                        )}
                      </div>
                    ))}
                  </div>
                ) : (
                  <div className="inspector-empty-note">
                    <span>No clips in this collection yet.</span>
                  </div>
                )}
              </div>
            </aside>
          )}
        </div>
      ) : (
        <div className="collections-empty">
          <div className="collections-empty-visual">
            <Folder size={44} aria-hidden />
          </div>
          <strong>No collections yet</strong>
          <span>Create custom folders to organize your clipboard history.</span>
          <button type="button" className="collection-empty-create" onClick={() => setCreating(true)}>
            <Plus size={15} aria-hidden /> Create first collection
          </button>
        </div>
      )}

      {/* Safe Delete Confirmation Modal */}
      {collectionToDelete && (
        <div
          className="collection-dialog-scrim"
          role="presentation"
          onClick={() => setCollectionToDelete(null)}
        >
          <div
            className="collection-dialog"
            role="alertdialog"
            aria-modal="true"
            aria-labelledby="collection-dialog-title"
            aria-describedby="collection-dialog-desc"
            onClick={(e) => e.stopPropagation()}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault();
                void handleConfirmDelete();
              }
            }}
          >
            <div className="collection-dialog-header">
              <span className="collection-dialog-icon">
                <Folder size={22} aria-hidden />
              </span>
              <h2 id="collection-dialog-title">Delete collection &ldquo;{collectionToDelete}&rdquo;?</h2>
            </div>
            <p id="collection-dialog-desc">
              Clips will remain in your history, but will be removed from this collection.
            </p>
            <div className="collection-dialog-actions">
              <button
                type="button"
                className="collection-dialog-button is-secondary"
                onClick={() => setCollectionToDelete(null)}
              >
                Cancel
              </button>
              <button
                ref={confirmDeleteBtnRef}
                type="button"
                className="collection-dialog-button is-danger"
                onClick={() => void handleConfirmDelete()}
              >
                Delete
              </button>
            </div>
          </div>
        </div>
      )}
    </main>
  );
}
