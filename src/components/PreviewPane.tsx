// ** import types
import type { ClipItem } from '../lib/types';

// ** import lib
import { useEffect, useRef, useState } from 'react';
import {
  AppWindow,
  CheckCircle2,
  CircleMinus,
  ClipboardCopy,
  Copy,
  ExternalLink,
  File,
  FileImage,
  Folder,
  FolderOpen,
  Layers,
  Link2,
  LoaderCircle,
  Mail,
  Maximize2,
  MoreVertical,
  PanelBottomClose,
  PanelBottomOpen,
  Pencil,
  Save,
  Star,
  Trash2,
  TriangleAlert,
  X,
} from 'lucide-react';

import { IconButton } from './IconButton';
import { copySelectedItems, pasteSelectedItems } from '../lib/clipboard-actions';
import { KindIcon } from './KindIcon';
import { useStore } from '../lib/store';
import { api, fileSrc } from '../lib/tauri';
import { getShortcutLabel } from '../lib/platform';
import { normaliseUrl, tryParseScheme } from '../lib/url';
import { toast } from '../lib/toast';
import { formatBytes } from '../lib/formatting';

/** Loads an image and falls back to a thumbnail-friendly placeholder on error. */
function SafeImage({
  src,
  alt,
  className,
  fallback,
}: {
  src: string;
  alt: string;
  className?: string;
  fallback: React.ReactNode;
}) {
  const [failed, setFailed] = useState(false);
  if (failed) return <>{fallback}</>;
  return (
    <img
      src={src}
      alt={alt}
      className={className}
      onError={() => setFailed(true)}
      loading="lazy"
      decoding="async"
    />
  );
}

export function PreviewPane() {
  const selectedId = useStore((s) => s.selectedId);
  const selectedIds = useStore((s) => s.selectedIds);
  const items = useStore((s) => s.items);
  const editItem = useStore((s) => s.editItem);
  const item = items.find((entry) => entry.id === selectedId) ?? null;
  const isMulti = selectedIds.length > 1;
  const [editing, setEditing] = useState(false);

  useEffect(() => setEditing(false), [selectedId]);
  useEffect(() => {
    const beginEditing = () => {
      if (item && !['image', 'files'].includes(item.kind) && !isMulti) setEditing(true);
    };
    window.addEventListener('clipmo:edit-selected', beginEditing);
    return () => window.removeEventListener('clipmo:edit-selected', beginEditing);
  }, [item?.id, isMulti]);

  return (
    <section className="preview-pane" aria-label="Preview">
      <PreviewToolbar item={item} onEdit={() => setEditing(true)} />
      {isMulti ? (
        <MultiItemPreview items={items} selectedIds={selectedIds} />
      ) : item ? (
        editing ? (
          <EditItem
            item={item}
            onCancel={() => setEditing(false)}
            onSave={async (content) => {
              await editItem(item.id, content);
              setEditing(false);
            }}
          />
        ) : (
          <PreviewBody item={item} onEdit={() => setEditing(true)} />
        )
      ) : (
        <PreviewEmpty />
      )}
    </section>
  );
}

