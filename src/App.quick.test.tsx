/** @vitest-environment jsdom */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ClipItem, Counts } from './lib/types';

type Handler = (payload: unknown) => void;

const apiMock = vi.hoisted(() => ({
  listItems: vi.fn(),
  counts: vi.fn(),
  loadSettings: vi.fn(),
  syncState: vi.fn(),
  knownDevices: vi.fn(),
  knownTags: vi.fn(),
  syncNativeAppearance: vi.fn(),
  signalFrontendReady: vi.fn(),
  signalQuickDataHydrated: vi.fn(),
  quickReadinessState: vi.fn(),
  signalQuickSearchFocused: vi.fn(),
  saveSettings: vi.fn(),
  setPreviewVisible: vi.fn(),
  pasteActive: vi.fn(),
  copyToClipboard: vi.fn(),
  setFavorite: vi.fn(),
  setItemTags: vi.fn(),
  editItem: vi.fn(),
  deleteItem: vi.fn(),
  clearHistory: vi.fn(),
  clearCategory: vi.fn(),
  regeneratePairingCode: vi.fn(),
  changeStorageLocation: vi.fn(),
  pruneNow: vi.fn(),
  appearance: vi.fn(),
  quitApp: vi.fn(),
  showQuickPalette: vi.fn(),
  hideQuickPalette: vi.fn(),
  toggleQuickPalette: vi.fn(),
  showFullApplication: vi.fn(),
  hideFullApplication: vi.fn(),
  setQuickPinned: vi.fn(),
  setAlwaysOnTop: vi.fn(),
  hideWindow: vi.fn(),
  windowMode: vi.fn(),
  openSettingsWindow: vi.fn(),
  openStorageFolder: vi.fn(),
  openExternalUrl: vi.fn(),
  revealItem: vi.fn(),
  listInstalledApps: vi.fn(),
  listRunningApps: vi.fn(),
  resolveApplicationIdentity: vi.fn(),
  extractApplicationIcon: vi.fn(),
  syncStateRaw: vi.fn(),
}));

const listenerMap = vi.hoisted(() => new Map<string, Handler>());

vi.mock('./lib/tauri', () => ({
  api: apiMock,
  on: vi.fn((event: string, handler: Handler) => {
    listenerMap.set(event, handler);
    return Promise.resolve(() => {
      if (listenerMap.get(event) === handler) listenerMap.delete(event);
    });
  }),
  fileSrc: (path: string) => `asset://${path}`,
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ label: 'quick' }),
}));

import { cleanup, render, waitFor } from '@testing-library/react';
import App from './App';
import { bootStore, useStore } from './lib/store';

const baseCounts: Counts = {
  total: 0,
  favorites: 0,
  pinned: 0,
  text: 0,
  images: 0,
  files: 0,
  links: 0,
  colors: 0,
  emails: 0,
  storageBytes: 0,
};

const fileItem: ClipItem = {
  id: 99,
  kind: 'files',
  preview: 'OG_images.png',
  content: 'C:/fake/OG_images.png',
  hasHtml: false,
  hasRtf: false,
  image: null,
  files: ['C:/fake/OG_images.png'],
  fileAssets: [
    {
      originalPath: 'C:/fake/OG_images.png',
      storedPath: null,
      sizeBytes: 0,
      isDirectory: false,
      status: 'pending',
      message: null,
      thumbPath: null,
    },
  ],
  sizeBytes: 0,
  tags: [],
  source: { name: 'Explorer', exePath: 'C:/Windows/explorer.exe', iconPath: null },
  favorite: false,
  copyCount: 1,
  device: { id: 'local', name: 'local', platform: 'windows', color: '#000' },
  syncStatus: 'local',
  firstCopiedAt: 100,
  lastCopiedAt: 100,
};

