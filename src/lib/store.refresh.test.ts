/** @vitest-environment jsdom */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ClipItem, Counts, ListQuery } from './types';

const apiMock = vi.hoisted(() => ({
  listItems: vi.fn(),
  counts: vi.fn(),
  setPreviewVisible: vi.fn(),
  saveSettings: vi.fn(),
  syncState: vi.fn(),
}));

vi.mock('./tauri', () => ({
  api: apiMock,
  on: vi.fn(),
}));

vi.mock('./toast', () => ({
  toast: vi.fn(),
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ label: 'main' }),
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

function makeItem(id: number, lastCopiedAt: number, favorite = false, kind: ClipItem['kind'] = 'text'): ClipItem {
  return {
    id,
    kind,
    preview: `entry ${id}`,
    content: `entry ${id}`,
    hasHtml: false,
    hasRtf: false,
    image: null,
    files: kind === 'files' ? [`C:/fake/path/${id}.png`] : [],
    fileAssets: kind === 'files' ? [{
      originalPath: `C:/fake/path/${id}.png`,
      storedPath: null,
      sizeBytes: 0,
      isDirectory: false,
      status: 'pending',
      message: null,
    }] : [],
    sizeBytes: 0,
    tags: [],
    source: null,
    favorite,
    copyCount: 1,
    device: { id: 'local', name: 'local', platform: 'windows', color: '#000' },
    syncStatus: 'local',
    firstCopiedAt: lastCopiedAt,
    lastCopiedAt,
  };
}

describe('refresh race resolution', () => {
  beforeEach(() => {
    apiMock.listItems.mockReset();
    apiMock.counts.mockReset().mockResolvedValue(baseCounts);
    apiMock.setPreviewVisible.mockReset().mockResolvedValue(true);
    apiMock.saveSettings.mockReset().mockImplementation(async (next) => next);
    apiMock.syncState.mockReset().mockResolvedValue(null);
    useStore.setState({
      mode: 'full',
      items: [],
      selectedId: null,
      selectedIds: [],
      selectionAnchor: null,
      pendingSelection: null,
      search: '',
      activeKinds: [],
      favoritesOnly: false,
      devices: [],
      activeDeviceId: null,
      sidebarOpen: false,
      counts: baseCounts,
      settings: null,
      sync: null,
      appearance: null,
      showPreview: false,
      showDetails: true,
      showCommands: false,
      bootstrapped: false,
      bootError: null,
      loading: true,
      loadingMore: false,
      hasMore: false,
      nextOffset: 0,
    });
  });

  it('keeps the newer page when an older fetch resolves after it', async () => {
    // Both refreshes see the same query; the older one returns fewer rows
    // because it was triggered before a new file was copied, the newer one
    // sees the new file. The newer page must win.
    const older: ClipItem[] = [makeItem(1, 100)];
    const newer: ClipItem[] = [makeItem(2, 200), makeItem(1, 100)];

    let resolveOlder: (value: ClipItem[]) => void = () => undefined;
    let resolveNewer: (value: ClipItem[]) => void = () => undefined;
    const olderFetch = new Promise<ClipItem[]>((resolve) => {
      resolveOlder = resolve;
    });
    const newerFetch = new Promise<ClipItem[]>((resolve) => {
      resolveNewer = resolve;
    });

    apiMock.listItems
      .mockImplementationOnce(() => olderFetch)
      .mockImplementationOnce(() => newerFetch);

    // Fire the older refresh first; do not await.
    const olderPromise = useStore.getState().refresh();
    // Fire the newer refresh while the older one is in flight.
    const newerPromise = useStore.getState().refresh();

    // Resolve them in the wrong order: older second, newer first. The newer
    // page must still be the one committed.
    resolveNewer(newer);
    resolveOlder(older);

    await Promise.all([olderPromise, newerPromise]);

    const committed = useStore.getState().items.map((item) => item.id);
    expect(committed).toEqual([2, 1]);
  });

  it('drops a slower in-flight refresh triggered before the latest one', async () => {
    // Simulate a sequence: refresh A starts, then refresh B starts, then A
    // resolves with a partial page from before the file copy, then B resolves
    // with the full page. Only B's data should land in the store.
    const partialBeforeCopy: ClipItem[] = [makeItem(1, 100), makeItem(2, 50)];
    const fullAfterCopy: ClipItem[] = [makeItem(3, 300), makeItem(1, 100), makeItem(2, 50)];

    const resolvers: Array<(value: ClipItem[]) => void> = [];
    apiMock.listItems.mockImplementation(() => new Promise<ClipItem[]>((resolve) => {
      resolvers.push(resolve);
    }));

    const firstRefresh = useStore.getState().refresh();
    // Trigger the second refresh synchronously so the generation counter
    // advances past the first call's value.
    const secondRefresh = useStore.getState().refresh();

    // Resolve the first call with the older snapshot.
    const resolveFirst = resolvers[0];
    const resolveSecond = resolvers[1];
    if (!resolveFirst || !resolveSecond) throw new Error('expected two refresh resolvers');
    resolveFirst(partialBeforeCopy);
    // Resolve the second call with the newer snapshot.
    resolveSecond(fullAfterCopy);

    await Promise.all([firstRefresh, secondRefresh]);

    const items = useStore.getState().items.map((item) => item.id);
    expect(items).toEqual([3, 1, 2]);
  });

  it('keeps the loading flag off once the latest refresh settles', async () => {
    apiMock.listItems.mockImplementationOnce(() => Promise.resolve([makeItem(1, 100)]));
    apiMock.counts.mockResolvedValueOnce({ ...baseCounts, total: 1 });

    await useStore.getState().refresh();

    expect(useStore.getState().loading).toBe(false);
    expect(useStore.getState().items).toHaveLength(1);
  });

  it('reuses the same query shape that the page is built from', async () => {
    apiMock.listItems.mockResolvedValueOnce([]);
    apiMock.counts.mockResolvedValueOnce(baseCounts);

    useStore.setState({
      search: 'clip',
      activeKinds: ['files'],
      favoritesOnly: true,
      activeDeviceId: 'android-1',
      activeTag: 'work',
    });
    await useStore.getState().refresh();

    const firstCall = apiMock.listItems.mock.calls[0];
    if (!firstCall) throw new Error('expected listItems to be called');
    const query: ListQuery = firstCall[0];
    expect(query.search).toBe('clip');
    expect(query.kinds).toEqual(['files']);
    expect(query.favoritesOnly).toBe(true);
    expect(query.deviceIds).toEqual(['android-1']);
    expect(query.tags).toEqual(['work']);
  });
});
