// ** import types
import type { KeyboardEvent as ReactKeyboardEvent, ReactNode } from 'react';
import type { ApplicationInfo, Backdrop, FileFilterMode, IgnoredApp, ImageCompression, ImageFormat, ItemKind, Settings as SettingsType, ThemeMode } from './lib/types';

// ** import utils
import { formatBytes } from './lib/formatting';
import { shortcutFromKeyEvent, shortcutRecorderKeyAction } from './lib/global-shortcut';
import { mutationErrorMessage } from './lib/mutation-error';

// ** import lib
import { createContext, useContext, useEffect, useMemo, useRef, useState } from 'react';
import {
  Database,
  FileImage,
  Files,
  AppWindow,
  Archive,
  Check,
  CircleAlert,
  FolderOpen,
  HardDrive,
  Keyboard,
  Link2,
  Mail,
  Monitor,
  Palette,
  Plus,
  Search,
  Save,
  Settings2,
  Star,
  RefreshCw,
  Trash2,
  Type,
  Wifi,
  X,
} from 'lucide-react';

import { DeviceBadge } from './components/DeviceBadge';
import { PairDeviceDialog } from './components/PairDeviceDialog';
import { SyncPreferencesPanel } from './components/SyncPreferencesPanel';
import { useStore } from './lib/store';
import { api, fileSrc } from './lib/tauri';
import { getPlatform } from './lib/platform';
import { APP_SHORTCUTS, shortcutKeys } from './lib/shortcuts';
import { FILTER_SHORTCUTS, resolvedFilterShortcuts } from './lib/filter-shortcuts';
import { applyTheme } from './lib/theme';

const HISTORY_KINDS = [
  { key: 'text', kind: 'text', label: 'Text', icon: Type },
  { key: 'images', kind: 'image', label: 'Images', icon: FileImage },
  { key: 'files', kind: 'files', label: 'Files', icon: Files },
  { key: 'links', kind: 'link', label: 'Links', icon: Link2 },
  { key: 'colors', kind: 'color', label: 'Colors', icon: Palette },
  { key: 'emails', kind: 'email', label: 'Emails', icon: Mail },
] as const;

type SettingsCategory = 'appearance' | 'capture' | 'history' | 'sync' | 'shortcuts' | 'advanced';

const SETTINGS_CATEGORIES = [
  { id: 'appearance', label: 'Appearance', icon: Monitor },
  { id: 'capture', label: 'Capture', icon: Database },
  { id: 'history', label: 'History and storage', icon: HardDrive },
  { id: 'sync', label: 'Cross-device sync', icon: Wifi },
  { id: 'shortcuts', label: 'Keyboard shortcuts', icon: Keyboard },
  { id: 'advanced', label: 'Advanced', icon: Settings2 },
] as const;

export const SHORTCUT_RECORDER_DESCRIPTION =
  'Click the field, press the shortcut you want, then press Escape to finish recording.';

