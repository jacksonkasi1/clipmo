import { ArrowLeft } from 'lucide-react';
import type { CSSProperties } from 'react';

import { useStore } from '../lib/store';
import { tagColorKey } from '../lib/tag-color';
import type { CollectionSummary } from '../lib/types';
import { FluentFolderIcon } from './FluentFolderIcon';

interface Props {
  collection: CollectionSummary;
  tone: number;
  onBack: () => void;
}

/** Keeps filtered history visibly anchored inside its collection with Windows Fluent styling. */
export function CollectionContextBar({ collection, tone, onBack }: Props) {
  const tagColors = useStore((state) => state.tagColors);
  const customColor = tagColors[tagColorKey(collection.name)];
  const style = customColor ? ({ '--collection-color': customColor } as CSSProperties) : undefined;

  return (
    <header
      className={`collection-context-bar ${customColor ? 'has-custom-color' : `collection-tone-${tone}`}`}
      style={style}
      aria-label={`Inside ${collection.name}`}
    >
      <button type="button" onClick={onBack} aria-label="Back to collections" title="Back to collections">
        <ArrowLeft size={15} aria-hidden />
      </button>
      <span className="collection-context-icon" aria-hidden>
        <FluentFolderIcon size={18} isOpen={true} color={customColor} />
      </span>
      <strong>{collection.name}</strong>
      <span className="collection-context-count">
        {collection.itemCount} {collection.itemCount === 1 ? 'item' : 'items'}
      </span>
    </header>
  );
}
