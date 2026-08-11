// ** import types
import type { ComponentType, SVGProps } from 'react';
import type { ItemKind, PlatformKind } from '../lib/types';

// ** import lib
import {
  Apple,
  File,
  FileText,
  Globe2,
  History,
  Image,
  Link,
  Mail,
  Monitor,
  PanelLeftClose,
  Palette,
  Smartphone,
  Star,
  Terminal,
} from 'lucide-react';

import { useStore } from '../lib/store';
import { resolvedFilterShortcuts } from '../lib/filter-shortcuts';

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

export function ClipboardSidebar() {
  const open = useStore((state) => state.sidebarOpen);
  const activeKinds = useStore((state) => state.activeKinds);
  const favoritesOnly = useStore((state) => state.favoritesOnly);
  const devices = useStore((state) => state.devices);
  const activeDeviceId = useStore((state) => state.activeDeviceId);
  const setCategory = useStore((state) => state.setCategory);
  const showFavorites = useStore((state) => state.showFavorites);
  const setDevice = useStore((state) => state.setDevice);
  const toggleSidebar = useStore((state) => state.toggleSidebar);
  const settings = useStore((state) => state.settings);
  const showDevices = devices.length > 1;
  const shortcuts = resolvedFilterShortcuts(settings?.filterShortcuts);

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

      {showDevices && (
        <div className="sidebar-devices" aria-label="Filter by source device">
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
          {devices.slice(0, 9).map((device, index) => {
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
              >
                <PlatformIcon size={16} aria-hidden />
                <span className="device-filter-dot" style={{ backgroundColor: device.color }} />
              </button>
            );
          })}
        </div>
      )}

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
    </nav>
  );
}

function platformIcon(platform: PlatformKind): SidebarIcon {
  if (platform === 'macos' || platform === 'ios') return Apple;
  if (platform === 'android') return Smartphone;
  if (platform === 'linux') return Terminal;
  return Monitor;
}