export default function Settings() {
  const settings = useStore((state) => state.settings);
  const saveSettings = useStore((state) => state.saveSettings);
  const setLaunchAtLogin = useStore((state) => state.setLaunchAtLogin);
  const setIgnoredApps = useStore((state) => state.setIgnoredApps);
  const appearance = useStore((state) => state.appearance);
  const counts = useStore((state) => state.counts);
  const sync = useStore((state) => state.sync);
  const clearHistory = useStore((state) => state.clearHistory);
  const clearCategory = useStore((state) => state.clearCategory);
  const changeStorageLocation = useStore((state) => state.changeStorageLocation);
  const regeneratePairingCode = useStore((state) => state.regeneratePairingCode);
  const [local, setLocal] = useState<SettingsType | null>(() => settings ? normaliseSettings(settings) : null);
  const dirtyRef = useRef(false);
  const [activeCategory, setActiveCategory] = useState<SettingsCategory>('appearance');
  const [saved, setSaved] = useState(false);
  const [saving, setSaving] = useState(false);
  const [storageBusy, setStorageBusy] = useState(false);
  const [syncNowBusy, setSyncNowBusy] = useState(false);
  const [pairDeviceOpen, setPairDeviceOpen] = useState(false);
  const [launchAtLoginBusy, setLaunchAtLoginBusy] = useState(false);
  const [ignoredAppsBusy, setIgnoredAppsBusy] = useState(false);
  const [mutationError, setMutationError] = useState<string | null>(null);

  useEffect(() => {
    if (settings && !dirtyRef.current) setLocal(normaliseSettings(settings));
  }, [settings]);

  useEffect(() => {
    document.title = 'Clipmo Settings';
    applyTheme(local?.theme ?? 'system', appearance);
    document.documentElement.dataset.backdrop = local?.backdrop ?? 'acrylic';
  }, [local?.theme, local?.backdrop, appearance]);

  useEffect(() => {
    const jumpToCategory = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey) || event.altKey || event.shiftKey) return;
      const index = Number(event.key) - 1;
      const category = SETTINGS_CATEGORIES[index];
      if (!category) return;
      event.preventDefault();
      setActiveCategory(category.id);
    };
    window.addEventListener('keydown', jumpToCategory);
    return () => window.removeEventListener('keydown', jumpToCategory);
  }, []);

  if (!local) return <SettingsLoading />;

  const update = <Key extends keyof SettingsType>(key: Key, value: SettingsType[Key]) => {
    dirtyRef.current = true;
    setSaved(false);
    setMutationError(null);
    setLocal({ ...local, [key]: value });
  };

  const persist = async () => {
    if (local.syncPairingCode.trim().length < 6) {
      setMutationError('Pairing code must be at least 6 digits/characters.');
      return;
    }
    setSaved(false);
    setMutationError(null);
    setSaving(true);
    try {
      const next = await saveSettings({
        ...local,
        fileIncludeExtensions: normaliseExtensions(local.fileIncludeExtensions.join(',')),
        fileExcludeExtensions: normaliseExtensions(local.fileExcludeExtensions.join(',')),
      });
      dirtyRef.current = false;
      setLocal(normaliseSettings(next));
      setSaved(true);
    } catch (error) {
      setMutationError(mutationErrorMessage('Settings could not be saved.', error));
    } finally {
      setSaving(false);
    }
  };

  const handleRegeneratePairingCode = async () => {
    setSaved(false);
    setMutationError(null);
    try {
      const next = await regeneratePairingCode();
      if (next?.syncPairingCode) {
        setLocal((prev) => (prev ? { ...prev, syncPairingCode: next.syncPairingCode } : prev));
      }
    } catch (error) {
      setMutationError(mutationErrorMessage('A new pairing code could not be created.', error));
    }
  };

  const configuredFilterShortcuts = resolvedFilterShortcuts(local.filterShortcuts);
  const shortcutConflict = findShortcutConflict([
    { label: 'Quick clipboard', shortcut: local.hotkey },
    { label: 'Open full Clipmo', shortcut: local.fullWindowHotkey },
    ...FILTER_SHORTCUTS.map((definition, index) => ({
      label: definition.label,
      shortcut: configuredFilterShortcuts[index] ?? definition.defaultShortcut,
    })),
  ]);

  const chooseStorage = async () => {
    setSaved(false);
    setMutationError(null);
    try {
      const selected = await api.chooseStorageFolder();
      if (typeof selected !== 'string') return;
      const approved = await api.confirm(
        'Clipmo will copy and verify managed content before switching locations. Original files are never moved or deleted.',
        'Change storage location',
      );
      if (!approved) return;
      setStorageBusy(true);
      const next = await changeStorageLocation(selected);
      setLocal(next);
      setSaved(true);
    } catch (error) {
      setMutationError(mutationErrorMessage('Storage location could not be changed.', error));
    } finally {
      setStorageBusy(false);
    }
  };

  const syncHistoryNow = async () => {
    setMutationError(null);
    setSyncNowBusy(true);
    try {
      await api.syncHistoryNow();
      setSaved(true);
    } catch (error) {
      setMutationError(mutationErrorMessage('Sync could not be started.', error));
    } finally {
      setSyncNowBusy(false);
    }
  };

  const updateLaunchAtLogin = async (enabled: boolean) => {
    setMutationError(null);
    setSaved(false);
    setLaunchAtLoginBusy(true);
    try {
      const next = await setLaunchAtLogin(enabled);
      setLocal((current) => current ? { ...current, launchAtLogin: next.launchAtLogin } : current);
      if (!dirtyRef.current) setSaved(true);
    } catch (error) {
      setMutationError(mutationErrorMessage('Launch at login could not be updated.', error));
    } finally {
      setLaunchAtLoginBusy(false);
    }
  };

  const updateIgnoredApps = async (ignoredApps: IgnoredApp[]) => {
    setMutationError(null);
    setSaved(false);
    setIgnoredAppsBusy(true);
    try {
      const next = await setIgnoredApps(ignoredApps);
      setLocal((current) => current ? { ...current, ignoredApps: next.ignoredApps } : current);
      if (!dirtyRef.current) setSaved(true);
    } catch (error) {
      setMutationError(mutationErrorMessage('Ignored applications could not be updated.', error));
    } finally {
      setIgnoredAppsBusy(false);
    }
  };

  const removeCategory = async (kind: ItemKind, label: string, count: number) => {
    if (count === 0) return;
    setMutationError(null);
    try {
      const approved = await api.confirm(
        `Delete ${count} non-favorite ${label.toLowerCase()} item${count === 1 ? '' : 's'}? Favorites will stay.`,
        `Clear ${label}`,
      );
      if (approved) await clearCategory(kind);
    } catch (error) {
      setMutationError(mutationErrorMessage(`${label} history could not be cleared.`, error));
    }
  };

  const removeHistory = async (includeFavorites: boolean) => {
    setMutationError(null);
    try {
      const approved = await api.confirm(
        includeFavorites
          ? 'Delete every history item, including favorites? This cannot be undone.'
          : 'Clear all non-favorite history items? Favorites will stay pinned.',
        includeFavorites ? 'Delete all history' : 'Clear history',
      );
      if (approved) await clearHistory(includeFavorites);
    } catch (error) {
      setMutationError(mutationErrorMessage('Clipboard history could not be cleared.', error));
    }
  };

  return (
    <div className="settings-shell">
      <header className="settings-header">
        <span className="settings-app-icon"><Settings2 size={21} aria-hidden /></span>
        <div>
          <h1>Clipmo settings</h1>
          <p>Appearance, capture, storage, and history controls</p>
        </div>
      </header>

      <div className="settings-body">
        <SettingsNav active={activeCategory} onChange={setActiveCategory} />
        <div className="settings-scroll" key={activeCategory}>
        {activeCategory === 'appearance' && (
        <Section title="Appearance" description="Match Windows or choose a fixed theme." icon={<Monitor size={18} />}>
          <Row id="theme" label="Theme" description="System is recommended and follows Windows automatically.">
            <Segmented<ThemeMode>
              value={local.theme}
              onChange={(value) => update('theme', value)}
              options={[
                { value: 'system', label: 'System' },
                { value: 'dark', label: 'Dark' },
                { value: 'light', label: 'Light' },
              ]}
            />
          </Row>
          <Row id="window-material" label="Windows glass style" description="Acrylic is the native Windows flyout look and the default; Mica is calmer, while Solid disables transparency.">
            <Segmented<Backdrop>
              value={local.backdrop}
              onChange={(value) => update('backdrop', value)}
              options={[
                { value: 'mica', label: 'Mica' },
                { value: 'acrylic', label: 'Acrylic' },
                { value: 'solid', label: 'Solid' },
              ]}
            />
          </Row>
          <Row id="show-preview" label="Show preview by default" description="Keep the history compact until you open the preview pane.">
            <Toggle checked={local.showPreview} onChange={(value) => update('showPreview', value)} />
          </Row>
        </Section>
        )}

        {activeCategory === 'capture' && (
        <Section title="Capture" description="Choose what Clipmo remembers locally." icon={<Database size={18} />}>
          <Row id="capture-images" label="Capture images" description="Save image bytes and fast thumbnails in Clipmo storage.">
            <Toggle checked={local.captureImages} onChange={(value) => update('captureImages', value)} />
          </Row>
          <Row id="image-format" label="Stored image format" description="PNG keeps exact pixels; JPEG is smaller for photos; WebP offers compact lossless storage.">
            <Segmented<ImageFormat>
              value={local.imageFormat}
              onChange={(value) => update('imageFormat', value)}
              options={[
                { value: 'original', label: 'As copied' },
                { value: 'png', label: 'PNG' },
                { value: 'jpeg', label: 'JPEG' },
                { value: 'webp', label: 'WebP' },
              ]}
            />
          </Row>
          <Row id="image-compression" label="Image compression" description="Normal is recommended. Best saves more space but takes longer; Manual exposes quality.">
            <Segmented<ImageCompression>
              value={local.imageCompression}
              onChange={(value) => update('imageCompression', value)}
              options={[
                { value: 'none', label: 'None' },
                { value: 'normal', label: 'Normal' },
                { value: 'best', label: 'Best' },
                { value: 'manual', label: 'Manual' },
              ]}
            />
          </Row>
          {local.imageCompression === 'manual' && (
            <Row id="image-quality" label="Image quality" description="Higher values preserve more detail and use more storage.">
              <NumberInput value={local.imageQuality} min={1} max={100} step={1} suffix="%" onChange={(value) => update('imageQuality', value)} />
            </Row>
          )}
          <Row id="capture-files" label="Capture files and folders" description="Keep durable local snapshots without blocking clipboard capture.">
            <Toggle checked={local.captureFiles} onChange={(value) => update('captureFiles', value)} />
          </Row>
          <Row id="file-filter-mode" label="File extension filter" description="Capture all files, only listed extensions, or everything except listed extensions. Folders remain available.">
            <Segmented<FileFilterMode>
              value={local.fileFilterMode}
              onChange={(value) => update('fileFilterMode', value)}
              options={[
                { value: 'all', label: 'All' },
                { value: 'include', label: 'Include' },
                { value: 'exclude', label: 'Exclude' },
              ]}
            />
          </Row>
          {local.fileFilterMode !== 'all' && (
            <Row id="file-filter-extensions" label={local.fileFilterMode === 'include' ? 'Included extensions' : 'Excluded extensions'} description="Separate extensions with commas, for example: .txt, .pdf, .exe">
              <ExtensionInput
                value={local.fileFilterMode === 'include' ? local.fileIncludeExtensions : local.fileExcludeExtensions}
                onChange={(value) => update(local.fileFilterMode === 'include' ? 'fileIncludeExtensions' : 'fileExcludeExtensions', value)}
              />
            </Row>
          )}
          <Row id="store-file-snapshots" label="Store file snapshots" description="Copy files into managed storage so history still works if the original changes.">
            <Toggle checked={local.storeFileSnapshots} onChange={(value) => update('storeFileSnapshots', value)} />
          </Row>
          <Row id="snapshot-limit" label="Maximum copied size" description="Files or folder groups above this total size stay out of managed snapshot storage.">
            <NumberInput
              value={local.maxSnapshotSizeMb}
              min={1}
              max={10_240}
              step={64}
              suffix="MB"
              onChange={(value) => update('maxSnapshotSizeMb', value)}
            />
          </Row>
          <Row id="maximum-history" label="Maximum history size" description="Favorites are not removed by normal retention cleanup.">
            <NumberInput
              value={local.maxItems}
              min={100}
              max={100_000}
              step={100}
              onChange={(value) => update('maxItems', value)}
            />
          </Row>
          <Row id="retention-days" label="Auto-delete after" description="Use 0 days to keep non-favorite entries indefinitely.">
            <NumberInput
              value={local.retentionDays}
              min={0}
              max={365}
              step={1}
              suffix="days"
              onChange={(value) => update('retentionDays', value)}
            />
          </Row>
          <Row id="paste-on-enter" label="Paste on Enter" description="Paste the selected item into the previously active app.">
            <Toggle checked={local.pasteOnEnter} onChange={(value) => update('pasteOnEnter', value)} />
          </Row>
        </Section>
        )}

        {activeCategory === 'history' && (
        <Section title="History and storage" icon={<Archive size={18} />}>
          <Row
            id="storage-location"
            label="Managed storage location"
            description="Choose where Clipmo keeps copied images and files."
          >
            <div className="storage-location-actions">
              <StorageLocationButton
                busy={storageBusy}
                path={local.storagePath}
                onClick={() => void chooseStorage()}
              />
              <button type="button" className="storage-open-button" title="Open Clipmo storage in File Explorer" aria-label="Open Clipmo storage in File Explorer" onClick={() => void api.openStorageFolder().catch((error: unknown) => setMutationError(mutationErrorMessage('Storage folder could not be opened.', error)))}>
                <FolderOpen size={16} aria-hidden />
              </button>
            </div>
          </Row>
          <div className="history-summary">
            <Metric label="All items" value={counts.total} icon={<Database size={17} />} />
            <Metric label="Favorites" value={counts.favorites} icon={<Star size={17} />} />
            <Metric label="Stored data" value={formatBytes(counts.storageBytes)} icon={<HardDrive size={17} />} />
          </div>
          <div className="kind-count-grid" aria-label="Clipboard items by type">
            {HISTORY_KINDS.map(({ key, kind, label, icon: KindGlyph }) => (
              <button
                type="button"
                className="kind-count"
                key={key}
                title={`Clear non-favorite ${label.toLowerCase()} items`}
                onClick={() => void removeCategory(kind, label, counts[key])}
              >
                <KindGlyph size={16} strokeWidth={1.7} aria-hidden />
                <span>{label}</span>
                <strong>{counts[key]}</strong>
                <Trash2 className="kind-clear-icon" size={14} aria-hidden />
              </button>
            ))}
          </div>
          <div className="history-actions">
            <button type="button" className="secondary-button" onClick={() => void removeHistory(false)}>
              <Trash2 size={16} aria-hidden /> Clear non-favorites
            </button>
            <button type="button" className="danger-button" onClick={() => void removeHistory(true)}>
              <Trash2 size={16} aria-hidden /> Delete all history
            </button>
          </div>
        </Section>
        )}

        {activeCategory === 'sync' && (
        <Section title="Cross-device sync" description="Pair trusted devices on the same local network." icon={<Wifi size={18} />}>
          <div className="history-actions sync-actions-header">
            <button type="button" className="primary-button" onClick={() => setPairDeviceOpen(true)}>
              <Plus size={16} aria-hidden /> Add / Pair device
            </button>
            <button type="button" className="secondary-button" disabled={!local.syncEnabled || syncNowBusy} onClick={() => void syncHistoryNow()}>
              <RefreshCw size={16} aria-hidden /> {syncNowBusy ? 'Syncing…' : 'Sync now'}
            </button>
          </div>
          <Row id="sync-enabled" label="LAN sync" description="Discover paired Clipmo devices and exchange text-like clipboard entries.">
            <Toggle checked={local.syncEnabled} onChange={(value) => update('syncEnabled', value)} />
          </Row>
          <Row id="sync-device-name" label="Device name" description="Shown beside history items copied from this device.">
            <TextInput
              value={local.syncDeviceName}
              onChange={(value) => update('syncDeviceName', value)}
            />
          </Row>
          <Row id="sync-device-color" label="Device color" description="Used as a quick visual identifier in the history list.">
            <ColorInput
              value={local.syncDeviceColor}
              onChange={(value) => update('syncDeviceColor', value)}
            />
          </Row>
          <Row id="sync-pairing-code" label="Pairing code" description="Devices with the same code on the same network can sync (6 digits).">
            <div className="pairing-code-field-group">
              <input
                type="text"
                className="setting-input pairing-code-text-input"
                value={local.syncPairingCode}
                maxLength={6}
                inputMode="numeric"
                placeholder="000000"
                onChange={(event) => {
                  const val = event.target.value.replace(/\D/g, '').slice(0, 6);
                  update('syncPairingCode', val);
                }}
                aria-label="Pairing code"
              />
              <button
                type="button"
                className="pairing-code-button"
                onClick={() => void handleRegeneratePairingCode()}
                title="Generate a new pairing code"
              >
                <RefreshCw size={15} aria-hidden />
                <span>Generate</span>
              </button>
            </div>
            {local.syncPairingCode.length > 0 && local.syncPairingCode.length < 6 && (
              <span className="setting-field-error">Pairing code must be at least 6 digits</span>
            )}
          </Row>
          <SyncPreferencesPanel />
          <div className="peer-list" aria-label="Discovered sync devices">
            {(sync?.peers.length ?? 0) === 0 ? (
              <span className="peer-empty">No paired devices discovered yet</span>
            ) : (
              sync?.peers.map((peer) => (
                <DeviceBadge key={peer.device.id} device={peer.device} status={peer.status} />
              ))
            )}
          </div>
        </Section>
        )}

        {activeCategory === 'shortcuts' && (
        <Section
          title="Keyboard shortcuts"
          description={`${getPlatform() === 'macos' ? 'macOS' : 'Windows'} key labels are used in this build.`}
          icon={<Keyboard size={18} />}
        >
          <Row
            id="global-hotkey"
            label="Quick clipboard"
            description={`Opens the compact clipboard flyout. ${SHORTCUT_RECORDER_DESCRIPTION}`}
          >
            <ShortcutRecorder value={local.hotkey} onChange={(value) => update('hotkey', value)} />
          </Row>
          <Row
            id="full-window-hotkey"
            label="Open full Clipmo"
            description={`Opens the full application window. ${SHORTCUT_RECORDER_DESCRIPTION}`}
          >
            <ShortcutRecorder
              value={local.fullWindowHotkey}
              onChange={(value) => update('fullWindowHotkey', value)}
            />
          </Row>
          {FILTER_SHORTCUTS.map((definition, index) => (
            <Row
              key={definition.target}
              id={`filter-shortcut-${definition.target}`}
              label={definition.label}
              description={`${definition.description} ${SHORTCUT_RECORDER_DESCRIPTION}`}
            >
              <ShortcutRecorder
                value={configuredFilterShortcuts[index] ?? definition.defaultShortcut}
                onChange={(value) => update(
                  'filterShortcuts',
                  configuredFilterShortcuts.map((shortcut, shortcutIndex) => (
                    shortcutIndex === index ? value : shortcut
                  )),
                )}
              />
            </Row>
          ))}
          {shortcutConflict && (
            <p className="settings-validation-error" role="alert">
              “{shortcutConflict[0]}” and “{shortcutConflict[1]}” cannot use the same shortcut.
            </p>
          )}
          <div className="shortcut-reference" aria-label="Keyboard shortcut reference">
            {APP_SHORTCUTS.map((shortcut) => (
              <div className="shortcut-reference-row" key={shortcut.id}>
                <div>
                  <strong>{shortcut.label}</strong>
                  <span>{shortcut.description}</span>
                </div>
                <span className="shortcut-keys">
                  {shortcutKeys(shortcut).map((key) => <kbd key={key}>{key}</kbd>)}
                </span>
              </div>
            ))}
          </div>
        </Section>
        )}

        {activeCategory === 'advanced' && (
          <Section title="Advanced" description="Startup and application-level behavior." icon={<Settings2 size={18} />}>
            <Row id="launch-at-login" label="Launch at login" description="Run quietly in the tray and monitor the clipboard after sign-in.">
              <Toggle checked={local.launchAtLogin} disabled={launchAtLoginBusy} onChange={(value) => void updateLaunchAtLogin(value)} />
            </Row>
            <Row id="ignored-apps" label="Ignored applications" description="Choose installed applications whose clipboard content Clipmo should never save.">
              <IgnoredApplications
                value={local.ignoredApps}
                busy={ignoredAppsBusy}
                onChange={(value) => void updateIgnoredApps(value)}
              />
            </Row>
          </Section>
        )}
        </div>
      </div>

      <footer className="settings-footer">
        <span
          className={`save-status ${saved || mutationError ? 'is-visible' : ''} ${mutationError ? 'is-error' : ''}`}
          role="status"
          aria-live="polite"
          aria-atomic="true"
        >
          {mutationError ?? (saved ? 'Settings saved' : '')}
        </span>
        <button
          type="button"
          className="primary-button"
          disabled={saving || Boolean(shortcutConflict)}
          onClick={() => void persist()}
        >
          <Save size={16} aria-hidden /> {saving ? 'Saving…' : 'Save changes'}
        </button>
      </footer>
      <PairDeviceDialog
        open={pairDeviceOpen}
        onClose={() => setPairDeviceOpen(false)}
        onSettingsUpdated={(updated) => {
          setLocal((prev) => (prev ? { ...prev, ...updated } : prev));
        }}
      />
    </div>
  );
}

