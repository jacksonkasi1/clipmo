// ** import lib
import { useEffect, useState } from 'react';
import { AppWindow, Plus, Tag, X } from 'lucide-react';

import { DeviceBadge } from './DeviceBadge';
import { formatBytes } from '../lib/formatting';
import { useStore } from '../lib/store';
import { tagColorClass, tagColorKey } from '../lib/tag-color';

const KIND_LABEL: Record<string, string> = {
  text: 'Plain text',
  link: 'Link',
  email: 'Email',
  color: 'Color',
  image: 'Image',
  files: 'File(s)',
};

export function DetailsTable() {
  const selectedId = useStore((s) => s.selectedId);
  const items = useStore((s) => s.items);
  const setItemTags = useStore((s) => s.setItemTags);
  const item = items.find((i) => i.id === selectedId);

  if (!item) return null;

  const app = item.source?.name ?? 'Unknown';
  const kind = KIND_LABEL[item.kind] ?? item.kind;
  const size = formatBytes(item.sizeBytes);

  return (
    <section className="details-panel" aria-label="Item details">
      <dl>
        <Row
          label="Application"
          value={<span className="source-value"><AppWindow size={15} aria-hidden />{app}</span>}
        />
        <Row label="Type" value={kind} />
        <Row label="Device" value={<DeviceBadge device={item.device} status={item.syncStatus} />} />
        <Row label="Sync status" value={syncLabel(item.syncStatus)} />
        <Row label="Number of copies" value={String(item.copyCount)} />
        <Row label="First copy time" value={formatDate(item.firstCopiedAt)} />
        <Row label="Last copy time" value={formatDate(item.lastCopiedAt)} />
        {item.kind === 'image' && item.image && (
          <Row
            label="Dimensions"
            value={`${item.image.width} × ${item.image.height} px`}
          />
        )}
        {item.sizeBytes > 0 && <Row label="Size" value={size} />}
        <Row label="Tags" value={<TagEditor itemId={item.id} tags={item.tags} onSave={setItemTags} />} />
      </dl>
    </section>
  );
}

function TagEditor({ itemId, tags, onSave }: { itemId: number; tags: string[]; onSave: (id: number, tags: string[]) => Promise<void> }) {
  const tagColors = useStore((state) => state.tagColors);
  const [value, setValue] = useState('');
  useEffect(() => setValue(''), [itemId]);
  const add = () => {
    const next = value.trim().replace(/^#/, '').toLowerCase();
    if (!next || tags.includes(next)) return;
    setValue('');
    void onSave(itemId, [...tags, next]);
  };
  return (
    <div className="tag-editor">
      <div className="tag-list">
        {tags.map((tag) => (
          <button key={tag} type="button" className={`tag-chip ${tagColorClass(tag)}`} style={{ color: tagColors[tagColorKey(tag)] }} title={`Remove #${tag}`} onClick={() => void onSave(itemId, tags.filter((value) => value !== tag))}>
            <Tag size={12} fill="currentColor" aria-hidden /> {tag} <X size={11} aria-hidden />
          </button>
        ))}
      </div>
      <div className="tag-input-wrap">
        <input value={value} maxLength={32} placeholder="Add tag" aria-label="Add tag" onChange={(event) => setValue(event.target.value)} onKeyDown={(event) => { if (event.key === 'Enter') { event.preventDefault(); add(); } }} />
        <button type="button" aria-label="Add tag" disabled={!value.trim()} onClick={add}><Plus size={13} aria-hidden /></button>
      </div>
    </div>
  );
}

function syncLabel(status: string): string {
  if (status === 'synced') return 'Synced';
  if (status === 'pending') return 'Pending';
  if (status === 'offline') return 'Offline';
  return 'Local';
}

function Row({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="metadata-row">
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

function formatDate(ms: number): string {
  if (!ms) return '—';
  return new Date(ms).toLocaleString();
}
