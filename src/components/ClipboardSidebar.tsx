// ** import types
import type { ComponentType, SVGProps } from 'react';
import type { FilterScope, ItemKind, PlatformKind } from '../lib/types';

// ** import lib
import {
  Apple,
  AppWindow,
  File,
  FileText,
  Globe2,
  History,
  Image,
  Link,
  Mail,
  MoreHorizontal,
  Monitor,
  PanelLeftClose,
  Palette,
  Plus,
  Smartphone,
  Star,
  Tag,
  Terminal,
} from 'lucide-react';
import { useEffect, useState } from 'react';
import { createPortal } from 'react-dom';

import { useStore } from '../lib/store';
import { api, fileSrc } from '../lib/tauri';
import { resolvedFilterShortcuts } from '../lib/filter-shortcuts';
import { tagColorClass, tagColorIndex, tagColorKey } from '../lib/tag-color';

type SidebarIcon = ComponentType<SVGProps<SVGSVGElement> & { size?: number }>;

interface CategoryDefinition {
  id: 'all' | 'favorites' | ItemKind;
  label: string;
  icon: SidebarIcon;
}

const CATEGORIES: CategoryDefinition[] = [
  { id: 'all', label: 'All history', icon: History },
  { id: 'favorites', label: 'Favorites', icon: Star },
  { id: 'text', label: 'Text', icon: FileText },
  { id: 'image', label: 'Images', icon: Image },
  { id: 'link', label: 'Links', icon: Link },
  { id: 'files', label: 'Files', icon: File },
  { id: 'email', label: 'Email addresses', icon: Mail },
  { id: 'color', label: 'Colors', icon: Palette },
];

interface ClipboardSidebarProps {
  onAddDevice?: () => void;
  onExploreFilters?: () => void;
}

const MAX_RAIL_TAGS = 3;
const MAX_RAIL_DEVICES = 3;
const MAX_RAIL_SOURCES = 3;
const CONTEXT_MENU_WIDTH = 190;
const CONTEXT_MENU_HEIGHT = 196;
const CONTEXT_MENU_MARGIN = 8;

type ContextState = { x: number; y: number; scope: FilterScope; label: string } | null;