function SettingsNav({
  active,
  onChange,
}: {
  active: SettingsCategory;
  onChange: (category: SettingsCategory) => void;
}) {
  return (
    <nav className="settings-nav" aria-label="Settings categories">
      {SETTINGS_CATEGORIES.map(({ id, label, icon: Icon }, index) => (
        <button
          key={id}
          type="button"
          className={active === id ? 'is-active' : ''}
          aria-current={active === id ? 'page' : undefined}
          onClick={() => onChange(id)}
        >
          <Icon size={17} aria-hidden />
          <span>{label}</span>
          <kbd>Ctrl+{index + 1}</kbd>
        </button>
      ))}
    </nav>
  );
}

function SettingsLoading() {
  return (
    <div className="settings-loading" role="status">
      Loading settings…
    </div>
  );
}

function Section({
  title,
  description,
  icon,
  children,
}: {
  title: string;
  description?: string;
  icon: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="settings-section">
      <header className="settings-section-header">
        <span>{icon}</span>
        <div>
          <h2>{title}</h2>
          {description && <p>{description}</p>}
        </div>
      </header>
      <div className="settings-section-body">{children}</div>
    </section>
  );
}

interface RowAria {
  labelledBy: string;
  describedBy: string;
}

const RowAriaContext = createContext<RowAria | null>(null);

