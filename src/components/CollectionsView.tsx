// ** import lib
import { ArrowLeft, Check, FileText, Folder, Plus, Trash2, X } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

import { useStore } from '../lib/store';

interface Props {
  onBack: () => void;
}

/** Folder-style collection overview, matching the mobile collection concept. */
export function CollectionsView({ onBack }: Props) {
  const collections = useStore((state) => state.collections);
  const createCollection = useStore((state) => state.createCollection);
  const deleteCollection = useStore((state) => state.deleteCollection);
  const setTag = useStore((state) => state.setTag);
  const [name, setName] = useState('');
  const [creating, setCreating] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (creating) inputRef.current?.focus();
  }, [creating]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape') return;
      if (creating) {
        setCreating(false);
        setName('');
      } else {
        onBack();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [creating, onBack]);

  const create = async () => {
    if (!name.trim()) return;
    await createCollection(name);
    setName('');
    setCreating(false);
  };

  return (
    <main className="collections-view" aria-label="Collections">
      <header className="collections-header">
        <button type="button" className="collection-header-button" onClick={onBack} aria-label="Back to history" title="Back to history (Esc)">
          <ArrowLeft size={18} aria-hidden />
        </button>
        <div className="collections-title">
          <h1>Collections</h1>
          <span>{collections.length} {collections.length === 1 ? 'folder' : 'folders'}</span>
        </div>
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
          <button type="button" className="collection-header-button is-primary" onClick={() => setCreating(true)} aria-label="New collection" title="New collection">
            <Plus size={18} aria-hidden />
          </button>
        )}
      </header>
      {collections.length ? (
        <div className="collection-grid">
          {collections.map((collection, index) => (
            <article key={collection.name} className={`collection-card collection-tone-${index % 6}`}>
              <button
                type="button"
                className="collection-open"
                aria-label={`Open ${collection.name}, ${collection.itemCount} ${collection.itemCount === 1 ? 'item' : 'items'}`}
                onClick={() => { void setTag(collection.name); onBack(); }}
              >
                <span className="collection-folder" aria-hidden>
                  <Folder className="collection-folder-back" fill="currentColor" strokeWidth={1.5} />
                  <span className="collection-papers">
                    <FileText /><FileText /><FileText />
                  </span>
                  <span className="collection-folder-front" />
                </span>
                <span className="collection-label">
                  <strong>{collection.name}</strong>
                  <span>{collection.itemCount} {collection.itemCount === 1 ? 'item' : 'items'}</span>
                </span>
              </button>
              <button type="button" className="collection-delete" aria-label={`Delete ${collection.name}`} title="Delete collection" onClick={() => void deleteCollection(collection.name)}>
                <Trash2 size={15} aria-hidden />
              </button>
            </article>
          ))}
        </div>
      ) : (
        <div className="collections-empty">
          <span className="collections-empty-icon"><Folder size={28} aria-hidden /></span>
          <strong>No collections yet</strong>
          <span>Use + to create your first folder.</span>
          <button type="button" className="collection-empty-create" onClick={() => setCreating(true)}>
            <Plus size={15} aria-hidden /> New collection
          </button>
        </div>
      )}
    </main>
  );
}
