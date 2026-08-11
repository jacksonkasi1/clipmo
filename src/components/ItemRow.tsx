// ** import types
import type { ClipItem } from '../lib/types';
import type { MouseEvent } from 'react';
import type { WindowMode } from '../lib/window-mode';

// ** import lib
import { Star } from 'lucide-react';

import { KindIcon } from './KindIcon';
import { IconButton } from './IconButton';
import { useStore } from '../lib/store';

interface Props {
  item: ClipItem;
  selected: boolean;
  /** True when the row is part of a multi-selection (highlighted but not the primary row). */
  multiSelected?: boolean;
  /** True when the list has keyboard focus and this row is the active one. */
  focused?: boolean;
  /** Quick rows are single-line; the full application affords one subtitle. */
  mode?: WindowMode;
  position: number;
  total: number;
  onSelect: (event: MouseEvent<HTMLDivElement>) => void;
}

export function ItemRow({
  item,
  selected,
  multiSelected = false,
  focused = false,
  mode = 'full',
  position,
  total,
  onSelect,
}: Props) {
  const toggleFavorite = useStore((s) => s.toggleFavorite);
  // The left list is for scanning, so it carries the smallest amount of
  // information that still identifies a row. A remote device name remains
  // visible as quiet provenance; verbose sync wording and copy counts live in
  // the details pane.
  const hasSyncState = item.syncStatus !== 'local';
  const remoteDeviceName = hasSyncState && item.device.name !== 'This device'
    ? item.device.name
    : null;
  const syncDescription = remoteDeviceName
    ? `${remoteDeviceName} · ${item.syncStatus}`
    : `Sync ${item.syncStatus}`;

  return (
    <div
      role="option"
      id={`clip-item-${item.id}`}
      aria-selected={selected || multiSelected}
      aria-posinset={position}
      aria-setsize={total}
      className={[
        'item-row',
        selected ? 'selected' : '',
        multiSelected ? 'is-multi-selected' : '',
        focused ? 'is-focused' : '',
        `item-kind-${item.kind}`,
      ].filter(Boolean).join(' ')}
      title="Double-click to paste"
      onClick={onSelect}
      onDoubleClick={() => {
        void import('../lib/tauri').then((m) => m.api.pasteActive(item.id, 'original'));
      }}
    >
      <span className="kind-icon">
        <KindIcon item={item} />
      </span>
      <div className="row-content">
        <div className="row-title" title={item.preview}>
          {item.preview || '(empty)'}
        </div>
        {/* One quiet source label, and only where there is room for it. The
            flyout stays single-line so more rows fit on screen. */}
        {mode === 'full' && (
          <div className="row-subtitle">
            <span>{item.source?.name ?? kindLabel(item.kind)}</span>
            {remoteDeviceName && <span className="row-device-name"> · {remoteDeviceName}</span>}
          </div>
        )}
      </div>
      <div className="row-trailing">
        {/* Cross-device state is a single dot, not a badge with text and icons. */}
        {hasSyncState && (
          <span
            className={`row-signal is-${item.syncStatus}`}
            title={syncDescription}
            aria-label={syncDescription}
            role="img"
          />
        )}
        <IconButton
          label={item.favorite ? 'Remove from favorites' : 'Add to favorites'}
          active={item.favorite}
          className="favorite-button"
          onClick={(e) => {
            e.stopPropagation();
            void toggleFavorite(item.id);
          }}
          onDoubleClick={(event) => event.stopPropagation()}
        >
          <Star size={15} fill={item.favorite ? 'currentColor' : 'none'} aria-hidden />
        </IconButton>
      </div>
    </div>
  );
}

function kindLabel(kind: ClipItem['kind']): string {
  return kind.charAt(0).toUpperCase() + kind.slice(1);
}