function useRowAria(): RowAria {
  const aria = useContext(RowAriaContext);
  if (!aria) throw new Error('Settings controls must be rendered inside a Row');
  return aria;
}

export function Row({
  id,
  label,
  description,
  children,
}: {
  id: string;
  label: string;
  description: string;
  children: ReactNode;
}) {
  const aria = {
    labelledBy: `${id}-label`,
    describedBy: `${id}-description`,
  };

  return (
    <div className="settings-row">
      <div className="settings-row-copy">
        <strong id={aria.labelledBy}>{label}</strong>
        <span id={aria.describedBy}>{description}</span>
      </div>
      <RowAriaContext.Provider value={aria}>
        <div className="settings-row-control">{children}</div>
      </RowAriaContext.Provider>
    </div>
  );
}

function Metric({ label, value, icon }: { label: string; value: ReactNode; icon: ReactNode }) {
  return (
    <div className="history-metric">
      <span className="history-metric-icon">{icon}</span>
      <div><strong>{value}</strong><span>{label}</span></div>
    </div>
  );
}

function StorageLocationButton({
  busy,
  path,
  onClick,
}: {
  busy: boolean;
  path: string | null;
  onClick: () => void;
}) {
  const aria = useRowAria();
  return (
    <button
      type="button"
      className="storage-location-button"
      aria-labelledby={aria.labelledBy}
      aria-describedby={aria.describedBy}
      disabled={busy}
      onClick={onClick}
    >
      <FolderOpen size={16} aria-hidden />
      <span>{busy ? 'Moving…' : (path ?? 'Windows app data (default)')}</span>
    </button>
  );
}

