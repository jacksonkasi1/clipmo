/** @vitest-environment jsdom */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ClipItem, Counts, QuickReadinessState } from './types';

const apiMock = vi.hoisted(() => ({
  listItems: vi.fn(),
  counts: vi.fn(),
  setPreviewVisible: vi.fn(),
  saveSettings: vi.fn(),
  syncState: vi.fn(),
  signalQuickDataHydrated: vi.fn(),
  quickReadinessState: vi.fn(),
  syncNativeAppearance: vi.fn(),
  loadSettings: vi.fn(),
  knownDevices: vi.fn(),
}));

vi.mock('./tauri', () => ({
  api: apiMock,
  on: vi.fn(),
}));

vi.mock('./toast', () => ({
  toast: vi.fn(),
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ label: 'quick' }),
}));

import { useStore } from './store';

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

const baseReadiness: QuickReadinessState = {
  frontendReady: true,
  dataHydrated: true,
  openPending: false,
};

function makeItem(id: number, lastCopiedAt: number, kind: ClipItem['kind'] = 'text'): ClipItem {
  return {
    id,
    kind,
    preview: `entry ${id}`,
    content: `entry ${id}`,
    hasHtml: false,
    hasRtf: false,
    image: null,
    files: kind === 'files' ? [`C:/fake/path/${id}.png`] : [],
    fileAssets: kind === 'files'
      ? [{
        originalPath: `C:/fake/path/${id}.png`,
        storedPath: null,
        sizeBytes: 0,
        isDirectory: false,
        status: 'pending',
        message: null,
        thumbPath: null,
      }]
      : [],
    sizeBytes: 0,
    tags: [],
    source: null,
    favorite: false,
    copyCount: 1,
    device: { id: 'local', name: 'local', platform: 'windows', color: '#000' },
    syncStatus: 'local',
    firstCopiedAt: lastCopiedAt,
    lastCopiedAt,
  };
}

function resetStore() {
  useStore.setState({
    mode: 'quick',
    items: [],
    selectedId: null,
    selectedIds: [],
    selectionAnchor: null,
    pendingSelection: null,
    search: '',
    activeKinds: [],
    favoritesOnly: false,
    counts: baseCounts,
    settings: null,
    sync: null,
    appearance: null,
    showPreview: false,
    showDetails: true,
    showCommands: false,
    bootstrapped: false,
    hydrated: false,
    bootError: null,
    loading: true,
    loadingMore: false,
    hasMore: false,
    nextOffset: 0,
    resyncGeneration: 0,
  });
}

beforeEach(() => {
  apiMock.listItems.mockReset();
  apiMock.counts.mockReset().mockResolvedValue(baseCounts);
  apiMock.setPreviewVisible.mockReset().mockResolvedValue(true);
  apiMock.saveSettings.mockReset().mockImplementation(async (next) => next);
  apiMock.syncState.mockReset().mockResolvedValue(null);
  apiMock.knownDevices.mockReset().mockResolvedValue([]);
  apiMock.signalQuickDataHydrated.mockReset().mockResolvedValue(undefined);
  apiMock.quickReadinessState.mockReset().mockResolvedValue(baseReadiness);
  apiMock.syncNativeAppearance.mockReset().mockResolvedValue({ accent: '#000', dark: false });
  apiMock.loadSettings.mockReset().mockResolvedValue({
    settingsVersion: 3,
    hotkey: 'Ctrl+Shift+V',
    fullWindowHotkey: 'Ctrl+Alt+Shift+V',
    maxItems: 10_000,
    retentionDays: 0,
    captureImages: true,
    captureFiles: true,
    storeFileSnapshots: true,
    maxSnapshotSizeMb: 512,
    fileFilterMode: 'all',
    fileIncludeExtensions: [],
    fileExcludeExtensions: [],
    imageFormat: 'original',
    imageCompression: 'normal',
    imageQuality: 80,
    storagePath: null,
    ignoredApps: [],
    backdrop: 'acrylic',
    theme: 'system',
    pasteOnEnter: true,
    launchAtLogin: false,
    showPreview: false,
    quickPreviewExpanded: false,
    syncEnabled: false,
    syncDeviceId: 'local',
    syncDeviceName: 'local',
    syncDeviceColor: '#000',
    syncPairingCode: '000000',
  });
  resetStore();
});

