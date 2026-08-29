// ** import lib
import { Folder, Plus, Trash2 } from 'lucide-react';
import { useState } from 'react';

import { useStore } from '../lib/store';

interface Props { onBack: () => void; }

/** Folder-style collection overview, matching the mobile collection concept. */
export function CollectionsView({ onBack }: Props) {
  const collections = useStore((state) => state.collections);
  const createCollection = useStore((state) => state.createCollection);
  const deleteCollection = useStore((state) => state.deleteCollection);
  const setTag = useStore((state) => state.setTag);
  const [name, setName] = useState('');

  const create = async () => {
    if (!name.trim()) return;
    await createCollection(name);
    setName('');
  };
  return <main className="collections-view" aria-label="Collections">
    <header className="collections-header">
      <div><p>Organize clipboard items</p><h1>Collections</h1></div>
      <div className="collection-create"><input value={name} onChange={(event) => setName(event.target.value)} onKeyDown={(event) => { if (event.key === 'Enter') void create(); }} placeholder="New collection" maxLength={40} /><button type="button" onClick={() => void create()} aria-label="Create collection"><Plus size={17} aria-hidden /></button></div>
      <button type="button" className="text-button" onClick={onBack}>Back to history</button>
    </header>
    {collections.length ? <div className="collection-grid">
      {collections.map((collection, index) => <article key={collection.name} className={`collection-card collection-tone-${index % 6}`}>
        <button type="button" className="collection-open" onClick={() => { void setTag(collection.name); onBack(); }}>
          <Folder size={44} fill="currentColor" aria-hidden /><strong>{collection.name}</strong><span>{collection.itemCount} {collection.itemCount === 1 ? 'item' : 'items'}</span>
        </button>
        <button type="button" className="collection-delete" aria-label={`Delete ${collection.name}`} onClick={() => void deleteCollection(collection.name)}><Trash2 size={15} aria-hidden /></button>
      </article>)}
    </div> : <div className="collections-empty"><Folder size={30} aria-hidden /><strong>No collections yet</strong><span>Create one, then use the multi-select right-click menu to add items.</span></div>}
  </main>;
}