export function Toggle({ checked, disabled = false, onChange }: { checked: boolean; disabled?: boolean; onChange: (value: boolean) => void }) {
  const aria = useRowAria();
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-labelledby={aria.labelledBy}
      aria-describedby={aria.describedBy}
      disabled={disabled}
      className={`toggle ${checked ? 'is-on' : ''}`}
      onClick={() => onChange(!checked)}
    >
      <span className="toggle-knob" />
    </button>
  );
}

export function Segmented<Value extends string>({
  value,
  onChange,
  options,
}: {
  value: Value;
  onChange: (value: Value) => void;
  options: { value: Value; label: string }[];
}) {
  const aria = useRowAria();
  return (
    <div
      className="segmented"
      role="radiogroup"
      aria-labelledby={aria.labelledBy}
      aria-describedby={aria.describedBy}
    >
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          role="radio"
          aria-checked={value === option.value}
          className={value === option.value ? 'is-active' : ''}
          onClick={() => onChange(option.value)}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

export function NumberInput({
  value,
  min,
  max,
  step,
  suffix,
  onChange,
}: {
  value: number;
  min: number;
  max: number;
  step: number;
  suffix?: string;
  onChange: (value: number) => void;
}) {
  const aria = useRowAria();
  return (
    <label className="number-field">
      <input
        type="number"
        aria-labelledby={aria.labelledBy}
        aria-describedby={aria.describedBy}
        value={value}
        min={min}
        max={max}
        step={step}
        onChange={(event) => {
          const next = Number(event.target.value);
          if (Number.isFinite(next)) onChange(Math.min(max, Math.max(min, next)));
        }}
      />
      {suffix && <span>{suffix}</span>}
    </label>
  );
}

function TextInput({ value, onChange }: { value: string; onChange: (value: string) => void }) {
  const aria = useRowAria();
  return (
    <input
      className="text-field"
      type="text"
      aria-labelledby={aria.labelledBy}
      aria-describedby={aria.describedBy}
      value={value}
      maxLength={64}
      onChange={(event) => onChange(event.target.value)}
    />
  );
}

export function ExtensionInput({ value, onChange }: { value: string[]; onChange: (value: string[]) => void }) {
  const aria = useRowAria();
  const [draft, setDraft] = useState(value.join(', '));
  const editing = useRef(false);

  useEffect(() => {
    if (!editing.current) setDraft(value.join(', '));
  }, [value]);

  const commit = () => {
    editing.current = false;
    const next = normaliseExtensions(draft);
    setDraft(next.join(', '));
    onChange(next);
  };

  return (
    <input
      className="text-field extension-field"
      type="text"
      aria-labelledby={aria.labelledBy}
      aria-describedby={aria.describedBy}
      value={draft}
      placeholder=".txt, .pdf"
      onFocus={() => { editing.current = true; }}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={commit}
      onKeyDown={(event) => {
        if (event.key === 'Enter') event.currentTarget.blur();
      }}
    />
  );
}

export function normaliseExtensions(value: string): string[] {
  return [...new Set(value.split(/[\s,;]+/).map((extension) => extension.trim().toLowerCase()).filter(Boolean).map((extension) => extension.startsWith('.') ? extension : `.${extension}`))];
}

function IgnoredApplications({ value, busy, onChange }: { value: IgnoredApp[]; busy: boolean; onChange: (value: IgnoredApp[]) => void }) {
  const aria = useRowAria();
  const [open, setOpen] = useState(false);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const close = () => {
    setOpen(false);
    requestAnimationFrame(() => triggerRef.current?.focus());
  };
  return (
    <div className="ignored-apps" aria-labelledby={aria.labelledBy} aria-describedby={aria.describedBy}>
      <div className="ignored-app-list" aria-label="Selected ignored applications">
        {value.map((app) => (
          <span className="ignored-app-chip" key={identityKey(app)} title={app.executablePath}>
            <ApplicationIcon app={app} />
            <span>{app.displayName}</span>
            <button type="button" disabled={busy} aria-label={`Remove ${app.displayName}`} onClick={() => onChange(value.filter((item) => identityKey(item) !== identityKey(app)))}>
              <X size={12} aria-hidden />
            </button>
          </span>
        ))}
      </div>
      <button ref={triggerRef} type="button" className="secondary-button" disabled={busy} onClick={() => setOpen(true)}><AppWindow size={15} aria-hidden /> {busy ? 'Updating…' : 'Choose applications'}</button>
      {open && <ApplicationPicker selected={value} onClose={close} onConfirm={(apps) => { onChange(apps); close(); }} />}
    </div>
  );
}

let applicationCache: ApplicationInfo[] | null = null;
let applicationRequest: Promise<ApplicationInfo[]> | null = null;

export function clearApplicationDiscoveryCache() {
  applicationCache = null;
  applicationRequest = null;
}

export function dedupeApplications(apps: ApplicationInfo[]): ApplicationInfo[] {
  const merged = new Map<string, ApplicationInfo>();
  for (const app of apps) {
    const key = identityKey(app);
    const existing = merged.get(key);
    merged.set(key, existing ? {
      ...existing,
      ...app,
      iconPath: app.iconPath ?? existing.iconPath,
      publisher: app.publisher ?? existing.publisher,
      running: existing.running || app.running,
      installed: existing.installed || app.installed,
      recentlyUsed: Boolean(existing.recentlyUsed || app.recentlyUsed),
    } : app);
  }
  return [...merged.values()].sort((a, b) =>
    Number(b.running) - Number(a.running)
    || Number(Boolean(b.recentlyUsed)) - Number(Boolean(a.recentlyUsed))
    || a.displayName.localeCompare(b.displayName, undefined, { sensitivity: 'base' }));
}

async function discoverApplications(refresh = false): Promise<ApplicationInfo[]> {
  if (refresh) {
    applicationCache = null;
    applicationRequest = null;
  }
  if (applicationCache) return applicationCache;
  if (!applicationRequest) {
    applicationRequest = Promise.all([api.listRunningApps(), api.listInstalledApps(refresh)])
      .then(([running, installed]) => {
        applicationCache = dedupeApplications([...running, ...installed]);
        return applicationCache;
      })
      .finally(() => { applicationRequest = null; });
  }
  return applicationRequest;
}

export function ApplicationPicker({ selected, onClose, onConfirm }: {
  selected: IgnoredApp[];
  onClose: () => void;
  onConfirm: (apps: IgnoredApp[]) => void;
}) {
  const [apps, setApps] = useState<ApplicationInfo[]>(applicationCache ?? []);
  const [chosen, setChosen] = useState<IgnoredApp[]>(selected);
  const [query, setQuery] = useState('');
  const [loading, setLoading] = useState(!applicationCache);
  const [error, setError] = useState<string | null>(null);
  const [activeIndex, setActiveIndex] = useState(0);
  const searchRef = useRef<HTMLInputElement>(null);
  const dialogRef = useRef<HTMLDivElement>(null);

  const load = async (refresh = false) => {
    setLoading(true);
    setError(null);
    try {
      setApps(await discoverApplications(refresh));
    } catch (loadError) {
      setError(mutationErrorMessage('Applications could not be loaded.', loadError));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void load();
    searchRef.current?.focus();
  }, []);

  const filtered = useMemo(() => {
    const needle = query.trim().toLocaleLowerCase();
    if (!needle) return apps;
    return apps.filter((app) => [app.displayName, app.publisher, app.executableName, app.executablePath]
      .some((part) => part?.toLocaleLowerCase().includes(needle)));
  }, [apps, query]);

  useEffect(() => {
    setActiveIndex((index) => Math.min(index, Math.max(0, filtered.length - 1)));
  }, [filtered.length]);

  const toggle = (app: IgnoredApp) => {
    const key = identityKey(app);
    setChosen((current) => current.some((item) => identityKey(item) === key)
      ? current.filter((item) => identityKey(item) !== key)
      : [...current, app]);
  };

  const browse = async () => {
    setError(null);
    try {
      const picked = await api.chooseApplications();
      const paths = typeof picked === 'string' ? [picked] : (picked ?? []);
      const identities = await Promise.all(paths.map((path) => api.resolveApplicationIdentity(path)));
      setChosen((current) => dedupeIgnored([...current, ...identities]));
    } catch (browseError) {
      setError(mutationErrorMessage('The application could not be added.', browseError));
    }
  };

  const handleKeys = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      onClose();
    } else if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const direction = event.key === 'ArrowDown' ? 1 : -1;
      setActiveIndex((index) => filtered.length ? (index + direction + filtered.length) % filtered.length : 0);
    } else if (event.key === 'Enter' && !(event.target instanceof HTMLButtonElement)) {
      event.preventDefault();
      onConfirm(chosen);
    } else if (event.key === 'Tab') {
      const focusable = [...(dialogRef.current?.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled)') ?? [])];
      const first = focusable[0];
      const last = focusable.at(-1);
      if (event.shiftKey && document.activeElement === first && last) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last && first) {
        event.preventDefault();
        first.focus();
      }
    }
  };

  return (
    <div className="application-picker-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <div ref={dialogRef} className="application-picker" role="dialog" aria-modal="true" aria-labelledby="application-picker-title" onKeyDown={handleKeys}>
        <header className="application-picker-header">
          <div><h2 id="application-picker-title">Choose applications</h2><p>Clipboard content copied from selected applications will not be saved.</p></div>
          <button type="button" className="picker-icon-button" aria-label="Close application picker" onClick={onClose}><X size={18} aria-hidden /></button>
        </header>
        <div className="application-picker-tools">
          <label className="application-search"><Search size={16} aria-hidden /><span className="sr-only">Search applications</span><input ref={searchRef} type="search" placeholder="Search applications" value={query} onChange={(event) => setQuery(event.target.value)} aria-controls="application-results" aria-activedescendant={filtered[activeIndex] ? `application-${safeId(identityKey(filtered[activeIndex]))}` : undefined} /></label>
          <button type="button" className="secondary-button" disabled={loading} onClick={() => void load(true)}><RefreshCw size={14} aria-hidden /> Refresh</button>
        </div>
        {chosen.length > 0 && <div className="picker-selection" aria-label={`${chosen.length} selected applications`}>
          {chosen.map((app) => <button type="button" key={identityKey(app)} className="selected-app-card" title={`Remove ${app.displayName}`} onClick={() => toggle(app)}><ApplicationIcon app={app} /><span>{app.displayName}</span><X size={12} aria-hidden /></button>)}
        </div>}
        <div id="application-results" className="application-results" role="listbox" aria-label="Applications" aria-multiselectable="true">
          {loading && apps.length === 0 && <PickerState icon={<RefreshCw className="is-spinning" size={22} />} title="Finding applications…" detail="Looking at installed and currently running applications." />}
          {!loading && error && apps.length === 0 && <PickerState icon={<CircleAlert size={22} />} title="Applications could not be loaded" detail={error}><button type="button" className="secondary-button" onClick={() => void load(true)}>Try again</button></PickerState>}
          {!loading && !error && filtered.length === 0 && <PickerState icon={<Search size={22} />} title={query ? 'No applications match your search' : 'No applications found'} detail={query ? 'Try a different name, publisher, or executable.' : 'Use Browse to choose an executable directly.'} />}
          {filtered.map((app, index) => {
            const checked = chosen.some((item) => identityKey(item) === identityKey(app));
            return <button id={`application-${safeId(identityKey(app))}`} key={identityKey(app)} type="button" role="option" aria-selected={checked} className={`application-option ${index === activeIndex ? 'is-active' : ''}`} onMouseEnter={() => setActiveIndex(index)} onClick={() => toggle(app)}>
              <ApplicationIcon app={app} />
              <span className="application-option-copy"><strong>{app.displayName}</strong><span>{[app.publisher, app.executablePath].filter(Boolean).join(' · ')}</span></span>
              <span className="application-statuses">{app.running && <span className="app-status is-running">Running</span>}{app.recentlyUsed && !app.running && <span className="app-status">Recent</span>}{app.installed && !app.running && <span className="app-status">Installed</span>}</span>
              <span className={`application-check ${checked ? 'is-checked' : ''}`}>{checked && <Check size={13} aria-hidden />}</span>
            </button>;
          })}
        </div>
        {error && apps.length > 0 && <p className="application-picker-error" role="status">{error}</p>}
        <footer className="application-picker-footer">
          <button type="button" className="secondary-button picker-browse" onClick={() => void browse()}><FolderOpen size={15} aria-hidden /> Browse for an executable</button>
          <span className="picker-selection-count">{chosen.length} selected</span>
          <button type="button" className="secondary-button" onClick={onClose}>Cancel</button>
          <button type="button" className="primary-button" onClick={() => onConfirm(chosen)}>Confirm selection</button>
        </footer>
      </div>
    </div>
  );
}