describe('Quick View hydration lifecycle', () => {
  it('stays unhydrated until the first refresh commits', async () => {
    expect(useStore.getState().hydrated).toBe(false);
    apiMock.listItems.mockResolvedValueOnce([makeItem(1, 100)]);
    await useStore.getState().refresh();
    expect(useStore.getState().hydrated).toBe(true);
    expect(useStore.getState().items.map((item) => item.id)).toEqual([1]);
  });

  it('keeps the items array empty when a slow in-flight refresh is dropped', async () => {
    // A file was copied, triggering an immediate resync. The original
    // `bootStore` refresh is still pending and will resolve later. Only the
    // newer fetch may commit — the older one must be silently discarded.
    const stale: ClipItem[] = [makeItem(1, 100)];
    const fresh: ClipItem[] = [makeItem(2, 200), makeItem(1, 100)];

    let resolveStale: (items: ClipItem[]) => void = () => undefined;
    const staleFetch = new Promise<ClipItem[]>((resolve) => {
      resolveStale = resolve;
    });
    apiMock.listItems
      .mockImplementationOnce(() => staleFetch)
      .mockImplementationOnce(() => Promise.resolve(fresh));

    const firstRefresh = useStore.getState().refresh();
    const secondRefresh = useStore.getState().refresh();
    resolveStale(stale);
    await Promise.all([firstRefresh, secondRefresh]);

    const ids = useStore.getState().items.map((item) => item.id);
    expect(ids).toEqual([2, 1]);
    expect(useStore.getState().hydrated).toBe(true);
  });

  it('increments resyncGeneration for every requestResync call', async () => {
    apiMock.listItems.mockResolvedValue([makeItem(1, 100)]);
    const before = useStore.getState().resyncGeneration;
    await useStore.getState().requestResync('open');
    await useStore.getState().requestResync('visible');
    const after = useStore.getState().resyncGeneration;
    expect(after - before).toBe(2);
  });

  it('signals the native side when bootStore completes the first refresh', async () => {
    apiMock.listItems.mockResolvedValueOnce([makeItem(1, 100)]);
    const { bootStore } = await import('./store');
    await bootStore();
    expect(apiMock.signalQuickDataHydrated).toHaveBeenCalledWith(true);
    expect(useStore.getState().hydrated).toBe(true);
  });

  it('marks hydrated even when the first refresh fails so the UI can recover', async () => {
    apiMock.listItems.mockRejectedValueOnce(new Error('first read failed'));
    apiMock.counts.mockRejectedValueOnce(new Error('counts failed'));
    const { bootStore } = await import('./store');
    await bootStore();
    expect(useStore.getState().hydrated).toBe(true);
    expect(useStore.getState().bootError).toMatch(/first read failed/);
  });

  it('keeps resync coalescing: a duplicate request does not start a new fetch until the previous one settles', async () => {
    // Two requests fired in the same micro-task share a single SQLite read
    // because `historyGeneration` drops the older one before its result lands.
    let resolveFirst: (items: ClipItem[]) => void = () => undefined;
    apiMock.listItems
      .mockImplementationOnce(() => new Promise<ClipItem[]>((resolve) => {
        resolveFirst = resolve;
      }))
      .mockImplementationOnce(() => Promise.resolve([makeItem(7, 700)]));

    const first = useStore.getState().refresh();
    const second = useStore.getState().refresh();
    // Resolve the older fetch with a stale snapshot.
    resolveFirst([makeItem(1, 100)]);
    await Promise.all([first, second]);
    expect(useStore.getState().items.map((item) => item.id)).toEqual([7]);
  });

  it('does not regress the unfiltered list when search is applied then cleared', async () => {
    const full: ClipItem[] = [
      makeItem(10, 1000),
      makeItem(9, 900),
      makeItem(8, 800),
      makeItem(7, 700),
    ];
    apiMock.listItems.mockResolvedValueOnce(full);
    await useStore.getState().refresh();
    const beforeIds = useStore.getState().items.map((item) => item.id);
    expect(beforeIds).toEqual([10, 9, 8, 7]);

    apiMock.listItems.mockResolvedValueOnce([makeItem(10, 1000)]);
    await useStore.getState().setSearch('entry 10');
    expect(useStore.getState().search).toBe('entry 10');
    expect(useStore.getState().items.map((item) => item.id)).toEqual([10]);

    apiMock.listItems.mockResolvedValueOnce(full);
    await useStore.getState().setSearch('');
    const afterIds = useStore.getState().items.map((item) => item.id);
    expect(afterIds).toEqual(beforeIds);
  });
});