function PreviewToolbar({ item, onEdit }: { item: ClipItem | null; onEdit: () => void }) {
  const showDetails = useStore((s) => s.showDetails);
  const setShowDetails = useStore((s) => s.setShowDetails);
  const toggleFavorite = useStore((s) => s.toggleFavorite);
  const deleteItem = useStore((s) => s.deleteItem);
  const deleteSelected = useStore((s) => s.deleteSelected);
  const selectedIds = useStore((s) => s.selectedIds);
  const isMulti = selectedIds.length > 1;
  const editable = !isMulti && item && ['text', 'link', 'email', 'color'].includes(item.kind);
  const contextAction = !isMulti ? describeContextAction(item) : null;
  const sourceName = isMulti ? `${selectedIds.length} items` : (item?.source?.name ?? null);

  const handleCopy = () => {
    if (isMulti) {
      void copySelectedItems(selectedIds);
    } else if (item) {
      void api.copyToClipboard(item.id, 'original');
    }
  };

  const handlePaste = () => {
    if (isMulti) {
      void pasteSelectedItems(selectedIds);
    } else if (item) {
      void api.pasteActive(item.id, 'original');
    }
  };

  return (
    <div className="preview-toolbar" role="toolbar" aria-label="Item actions">
      <SourceIndicator name={sourceName} isMulti={isMulti} />
      <div className="toolbar-spacer" />
      <div className="toolbar-group toolbar-group--primary">
        <IconButton
          label={isMulti ? `Copy ${selectedIds.length} items (${getShortcutLabel('copy')})` : `Copy to clipboard (${getShortcutLabel('copy')})`}
          disabled={!item && !isMulti}
          onClick={handleCopy}
        >
          <Copy size={18} aria-hidden />
        </IconButton>
        <IconButton
          label={isMulti ? `Paste ${selectedIds.length} items (${getShortcutLabel('paste')})` : `Paste to active application (${getShortcutLabel('paste')})`}
          disabled={!item && !isMulti}
          onClick={handlePaste}
        >
          <ClipboardCopy size={18} aria-hidden />
        </IconButton>
        {editable && (
          <IconButton label={`Edit item (${getShortcutLabel('edit')})`} onClick={onEdit}>
            <Pencil size={18} aria-hidden />
          </IconButton>
        )}
        {contextAction && (
          <IconButton
            label={contextAction.label}
            disabled={!item}
            onClick={() => item && void contextAction.run()}
          >
            {contextAction.icon}
          </IconButton>
        )}
      </div>
      <div className="toolbar-group toolbar-group--secondary">
        <IconButton
          label={item?.favorite ? 'Remove from favorites' : 'Add to favorites'}
          active={item?.favorite ?? false}
          disabled={!item || isMulti}
          onClick={() => item && void toggleFavorite(item.id)}
        >
          <Star size={19} fill={item?.favorite ? 'currentColor' : 'none'} aria-hidden />
        </IconButton>
        <OverflowMenu
          item={item}
          showDetails={showDetails}
          setShowDetails={setShowDetails}
          selectedCount={selectedIds.length}
          onDelete={async () => {
            if (isMulti) {
              await deleteSelected();
            } else if (item) {
              await deleteItem(item.id);
            }
          }}
        />
      </div>
    </div>
  );
}

/** Small "From {AppName}" tag with a generic window glyph for the source attribution. */
function SourceIndicator({ name, isMulti }: { name: string | null; isMulti?: boolean }) {
  if (isMulti) {
    return (
      <span className="source-indicator" title={`${name} selected`}>
        <Layers size={14} aria-hidden />
        <span className="source-indicator-name">{name}</span>
      </span>
    );
  }
  if (!name) {
    return (
      <span className="source-indicator" aria-hidden>
        <AppWindow size={14} />
      </span>
    );
  }
  return (
    <span className="source-indicator" title={`Copied from ${name}`}>
      <AppWindow size={14} aria-hidden />
      <span className="source-indicator-name">From {name}</span>
    </span>
  );
}

interface ContextAction {
  label: string;
  icon: React.ReactNode;
  run: () => Promise<void> | void;
}

function describeContextAction(item: ClipItem | null): ContextAction | null {
  if (!item) return null;
  if (item.kind === 'link' || item.kind === 'email') {
    const raw = item.kind === 'email' ? `mailto:${item.content || item.preview}` : (item.content || item.preview);
    return {
      label: 'Open in browser',
      icon: <ExternalLink size={18} aria-hidden />,
      run: () => openExternalLink(raw),
    };
  }
  if (item.kind === 'files') {
    const target = item.fileAssets[0]?.storedPath
      ?? item.fileAssets[0]?.originalPath
      ?? item.files[0];
    if (!target) return null;
    return {
      label: 'Reveal in File Explorer',
      icon: <FolderOpen size={18} aria-hidden />,
      run: () => api.revealItem(target),
    };
  }
  if (item.kind === 'image' && item.image?.path) {
    return {
      label: 'Reveal in File Explorer',
      icon: <FolderOpen size={18} aria-hidden />,
      run: () => api.revealItem(item.image!.path),
    };
  }
  return null;
}