function PickerState({ icon, title, detail, children }: { icon: ReactNode; title: string; detail: string; children?: ReactNode }) {
  return <div className="picker-state">{icon}<strong>{title}</strong><span>{detail}</span>{children}</div>;
}

function ApplicationIcon({ app }: { app: Pick<IgnoredApp, 'displayName' | 'iconPath' | 'executablePath'> }) {
  return <span className="application-icon">{app.iconPath ? <img src={fileSrc(app.iconPath)} alt="" /> : <AppWindow size={17} aria-hidden />}<span className="sr-only">{app.displayName}</span></span>;
}

function identityKey(app: Pick<IgnoredApp, 'id' | 'packageFamilyName' | 'appUserModelId' | 'executablePath'>): string {
  return (app.packageFamilyName || app.appUserModelId || app.id || app.executablePath).toLocaleLowerCase();
}

function dedupeIgnored(apps: IgnoredApp[]): IgnoredApp[] {
  return [...new Map(apps.map((app) => [identityKey(app), app])).values()];
}

function safeId(value: string): string {
  return value.replace(/[^a-z0-9_-]/gi, '-');
}

function baseName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;
}

function normaliseSettings(settings: SettingsType): SettingsType {
  const legacyApps = settings.ignoredApps as unknown as Array<IgnoredApp | string>;
  return {
    ...settings,
    filterShortcuts: resolvedFilterShortcuts(settings.filterShortcuts),
    ignoredApps: dedupeIgnored(legacyApps.map((app) => typeof app === 'string' ? {
      id: app.toLocaleLowerCase(),
      displayName: baseName(app).replace(/\.exe$/i, ''),
      executablePath: app,
      executableName: baseName(app),
      appUserModelId: null,
      packageFamilyName: null,
      iconPath: null,
    } : app)),
  };
}