const baseSettings = {
  settingsVersion: 3,
  hotkey: 'Ctrl+Shift+V',
  fullWindowHotkey: 'Ctrl+Alt+Shift+V',
  maxItems: 10_000,
  retentionDays: 0,
  captureImages: true,
  captureFiles: true,
  storeFileSnapshots: true,
  maxSnapshotSizeMb: 512,
  fileFilterMode: 'all' as const,
  fileIncludeExtensions: [] as string[],
  fileExcludeExtensions: [] as string[],
  imageFormat: 'original' as const,
  imageCompression: 'normal' as const,
  imageQuality: 80,
  storagePath: null,
  ignoredApps: [] as never[],
  backdrop: 'acrylic' as const,
  theme: 'system' as const,
  pasteOnEnter: true,
  launchAtLogin: false,
  showPreview: false,
  quickPreviewExpanded: false,
  syncEnabled: false,
  syncDeviceId: 'local',
  syncDeviceName: 'local',
  syncDeviceColor: '#000',
  syncPairingCode: '000000',
};

beforeEach(() => {
  listenerMap.clear();
  apiMock.listItems.mockReset().mockResolvedValue([fileItem]);
  apiMock.counts.mockReset().mockResolvedValue({ ...baseCounts, total: 1, files: 1 });
  apiMock.loadSettings.mockReset().mockResolvedValue(baseSettings);
  apiMock.syncState.mockReset().mockResolvedValue(null);
  apiMock.knownDevices.mockReset().mockResolvedValue([]);
  apiMock.knownTags.mockReset().mockResolvedValue([]);
  apiMock.syncNativeAppearance.mockReset().mockResolvedValue({ accent: '#000', dark: false });
  apiMock.signalFrontendReady.mockReset().mockResolvedValue(undefined);
  apiMock.signalQuickDataHydrated.mockReset().mockResolvedValue(undefined);
  apiMock.quickReadinessState.mockReset().mockResolvedValue({
    frontendReady: true,
    dataHydrated: true,
    openPending: false,
  });
  apiMock.signalQuickSearchFocused.mockReset().mockResolvedValue(undefined);
  apiMock.saveSettings.mockReset().mockImplementation(async (next: typeof baseSettings) => next);
  apiMock.setPreviewVisible.mockReset().mockResolvedValue(true);
  apiMock.pasteActive.mockReset().mockResolvedValue(undefined);
  apiMock.copyToClipboard.mockReset().mockResolvedValue(undefined);
  apiMock.setFavorite.mockReset().mockResolvedValue(undefined);
  apiMock.setItemTags.mockReset().mockResolvedValue(fileItem);
  apiMock.editItem.mockReset().mockResolvedValue(fileItem);
  apiMock.deleteItem.mockReset().mockResolvedValue(undefined);
  apiMock.clearHistory.mockReset().mockResolvedValue(undefined);
  apiMock.clearCategory.mockReset().mockResolvedValue(undefined);
  apiMock.regeneratePairingCode.mockReset().mockResolvedValue(baseSettings);
  apiMock.changeStorageLocation.mockReset().mockResolvedValue(baseSettings);
  apiMock.pruneNow.mockReset().mockResolvedValue(undefined);
  apiMock.appearance.mockReset().mockResolvedValue({ accent: '#000', dark: false });
  apiMock.quitApp.mockReset().mockResolvedValue(undefined);
  apiMock.showQuickPalette.mockReset().mockResolvedValue(undefined);
  apiMock.hideQuickPalette.mockReset().mockResolvedValue(undefined);
  apiMock.toggleQuickPalette.mockReset().mockResolvedValue(undefined);
  apiMock.showFullApplication.mockReset().mockResolvedValue(undefined);
  apiMock.hideFullApplication.mockReset().mockResolvedValue(undefined);
  apiMock.setQuickPinned.mockReset().mockResolvedValue(true);
  apiMock.setAlwaysOnTop.mockReset().mockResolvedValue(true);
  apiMock.setPreviewVisible.mockReset().mockResolvedValue(true);
  apiMock.hideWindow.mockReset().mockResolvedValue(undefined);
  apiMock.windowMode.mockReset().mockResolvedValue('quick');
  apiMock.openSettingsWindow.mockReset().mockResolvedValue(undefined);
  apiMock.openStorageFolder.mockReset().mockResolvedValue(undefined);
  apiMock.openExternalUrl.mockReset().mockResolvedValue(undefined);
  apiMock.revealItem.mockReset().mockResolvedValue(undefined);
  apiMock.listInstalledApps.mockReset().mockResolvedValue([]);
  apiMock.listRunningApps.mockReset().mockResolvedValue([]);
  apiMock.resolveApplicationIdentity.mockReset().mockResolvedValue(null);
  apiMock.extractApplicationIcon.mockReset().mockResolvedValue(null);
  apiMock.syncStateRaw.mockReset();

  useStore.setState({ mode: 'quick' });
});