async function openExternalLink(raw: string): Promise<void> {
  const scheme = tryParseScheme(raw);
  if (!scheme) {
    toast('That link is not a URL Clipmo can open.', 'error');
    return;
  }
  try {
    await api.openExternalUrl(normaliseUrl(raw));
  } catch (error: unknown) {
    toast(`The default browser could not be opened: ${String(error)}`, 'error');
  }
}

/** A small kebab menu that surfaces the less common per-item actions. */
function OverflowMenu({
  item,
  showDetails,
  setShowDetails,
  selectedCount,
  onDelete,
}: {
  item: ClipItem | null;
  showDetails: boolean;
  setShowDetails: (show: boolean) => void;
  selectedCount: number;
  onDelete: () => void | Promise<void>;
}) {
  const [open, setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    if (!open) return undefined;
    const onDocumentClick = (event: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    const onEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setOpen(false);
    };
    document.addEventListener('mousedown', onDocumentClick);
    document.addEventListener('keydown', onEscape);
    return () => {
      document.removeEventListener('mousedown', onDocumentClick);
      document.removeEventListener('keydown', onEscape);
    };
  }, [open]);
  return (
    <div className="toolbar-overflow" ref={containerRef}>
      <IconButton
        label="More actions"
        active={open}
        disabled={!item}
        onClick={() => setOpen(!open)}
        aria-haspopup="menu"
        aria-expanded={open}
      >
        <MoreVertical size={18} aria-hidden />
      </IconButton>
      {open && (
        <div role="menu" className="toolbar-overflow-menu">
          <button
            type="button"
            role="menuitemcheckbox"
            aria-checked={item ? showDetails : false}
            className={`toolbar-overflow-item${item && showDetails ? ' is-active' : ''}`}
            onClick={() => {
              setShowDetails(!showDetails);
              setOpen(false);
            }}
          >
            {item && showDetails ? (
              <PanelBottomClose size={15} aria-hidden />
            ) : (
              <PanelBottomOpen size={15} aria-hidden />
            )}
            <span>{item && showDetails ? 'Hide details' : 'Show details'}</span>
          </button>
          <button
            type="button"
            role="menuitem"
            className="toolbar-overflow-item is-danger"
            onClick={async () => {
              setOpen(false);
              await onDelete();
            }}
          >
            <Trash2 size={15} aria-hidden />
            <span>{selectedCount > 1 ? `Delete ${selectedCount} items` : 'Delete item'}</span>
          </button>
        </div>
      )}
    </div>
  );
}

function PreviewBody({ item, onEdit }: { item: ClipItem; onEdit: () => void }) {
  switch (item.kind) {
    case 'image':
      return <ImagePreview item={item} />;
    case 'color':
      return <ColorPreview value={item.content || item.preview.trim()} onEdit={onEdit} />;
    case 'files':
      return <FilePreview item={item} />;
    case 'link':
      return <LinkPreview item={item} onEdit={onEdit} />;
    case 'email':
      return <EmailPreview item={item} onEdit={onEdit} />;
    default:
      return <TextPreview item={item} onEdit={onEdit} />;
  }
}

function TextPreview({ item, onEdit }: { item: ClipItem; onEdit: () => void }) {
  const codeLike = /(^|\n)\s*(const|let|fn|use|import|SELECT|class|function)\b|[{};]\s*$/m.test(
    item.content,
  );
  return (
    <button
      type="button"
      className={`preview-scroll preview-text-wrap preview-edit-trigger ${codeLike ? 'is-code' : ''}`}
      onClick={onEdit}
      title="Edit item"
    >
      <pre className="preview-text">{item.content || item.preview}</pre>
    </button>
  );
}