function findShortcutConflict(
  values: Array<{ label: string; shortcut: string }>,
): [string, string] | null {
  const seen = new Map<string, string>();
  for (const value of values) {
    const normalized = value.shortcut.trim().toLowerCase();
    const existing = seen.get(normalized);
    if (existing) return [value.label, existing];
    seen.set(normalized, value.label);
  }
  return null;
}

function ColorInput({ value, onChange }: { value: string; onChange: (value: string) => void }) {
  const aria = useRowAria();
  return (
    <label className="color-field">
      <input
        type="color"
        aria-labelledby={aria.labelledBy}
        aria-describedby={aria.describedBy}
        value={value}
        onChange={(event) => onChange(event.target.value)}
      />
      <span>{value}</span>
    </label>
  );
}

export function ShortcutRecorder({ value, onChange }: { value: string; onChange: (value: string) => void }) {
  const aria = useRowAria();
  const keys = value.split('+').filter(Boolean);
  return (
    <button
      type="button"
      className="shortcut-recorder"
      aria-labelledby={aria.labelledBy}
      aria-describedby={aria.describedBy}
      title="Press a shortcut, then press Escape to finish"
      onKeyDown={(event) => {
        const action = shortcutRecorderKeyAction(event.key);
        if (action === 'leave') return;
        event.preventDefault();
        event.stopPropagation();
        if (action === 'blur') {
          event.currentTarget.blur();
          return;
        }
        const shortcut = shortcutFromKeyEvent(
          event,
          getPlatform() === 'macos' ? 'Super' : 'Win',
        );
        if (shortcut) onChange(shortcut);
      }}
    >
      <span className="shortcut-keys">
        {keys.map((key) => <kbd key={key}>{key}</kbd>)}
      </span>
    </button>
  );
}