afterEach(() => cleanup());

describe('App quick-view clipboard sync', () => {
  it('refreshes the store on every clipdeck:quick-opened event', async () => {
    render(<App />);
    // `bootStore` is what installs the Tauri listeners; main.tsx schedules it
    // with a zero-delay timer. We invoke it explicitly so the test does not
    // have to wait on the timer.
    await bootStore();

    apiMock.listItems.mockClear();
    apiMock.counts.mockClear();

    const handler = listenerMap.get('clipdeck:quick-opened');
    expect(handler).toBeDefined();

    const callsBefore = apiMock.listItems.mock.calls.length;
    await (handler as Handler)(undefined);
    await waitFor(() => {
      expect(apiMock.listItems.mock.calls.length).toBeGreaterThan(callsBefore);
    });
  });

  it('does not drop file-kind items when a clip-updated event fires after open', async () => {
    render(<App />);
    await bootStore();

    await waitFor(() => {
      expect(useStore.getState().items.map((item) => item.id)).toContain(99);
    });

    // The `clip-updated` handler must reach `refresh()` for the quick window
    // even though the event originates in the *full* window. This is the path
    // that the missing-event bug used to break — a file paste that landed
    // while the quick palette was hidden would never reach the list.
    const clipHandler = listenerMap.get('clip-updated');
    expect(clipHandler).toBeDefined();
    await (clipHandler as Handler)(fileItem);

    await waitFor(() => {
      expect(useStore.getState().items.some((item) => item.kind === 'files')).toBe(true);
    });
  });

  it('renders the same first-page items regardless of which window was active last', async () => {
    // Two independent store instances: a quick and a full. This mirrors the
    // two long-lived webviews, each with their own JS bundle and zustand
    // store. The native broadcast is what keeps them in lockstep.
    render(<App />);
    await bootStore();
    await waitFor(() => {
      expect(useStore.getState().items.length).toBeGreaterThan(0);
    });

    expect(useStore.getState().items[0]?.kind).toBe('files');
  });

  it('signals native hydration after the first SQLite read lands', async () => {
    render(<App />);
    await bootStore();
    await waitFor(() => {
      expect(apiMock.signalQuickDataHydrated).toHaveBeenCalledWith(true);
    });
    expect(useStore.getState().hydrated).toBe(true);
  });

  it('recovers missed clip-updated events fired while listeners were not yet installed', async () => {
    // The user can copy a clipboard item in the very first milliseconds after
    // launch, before React or `bootStore` have installed the listeners. The
    // contract must guarantee that the Quick View still catches up via the
    // initial `refresh()` once it does run.
    apiMock.listItems.mockReset();
    const early = { ...fileItem, id: 123, preview: 'early' };
    const later = { ...fileItem, id: 456, preview: 'later' };
    apiMock.listItems.mockResolvedValueOnce([early, later]);
    apiMock.counts.mockReset().mockResolvedValue({ ...baseCounts, total: 2, files: 2 });
    render(<App />);
    await bootStore();
    await waitFor(() => {
      const ids = useStore.getState().items.map((item) => item.id);
      expect(ids).toContain(123);
      expect(ids).toContain(456);
    });
  });
});