function ImagePreview({ item }: { item: ClipItem }) {
  const [fullscreen, setFullscreen] = useState(false);

  if (!item.image) {
    return <PreviewFailure title={item.preview} message="The image preview is unavailable." />;
  }

  const imageSrc = fileSrc(item.image.path);

  return (
    <>
      <div className="preview-scroll preview-image-wrap">
        <div
          className="image-canvas"
          role="button"
          tabIndex={0}
          title="Click to view full screen"
          aria-label="Click image to view full screen"
          onClick={() => setFullscreen(true)}
          onKeyDown={(event) => {
            if (event.key === 'Enter' || event.key === ' ') {
              event.preventDefault();
              setFullscreen(true);
            }
          }}
        >
          <SafeImage
            src={imageSrc}
            alt={item.preview}
            className="preview-image"
            fallback={
              <div className="preview-fallback">
                <FileImage size={48} strokeWidth={1.4} aria-hidden />
                <span>The image preview is unavailable.</span>
              </div>
            }
          />
        </div>
        <div className="preview-caption">
          <div className="preview-caption-info">
            <FileImage size={16} aria-hidden />
            <span>{item.image.width} × {item.image.height} pixels</span>
          </div>
          <div className="preview-caption-actions">
            <IconButton
              label="View full screen"
              onClick={() => setFullscreen(true)}
            >
              <Maximize2 size={16} aria-hidden />
            </IconButton>
          </div>
        </div>
      </div>
      {fullscreen && (
        <FullscreenImageModal
          item={item}
          imageSrc={imageSrc}
          onClose={() => setFullscreen(false)}
        />
      )}
    </>
  );
}

function FullscreenImageModal({
  item,
  imageSrc,
  onClose,
}: {
  item: ClipItem;
  imageSrc: string;
  onClose: () => void;
}) {
  const closeButtonRef = useRef<HTMLButtonElement | null>(null);

  useEffect(() => {
    closeButtonRef.current?.focus();
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.preventDefault();
        event.stopPropagation();
        onClose();
      }
    };
    window.addEventListener('keydown', handleKeyDown, true);
    return () => window.removeEventListener('keydown', handleKeyDown, true);
  }, [onClose]);

  return (
    <div
      className="image-fullscreen-overlay"
      role="dialog"
      aria-modal="true"
      aria-label={`Full screen preview: ${item.preview}`}
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <header className="image-fullscreen-header">
        <div className="image-fullscreen-title-area">
          <FileImage size={18} aria-hidden />
          <span className="image-fullscreen-title">{item.preview || 'Image Preview'}</span>
          {item.image && (
            <span className="image-fullscreen-badge">
              {item.image.width} × {item.image.height} px
            </span>
          )}
        </div>
        <div className="image-fullscreen-actions">
          <IconButton
            label="Copy to clipboard"
            onClick={() => void api.copyToClipboard(item.id, 'original')}
          >
            <Copy size={17} aria-hidden />
          </IconButton>
          {item.image?.path && (
            <IconButton
              label="Reveal in File Explorer"
              onClick={() => void api.revealItem(item.image!.path)}
            >
              <FolderOpen size={17} aria-hidden />
            </IconButton>
          )}
          <button
            ref={closeButtonRef}
            type="button"
            className="image-fullscreen-close-btn"
            onClick={onClose}
            aria-label="Close full screen"
            title="Close (Esc)"
          >
            <X size={17} aria-hidden />
            <span>Close (Esc)</span>
          </button>
        </div>
      </header>
      <div
        className="image-fullscreen-content"
        onClick={(e) => {
          if (e.target === e.currentTarget) onClose();
        }}
      >
        <div className="image-fullscreen-img-wrap">
          <img
            src={imageSrc}
            alt={item.preview}
            className="image-fullscreen-img"
            loading="eager"
            decoding="async"
          />
        </div>
      </div>
    </div>
  );
}

