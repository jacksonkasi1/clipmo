// ** import types
import type { ComponentType, SVGProps } from 'react';
import type { PlatformKind } from '../lib/types';

// ** import lib
import { useEffect } from 'react';
import { Apple, AppWindow, Globe2, Monitor, Smartphone, Tag, Terminal, X } from 'lucide-react';

import { useStore } from '../lib/store';
import { fileSrc } from '../lib/tauri';
import { tagColorClass, tagColorKey } from '../lib/tag-color';

type ExplorerIcon = ComponentType<SVGProps<SVGSVGElement> & { size?: number }>;

export function SidebarExplorerDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const tags = useStore((state) => state.tags);
  const activeTag = useStore((state) => state.activeTag);
  const tagColors = useStore((state) => state.tagColors);
  const devices = useStore((state) => state.devices);
  const activeDeviceId = useStore((state) => state.activeDeviceId);
  const setTag = useStore((state) => state.setTag);
  const setDevice = useStore((state) => state.setDevice);
  const sources = useStore((state) => state.sources);
  const activeSourceExe = useStore((state) => state.activeSourceExe);
  const setSource = useStore((state) => state.setSource);

  useEffect(() => {
    if (!open) return;
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', closeOnEscape);
    return () => window.removeEventListener('keydown', closeOnEscape);
  }, [onClose, open]);

  if (!open) return null;

  return (
    <div className="sidebar-explorer-backdrop" role="presentation" onMouseDown={(event) => {
      if (event.target === event.currentTarget) onClose();
    }}>
      <section className="sidebar-explorer" role="dialog" aria-modal="true" aria-labelledby="sidebar-explorer-title">
        <header>
          <div>
            <h2 id="sidebar-explorer-title">Explore filters</h2>
            <p>Find every tag, source application, and connected device.</p>
          </div>
          <button type="button" className="icon-button" aria-label="Close filter explorer" onClick={onClose}>
            <X size={16} aria-hidden />
          </button>
        </header>

        <div className="sidebar-explorer-content">
          <section aria-labelledby="explorer-tags-title">
            <h3 id="explorer-tags-title">Tags</h3>
            <div className="explorer-option-grid">
              <button
                type="button"
                className={`explorer-option ${activeTag === null ? 'is-active' : ''}`}
                onClick={() => { void setTag(null); onClose(); }}
              >
                <Globe2 size={16} aria-hidden /> All tags
              </button>
              {tags.map((tag) => (
                <button
                  key={tag}
                  type="button"
                  className={`explorer-option ${activeTag === tag ? 'is-active' : ''}`}
                  onClick={() => { void setTag(tag); onClose(); }}
                >
                  <Tag className={tagColorClass(tag)} style={{ color: tagColors[tagColorKey(tag)] }} size={16} fill="currentColor" aria-hidden /> #{tag}
                </button>
              ))}
            </div>
          </section>

          <section aria-labelledby="explorer-applications-title">
            <h3 id="explorer-applications-title">Applications</h3>
            <div className="explorer-option-grid">
              <button type="button" className={`explorer-option ${activeSourceExe === null ? 'is-active' : ''}`} onClick={() => { void setSource(null); onClose(); }}>
                <Globe2 size={16} aria-hidden /> All applications
              </button>
              {sources.map((source) => (
                <button key={source.exePath} type="button" className={`explorer-option ${activeSourceExe?.toLowerCase() === source.exePath.toLowerCase() ? 'is-active' : ''}`} onClick={() => { void setSource(source.exePath); onClose(); }}>
                  {source.iconPath ? <img className="explorer-app-icon" src={fileSrc(source.iconPath)} alt="" aria-hidden /> : <AppWindow size={16} aria-hidden />}
                  {source.name}
                </button>
              ))}
            </div>
          </section>

          <section aria-labelledby="explorer-devices-title">
            <h3 id="explorer-devices-title">Devices</h3>
            <div className="explorer-option-grid">
              <button
                type="button"
                className={`explorer-option ${activeDeviceId === null ? 'is-active' : ''}`}
                onClick={() => { void setDevice(null); onClose(); }}
              >
                <Globe2 size={16} aria-hidden /> All devices
              </button>
              {devices.map((device) => {
                const DeviceIcon = platformIcon(device.platform);
                return (
                  <button
                    key={device.id}
                    type="button"
                    className={`explorer-option ${activeDeviceId === device.id ? 'is-active' : ''}`}
                    onClick={() => { void setDevice(device.id); onClose(); }}
                  >
                    <DeviceIcon size={16} aria-hidden /> {device.name}
                    <span className="explorer-device-dot" style={{ backgroundColor: device.color }} />
                  </button>
                );
              })}
            </div>
          </section>
        </div>
      </section>
    </div>
  );
}

function platformIcon(platform: PlatformKind): ExplorerIcon {
  if (platform === 'macos' || platform === 'ios') return Apple;
  if (platform === 'android') return Smartphone;
  if (platform === 'linux') return Terminal;
  return Monitor;
}