export function ClipboardSidebar({ onAddDevice, onExploreFilters }: ClipboardSidebarProps) {
  const open = useStore((state) => state.sidebarOpen);
  const activeKinds = useStore((state) => state.activeKinds);
  const favoritesOnly = useStore((state) => state.favoritesOnly);
  const devices = useStore((state) => state.devices);
  const activeDeviceId = useStore((state) => state.activeDeviceId);
  const tags = useStore((state) => state.tags);
  const activeTag = useStore((state) => state.activeTag);
  const tagColors = useStore((state) => state.tagColors);
  const sources = useStore((state) => state.sources);
  const activeSourceExe = useStore((state) => state.activeSourceExe);
  const setCategory = useStore((state) => state.setCategory);
  const showFavorites = useStore((state) => state.showFavorites);
  const setDevice = useStore((state) => state.setDevice);
  const setTag = useStore((state) => state.setTag);
  const setTagColor = useStore((state) => state.setTagColor);
  const setSource = useStore((state) => state.setSource);
  const applyFilterAction = useStore((state) => state.applyFilterAction);
  const toggleSidebar = useStore((state) => state.toggleSidebar);
  const settings = useStore((state) => state.settings);
  const showDevices = devices.length > 1;
  const hasOverflow = tags.length > MAX_RAIL_TAGS || devices.length > MAX_RAIL_DEVICES || sources.length > MAX_RAIL_SOURCES;
  const shortcuts = resolvedFilterShortcuts(settings?.filterShortcuts);
  const [context, setContext] = useState<ContextState>(null);

  useEffect(() => {
    if (!context) return;
    const close = () => setContext(null);
    window.addEventListener('pointerdown', close);
    window.addEventListener('blur', close);
    return () => {
      window.removeEventListener('pointerdown', close);
      window.removeEventListener('blur', close);
    };
  }, [context]);

  const openContext = (event: React.MouseEvent, scope: FilterScope, label: string) => {
    event.preventDefault();
    event.stopPropagation();
    const x = Math.max(CONTEXT_MENU_MARGIN, Math.min(
      event.clientX,
      window.innerWidth - CONTEXT_MENU_WIDTH - CONTEXT_MENU_MARGIN,
    ));
    const y = Math.max(CONTEXT_MENU_MARGIN, Math.min(
      event.clientY,
      window.innerHeight - CONTEXT_MENU_HEIGHT - CONTEXT_MENU_MARGIN,
    ));
    setContext({ x, y, scope, label });
  };

  const contextTagColor = context?.scope.kind === 'tag'
    ? tagColors[tagColorKey(context.scope.value)]
    : undefined;
  const contextTagPickerColor = context?.scope.kind === 'tag'
    ? contextTagColor ?? automaticTagColor(context.scope.value)
    : undefined;

  const runContextAction = async (action: 'favoriteAll' | 'deleteNonFavorites' | 'deleteAll') => {
    if (!context) return;
    const current = context;
    setContext(null);
    if (action !== 'favoriteAll') {
      const wording = action === 'deleteAll' ? 'all items' : 'all non-favorite items';
      const approved = await api.confirm(`Delete ${wording} in ${current.label}?`, 'Delete filtered history');
      if (!approved) return;
    }
    await applyFilterAction(current.scope, action);
  };

  const chooseCategory = (category: CategoryDefinition['id']) => {
    if (category === 'favorites') {
      if (!favoritesOnly) void showFavorites();
      return;
    }
    void setCategory(category === 'all' ? null : category);
  };

  return (
    <nav className="clipboard-sidebar" aria-label="Clipboard filters" aria-hidden={!open}>
      <div className="sidebar-actions">
        {CATEGORIES.map((category, index) => {
          const Icon = category.icon;
          const shortcut = shortcuts[index + 1];
          const active = category.id === 'favorites'
            ? favoritesOnly
            : !favoritesOnly && (
              category.id === 'all'
                ? activeKinds.length === 0
                : activeKinds.length === 1 && activeKinds[0] === category.id
            );
          return (
            <button
              key={category.id}
              type="button"
              className={`sidebar-button ${active ? 'is-active' : ''}`}
              title={`${category.label} (${shortcut})`}
              aria-label={`${category.label}, ${shortcut}`}
              aria-pressed={active}
              tabIndex={open ? 0 : -1}
              onClick={() => chooseCategory(category.id)}
            >
              <Icon size={17} aria-hidden />
            </button>
          );
        })}
      </div>

      <div className="sidebar-devices" aria-label="Devices">
        {onAddDevice && (
          <button
            type="button"
            className="sidebar-button"
            title="Pair another device"
            aria-label="Pair another device"
            tabIndex={open ? 0 : -1}
            onClick={onAddDevice}
          >
            <Plus size={18} aria-hidden />
          </button>
        )}
        {tags.slice(0, MAX_RAIL_TAGS).map((tag) => (
          <button
            key={tag}
            type="button"
            className={`sidebar-button sidebar-tag ${activeTag === tag ? 'is-active' : ''}`}
            title={`#${tag}`}
            aria-label={`Filter by tag ${tag}`}
            aria-pressed={activeTag === tag}
            tabIndex={open ? 0 : -1}
            onClick={() => void setTag(activeTag === tag ? null : tag)}
            onContextMenu={(event) => openContext(event, { kind: 'tag', value: tag }, `#${tag}`)}
          >
            <Tag
              className={tagColorClass(tag)}
              style={{ color: tagColors[tagColorKey(tag)] }}
              size={17}
              fill="currentColor"
              aria-hidden
            />
          </button>
        ))}
        {sources.slice(0, MAX_RAIL_SOURCES).map((source) => {
          const active = activeSourceExe?.toLowerCase() === source.exePath.toLowerCase();
          return (
            <button
              key={source.exePath}
              type="button"
              className={`sidebar-button source-filter ${active ? 'is-active' : ''}`}
              title={`From ${source.name}`}
              aria-label={`Filter by application ${source.name}`}
              aria-pressed={active}
              tabIndex={open ? 0 : -1}
              onClick={() => void setSource(active ? null : source.exePath)}
              onContextMenu={(event) => openContext(event, { kind: 'source', value: source.exePath }, source.name)}
            >
              {source.iconPath
                ? <img src={fileSrc(source.iconPath)} alt="" aria-hidden />
                : <AppWindow size={16} aria-hidden />}
            </button>
          );
        })}
        {showDevices && (
          <>
          <button
            type="button"
            className={`sidebar-button ${activeDeviceId === null ? 'is-active' : ''}`}
            title="All devices (Ctrl+0)"
            aria-label="All devices, Ctrl+0"
            aria-pressed={activeDeviceId === null}
            tabIndex={open ? 0 : -1}
            onClick={() => void setDevice(null)}
          >
            <Globe2 size={17} aria-hidden />
          </button>
          {devices.slice(0, MAX_RAIL_DEVICES).map((device, index) => {
            const PlatformIcon = platformIcon(device.platform);
            const active = activeDeviceId === device.id;
            return (
              <button
                key={device.id}
                type="button"
                className={`sidebar-button device-filter ${active ? 'is-active' : ''}`}
                title={`${device.name} (Ctrl+${index + 1})`}
                aria-label={`${device.name}, Ctrl+${index + 1}`}
                aria-pressed={active}
                tabIndex={open ? 0 : -1}
                onClick={() => void setDevice(device.id)}
                onContextMenu={(event) => openContext(event, { kind: 'device', value: device.id }, device.name)}
              >
                <PlatformIcon size={16} aria-hidden />
                <span className="device-filter-dot" style={{ backgroundColor: device.color }} />
              </button>
            );
          })}
          </>
        )}
        {hasOverflow && onExploreFilters && (
          <button
            type="button"
            className="sidebar-button"
            title="Explore all tags, applications, and devices"
            aria-label="Explore all tags, applications, and devices"
            tabIndex={open ? 0 : -1}
            onClick={onExploreFilters}
          >
            <MoreHorizontal size={18} aria-hidden />
          </button>
        )}
      </div>

      <button
        type="button"
        className="sidebar-button sidebar-collapse"
        title={`Close navigation (${shortcuts[0]})`}
        aria-label={`Close navigation, ${shortcuts[0]}`}
        tabIndex={open ? 0 : -1}
        onClick={toggleSidebar}
      >
        <PanelLeftClose size={17} aria-hidden />
      </button>
      {context && createPortal(
        <div
          className="sidebar-context-menu"
          role="menu"
          aria-label={`Actions for ${context.label}`}
          style={{ left: context.x, top: context.y }}
          onPointerDown={(event) => event.stopPropagation()}
        >
          <div className="sidebar-context-title">{context.label}</div>
          {context.scope.kind === 'tag' && (
            <div className="sidebar-context-color">
              <label htmlFor="sidebar-tag-color">Custom color</label>
              <input
                id="sidebar-tag-color"
                type="color"
                aria-label={`Custom color for ${context.label}`}
                value={contextTagPickerColor}
                onChange={(event) => setTagColor(context.scope.value, event.target.value)}
              />
              <button
                type="button"
                className="sidebar-color-reset"
                disabled={!contextTagColor}
                onClick={() => setTagColor(context.scope.value, null)}
              >
                Auto
              </button>
            </div>
          )}
          <button type="button" role="menuitem" onClick={() => void runContextAction('favoriteAll')}>Star all</button>
          <button type="button" role="menuitem" onClick={() => void runContextAction('deleteNonFavorites')}>Delete non-favorites</button>
          <button type="button" role="menuitem" className="is-danger" onClick={() => void runContextAction('deleteAll')}>Delete all</button>
        </div>,
        document.body,
      )}
    </nav>
  );
}

function platformIcon(platform: PlatformKind): SidebarIcon {
  if (platform === 'macos' || platform === 'ios') return Apple;
  if (platform === 'android') return Smartphone;
  if (platform === 'linux') return Terminal;
  return Monitor;
}

function automaticTagColor(tag: string): string {
  const value = getComputedStyle(document.documentElement)
    .getPropertyValue(`--tag-${tagColorIndex(tag)}`)
    .trim();
  return /^#[0-9a-f]{6}$/i.test(value) ? value : '#46a6ed';
}