function FilePreview({ item }: { item: ClipItem }) {
  const assets = item.fileAssets.length
    ? item.fileAssets
    : item.files.map((path) => ({
        originalPath: path,
        storedPath: null,
        sizeBytes: 0,
        isDirectory: false,
        status: 'skipped' as const,
        message: 'Original path only',
        thumbPath: null,
      }));
  return (
    <div className="preview-scroll file-preview">
      {assets.map((asset) => (
        <article className="file-card" key={asset.originalPath}>
          <span className="file-card-icon">
            {asset.thumbPath
              ? (
                <SafeImage
                  src={fileSrc(asset.thumbPath)}
                  alt={baseName(asset.originalPath)}
                  className="file-card-thumbnail"
                  fallback={
                    asset.isDirectory
                      ? <Folder size={28} strokeWidth={1.5} aria-hidden />
                      : <File size={28} strokeWidth={1.5} aria-hidden />
                  }
                />
              )
              : asset.isDirectory
                ? <Folder size={28} strokeWidth={1.5} aria-hidden />
                : <File size={28} strokeWidth={1.5} aria-hidden />}
          </span>
          <div className="file-card-copy">
            <strong>{baseName(asset.originalPath)}</strong>
            <span>{asset.storedPath ?? asset.originalPath}</span>
            <small className={`snapshot-status is-${asset.status}`}>
              <SnapshotStatusIcon status={asset.status} />
              {snapshotLabel(asset.status, asset.message)}
            </small>
          </div>
          <IconButton
            label="Show in File Explorer"
            onClick={() => void api.revealItem(asset.storedPath ?? asset.originalPath)}
          >
            <FolderOpen size={17} aria-hidden />
          </IconButton>
        </article>
      ))}
    </div>
  );
}

function LinkPreview({ item, onEdit }: { item: ClipItem; onEdit: () => void }) {
  const url = item.content || item.preview;
  const domain = safeDomain(url);
  return (
    <div className="preview-scroll link-preview">
      <article className="link-card">
        <div className="link-hero">
          <span className="link-mark"><Link2 size={34} aria-hidden /></span>
          <span>{domain}</span>
        </div>
        <button type="button" className="link-card-copy preview-edit-trigger" onClick={onEdit}>
          <strong>{domain || 'Web link'}</strong>
          <span>{url}</span>
        </button>
      </article>
      <button
        type="button"
        className="secondary-button"
        onClick={() => {
          const scheme = tryParseScheme(url);
          if (!scheme) {
            toast('That link is not a URL Clipmo can open.', 'error');
            return;
          }
          void api.openExternalUrl(normaliseUrl(url)).catch((error: unknown) => {
            toast(`The default browser could not be opened: ${String(error)}`, 'error');
          });
        }}
      >
        <ExternalLink size={16} aria-hidden /> Open in browser
      </button>
    </div>
  );
}

function EmailPreview({ item, onEdit }: { item: ClipItem; onEdit: () => void }) {
  const address = item.content || item.preview;
  return (
    <div className="preview-scroll email-preview">
      <span className="email-mark"><Mail size={34} strokeWidth={1.5} aria-hidden /></span>
      <button type="button" className="editable-preview" onClick={onEdit} title="Edit email address">
        {address}
      </button>
      <span>Email address</span>
    </div>
  );
}

function ColorPreview({ value, onEdit }: { value: string; onEdit: () => void }) {
  const rgb = hexToRgb(value);
  return (
    <div className="preview-scroll color-preview">
      <div
        className="color-preview-swatch"
        style={{ backgroundColor: value }}
        role="img"
        aria-label={`Color preview ${value}`}
      />
      <button type="button" className="editable-preview" onClick={onEdit} title="Edit color value">
        {value}
      </button>
      <span>{rgb}</span>
    </div>
  );
}

function EditItem({
  item,
  onSave,
  onCancel,
}: {
  item: ClipItem;
  onSave: (content: string) => Promise<void>;
  onCancel: () => void;
}) {
  const [value, setValue] = useState(item.content || item.preview);
  const [saving, setSaving] = useState(false);

  return (
    <form
      className="preview-editor"
      onSubmit={(event) => {
        event.preventDefault();
        if (!value.trim() || saving) return;
        setSaving(true);
        void onSave(value).finally(() => setSaving(false));
      }}
    >
      <header>
        <div>
          <strong>Edit clipboard item</strong>
          <span>Changes are saved locally and become the new copy value.</span>
        </div>
        <IconButton label="Cancel editing" onClick={onCancel}>
          <X size={18} aria-hidden />
        </IconButton>
      </header>
      <textarea
        autoFocus
        aria-label="Clipboard item content"
        spellCheck
        value={value}
        onChange={(event) => setValue(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === 'Escape') {
            event.preventDefault();
            event.stopPropagation();
            onCancel();
          } else if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
            event.preventDefault();
            event.currentTarget.form?.requestSubmit();
          } else if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 's') {
            event.preventDefault();
            event.currentTarget.form?.requestSubmit();
          }
        }}
      />
      <footer>
        <button type="button" className="secondary-button" onClick={onCancel}>Cancel</button>
        <button type="submit" className="primary-button" disabled={!value.trim() || saving}>
          <Save size={16} aria-hidden /> {saving ? 'Saving…' : 'Save item'}
        </button>
      </footer>
    </form>
  );
}

function SnapshotStatusIcon({ status }: { status: 'pending' | 'ready' | 'skipped' | 'failed' }) {
  if (status === 'pending') return <LoaderCircle size={13} className="spin" aria-hidden />;
  if (status === 'ready') return <CheckCircle2 size={13} aria-hidden />;
  if (status === 'skipped') return <CircleMinus size={13} aria-hidden />;
  return <TriangleAlert size={13} aria-hidden />;
}

function snapshotLabel(status: 'pending' | 'ready' | 'skipped' | 'failed', message: string | null) {
  if (status === 'pending') return 'Saving a managed snapshot…';
  if (status === 'ready') return 'Saved in Clipmo storage';
  return message ?? (status === 'failed' ? 'Snapshot failed' : 'Snapshot skipped');
}

function PreviewEmpty() {
  return (
    <div className="preview-empty">
      <span className="preview-empty-icon"><ClipboardCopy size={26} aria-hidden /></span>
      <strong>Select an item to preview</strong>
      <span>Use ↑ and ↓ to move through your clipboard history.</span>
    </div>
  );
}

function PreviewFailure({ title, message }: { title: string; message: string }) {
  return (
    <div className="preview-empty preview-failure">
      <span className="preview-empty-icon"><FileImage size={26} aria-hidden /></span>
      <strong>{title}</strong>
      <span>{message}</span>
    </div>
  );
}

function baseName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;
}

function safeDomain(value: string): string {
  try {
    return new URL(value).hostname.replace(/^www\./, '');
  } catch {
    return value;
  }
}

function hexToRgb(value: string): string {
  const match = /^#([\da-f]{3}|[\da-f]{6})$/i.exec(value);
  if (!match?.[1]) return value;
  const body = match[1].length === 3
    ? match[1].split('').map((part) => part + part).join('')
    : match[1];
  const number = Number.parseInt(body, 16);
  return `rgb(${number >> 16}, ${(number >> 8) & 255}, ${number & 255})`;
}

function MultiItemPreview({ items, selectedIds }: { items: ClipItem[]; selectedIds: number[] }) {
  const selectedItems = selectedIds
    .map((id) => items.find((item) => item.id === id) || {
      id,
      kind: 'text' as const,
      preview: `Item #${id}`,
      content: `Item #${id}`,
      hasHtml: false,
      hasRtf: false,
      image: null,
      files: [],
      fileAssets: [],
      sizeBytes: 0,
      tags: [],
      source: null,
      favorite: false,
      copyCount: 1,
      device: { id: 'local', name: 'This device', platform: 'windows' as const, color: '#000' },
      syncStatus: 'local' as const,
      firstCopiedAt: 0,
      lastCopiedAt: 0,
    });
  const totalBytes = selectedItems.reduce((acc, item) => acc + item.sizeBytes, 0);

  return (
    <div className="preview-scroll multi-preview" aria-label="Multiple items selected">
      <div className="multi-preview-header">
        <div className="multi-preview-badge">
          <Layers size={15} aria-hidden />
          <span>{selectedIds.length} items selected</span>
        </div>
        {totalBytes > 0 && <span className="multi-preview-size">{formatBytes(totalBytes)}</span>}
      </div>
      <div className="multi-preview-list" role="list">
        {selectedItems.map((item, index) => (
          <div key={item.id} className="multi-preview-card" role="listitem">
            <div className="multi-preview-card-header">
              <span className="multi-preview-card-index">#{index + 1}</span>
              <KindIcon item={item} size={14} />
              <span className="multi-preview-card-kind">{item.kind}</span>
              <div className="multi-preview-card-spacer" />
              <span className="multi-preview-card-time">{item.source?.name ?? 'Clip'}</span>
            </div>
            <div className="multi-preview-card-content">
              {item.preview || item.content}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
