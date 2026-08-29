// ** import types
import type { BulkFilterAction, ClipItem, CollectionSummary, Counts, DeviceIdentity, FilterScope, ItemKind, ListQuery, Settings, SourceApp, SyncState, SystemAppearance } from './types';
import type { TagColorMap } from './tag-color';

// ** import lib
import { create } from 'zustand';

import { HISTORY_PAGE_SIZE, mergeUniquePage, pageMayHaveMore } from './paging';
import { api, on } from './tauri';
import { loadTagColors, saveTagColors, tagColorKey } from './tag-color';
import { isDevBuild, resolveWindowMode, type WindowMode } from './window-mode';

interface State {
  items: ClipItem[];
  selectedId: number | null;
  selectedIds: number[];
  selectionAnchor: number | null;
  search: string;
  activeKinds: ItemKind[];
  favoritesOnly: boolean;
  devices: DeviceIdentity[];
  activeDeviceId: string | null;
  tags: string[];
  collections: CollectionSummary[];
  activeTag: string | null;
  tagColors: TagColorMap;
  sources: SourceApp[];
  activeSourceExe: string | null;
  sidebarOpen: boolean;
  counts: Counts;
  settings: Settings | null;
  sync: SyncState | null;
  appearance: SystemAppearance | null;
  /** Which native window this store instance belongs to. */
  mode: WindowMode;
  /**
   * Preview visibility for *this* window only.
   *
   * Quick and full each persist their own preference (`quickPreviewExpanded`
   * and `showPreview`). A single shared flag used to resize whichever window
   * happened to be mounted, so expanding the flyout also resized the desktop
   * application and clobbered its remembered width.
   */
  showPreview: boolean;
  showDetails: boolean;
  showCommands: boolean;
  /** Native listeners and initial requests have completed (successfully or not). */
  bootstrapped: boolean;
  /**
   * The first SQLite read for this webview has landed and the list reflects
   * the current history. The Quick View refuses to render "ready" content
   * until this flag is true; the full window also gates its first paint on it
   * to keep the two surfaces in lockstep.
   */
  hydrated: boolean;
  /** A recoverable startup error shown in the list instead of a blank surface. */
  bootError: string | null;
  loading: boolean;
  loadingMore: boolean;
  hasMore: boolean;
  nextOffset: number;
  /**
   * Monotonic id of the most recent user-driven resync request (opening
   * Quick View, becoming visible after being hidden, focus regained). Pairs
   * with the native `quick_open_pending` flag so the React store and the
   * native show path agree on the contract end-to-end.
   */
  resyncGeneration: number;
  /**
   * Optional override consumed by `refresh()` after a destructive action.
   * Stores the chosen successor id so the user lands on the same logical row
   * instead of having the selection jump to the top of the list.
   */
  pendingSelection: number | null;
}

interface Actions {
  refresh: (includeCounts?: boolean) => Promise<void>;
  loadMore: () => Promise<void>;
  requestResync: (source: ResyncSource) => Promise<void>;
  setSearch: (search: string) => Promise<void>;
  toggleKind: (kind: ItemKind) => Promise<void>;
  setCategory: (kind: ItemKind | null) => Promise<void>;
  showFavorites: () => Promise<void>;
  toggleFavoritesOnly: () => Promise<void>;
  loadKnownDevices: () => Promise<void>;
  setDevice: (deviceId: string | null) => Promise<void>;
  loadKnownTags: () => Promise<void>;
  loadCollections: () => Promise<void>;
  createCollection: (name: string) => Promise<void>;
  deleteCollection: (name: string) => Promise<void>;
  addSelectedToCollection: (name: string) => Promise<void>;
  removeSelectedFromCollection: (name: string) => Promise<void>;
  setSelectedFavorites: (value: boolean) => Promise<void>;
  setTag: (tag: string | null) => Promise<void>;
  setTagColor: (tag: string, color: string | null) => void;
  loadKnownSources: () => Promise<void>;
  setSource: (sourceExe: string | null) => Promise<void>;
  applyFilterAction: (scope: FilterScope, action: BulkFilterAction) => Promise<void>;
  toggleSidebar: () => void;
  select: (id: number | null) => void;
  selectOnly: (id: number) => void;
  selectToggle: (id: number) => void;
  selectRange: (id: number) => void;
  selectAll: () => void;
  toggleFavorite: (id: number) => Promise<void>;
  setItemTags: (id: number, tags: string[]) => Promise<void>;
  editItem: (id: number, content: string) => Promise<void>;
  deleteItem: (id: number) => Promise<void>;
  deleteSelected: () => Promise<void>;
  clearHistory: (includeFavorites: boolean) => Promise<void>;
  clearCategory: (kind: ItemKind, includeFavorites?: boolean) => Promise<void>;
  loadSettings: () => Promise<void>;
  loadSyncState: () => Promise<void>;
  saveSettings: (settings: Settings) => Promise<Settings>;
  setLaunchAtLogin: (enabled: boolean) => Promise<Settings>;
  setIgnoredApps: (ignoredApps: Settings['ignoredApps']) => Promise<Settings>;
  regeneratePairingCode: () => Promise<Settings>;
  changeStorageLocation: (path: string) => Promise<Settings>;
  setShowPreview: (show: boolean) => Promise<void>;
  setShowDetails: (show: boolean) => void;
  setShowCommands: (show: boolean) => void;
  applyAppearance: (appearance: SystemAppearance) => void;
}

/**
 * Why a resync was requested. The store and the diagnostics layer use this
 * to disambiguate the four real paths into a single `requestResync`:
 *
 * - `open`: emitted by the native `clipdeck:quick-opened` listener
 * - `visible`: window transitioned from hidden to visible
 * - `focus`: the webview regained focus while it was already visible
 * - `manual`: the user pressed F5 or another explicit refresh trigger
 */
export type ResyncSource = 'open' | 'visible' | 'focus' | 'manual';

let historyGeneration = 0;
let metadataRefreshTimer: ReturnType<typeof setTimeout> | null = null;

export const useStore = create<State & Actions>((set, get) => ({
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
  tags: [],
  collections: [],
  activeTag: null,
  tagColors: loadTagColors(),
  sources: [],
  activeSourceExe: null,
  sidebarOpen: false,
  counts: {
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
  },
  settings: null,
  sync: null,
  appearance: null,
  mode: resolveWindowMode(),
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

  refresh: async (includeCounts = true) => {
    // Each call bumps the shared `historyGeneration` so the *latest* call is
    // the only one that can commit results. Older fetches that resolve after a
    // newer one already started are silently dropped, so a slow SQLite read
    // started before a `clip-updated` event can never overwrite the newer
    // page that the event's own refresh will produce.
    const generation = ++historyGeneration;
    set({ loading: true, loadingMore: false });
    try {
      const query = buildQuery(get(), 0);
      const [page, counts] = await Promise.all([
        api.listItems(query),
        includeCounts ? api.counts() : Promise.resolve(get().counts),
      ]);
      if (generation !== historyGeneration) {
        if (isDevBuild()) {
          console.debug('[clipmo] refresh discarded as stale', {
            window: get().mode,
            discardedGeneration: generation,
            currentGeneration: historyGeneration,
            requestedLimit: query.limit,
            requestedOffset: query.offset,
          });
        }
        return;
      }
      const items = mergeUniquePage([], page);
      set((s) => {
        const override = s.pendingSelection;
        const fallback = items.some((i) => i.id === s.selectedId)
          ? s.selectedId
          : (items[0]?.id ?? null);
        const nextSelectedId =
          override !== null && items.some((i) => i.id === override)
            ? override
            : fallback;
        // Re-validate the multi-selection against the now-current items.
        // A filter change (search, kind, favorites) can leave selectedIds
        // pointing at rows that are no longer visible — drop them so the
        // range/preview won't lie about what the user has highlighted.
        const validIds = items.map((i) => i.id);
        const nextSelectedIds = s.selectedIds.filter((id) => validIds.includes(id));
        if (nextSelectedId !== null && !nextSelectedIds.includes(nextSelectedId)) {
          nextSelectedIds.unshift(nextSelectedId);
        }
        // Re-anchor: if the anchor itself was filtered out, fall back to
        // the first remaining selected id (or just the head of the list),
        // otherwise Shift+arrow would either collapse selection or pick
        // up a row that isn't visible.
        const anchorStillVisible = s.selectionAnchor !== null
          && validIds.includes(s.selectionAnchor);
        const nextAnchor = anchorStillVisible
          ? s.selectionAnchor
          : (nextSelectedIds[0] ?? nextSelectedId);
        if (isDevBuild()) {
          console.debug('[clipmo] refresh committed', {
            window: s.mode,
            generation,
            requestedQuery: query,
            committedIds: items.map((i) => i.id),
          });
        }
        return {
          items,
          counts,
          nextOffset: page.length,
          hasMore: pageMayHaveMore(page.length),
          selectedId: nextSelectedId,
          selectedIds: nextSelectedIds,
          selectionAnchor: nextSelectedIds.length ? nextAnchor : null,
          pendingSelection: null,
          hydrated: true,
        };
      });
    } finally {
      if (generation === historyGeneration) set({ loading: false });
    }
  },

  requestResync: async (source) => {
    // Each user-driven resync bumps `resyncGeneration`. The store drops any
    // older in-flight refresh by way of `historyGeneration` (set inside
    // `refresh()` itself), so the list can never regress to a state that
    // existed before the user-visible reason for the resync.
    set((state) => ({ resyncGeneration: state.resyncGeneration + 1 }));
    if (isDevBuild()) {
      console.debug('[clipmo] resync requested', {
        window: get().mode,
        source,
        resyncGeneration: get().resyncGeneration,
      });
    }
    await get().refresh();
  },

  loadMore: async () => {
    const current = get();
    if (current.loading || current.loadingMore || !current.hasMore) return;
    const generation = historyGeneration;
    const offset = current.nextOffset;
    set({ loadingMore: true });
    try {
      const page = await api.listItems(buildQuery(get(), offset));
      if (generation !== historyGeneration) return;
      set((state) => ({
        items: mergeUniquePage(state.items, page),
        nextOffset: offset + page.length,
        hasMore: pageMayHaveMore(page.length),
      }));
    } finally {
      if (generation === historyGeneration) set({ loadingMore: false });
    }
  },

  setSearch: async (search) => {
    set({ search });
    await get().refresh(false);
  },

  toggleKind: async (kind) => {
    const active = get().activeKinds.includes(kind)
      ? get().activeKinds.filter((k) => k !== kind)
      : [...get().activeKinds, kind];
    set({ activeKinds: active });
    await get().refresh(false);
  },

  setCategory: async (kind) => {
    set({ activeKinds: kind ? [kind] : [], favoritesOnly: false });
    await get().refresh(false);
  },

  showFavorites: async () => {
    set({ activeKinds: [], favoritesOnly: true });
    await get().refresh(false);
  },

  toggleFavoritesOnly: async () => {
    set({ favoritesOnly: !get().favoritesOnly });
    await get().refresh(false);
  },

  loadKnownDevices: async () => {
    const devices = await api.knownDevices();
    set((state) => ({
      devices,
      activeDeviceId: state.activeDeviceId !== null
        && devices.some((device) => device.id === state.activeDeviceId)
        ? state.activeDeviceId
        : null,
    }));
  },

  setDevice: async (deviceId) => {
    set({ activeDeviceId: deviceId });
    await get().refresh(false);
  },

  loadKnownTags: async () => {
    const tags = await api.knownTags();
    set((state) => ({
      tags,
      activeTag: state.activeTag !== null && tags.includes(state.activeTag)
        ? state.activeTag
        : null,
    }));
  },

  loadCollections: async () => set({ collections: await api.listCollections() }),

  createCollection: async (name) => {
    await api.createCollection(name);
    await Promise.all([get().loadCollections(), get().loadKnownTags()]);
  },

  deleteCollection: async (name) => {
    await api.deleteCollection(name);
    if (get().activeTag?.toLowerCase() === name.trim().toLowerCase()) set({ activeTag: null });
    await Promise.all([get().loadCollections(), get().loadKnownTags(), get().refresh(false)]);
  },

  addSelectedToCollection: async (name) => {
    const normalized = name.trim();
    if (!normalized) return;
    await Promise.all(get().selectedIds.map(async (id) => {
      const item = get().items.find((entry) => entry.id === id);
      if (item && !item.tags.some((tag) => tag.toLowerCase() === normalized.toLowerCase())) {
        await api.setItemTags(id, [...item.tags, normalized]);
      }
    }));
    await Promise.all([get().loadCollections(), get().loadKnownTags(), get().refresh(false)]);
  },

  removeSelectedFromCollection: async (name) => {
    const normalized = name.trim().toLowerCase();
    await Promise.all(get().selectedIds.map(async (id) => {
      const item = get().items.find((entry) => entry.id === id);
      if (item) await api.setItemTags(id, item.tags.filter((tag) => tag.toLowerCase() !== normalized));
    }));
    await Promise.all([get().loadCollections(), get().loadKnownTags(), get().refresh(false)]);
  },

  setSelectedFavorites: async (value) => {
    await Promise.all(get().selectedIds.map((id) => api.setFavorite(id, value)));
    await get().refresh();
  },

  setTag: async (tag) => {
    set({ activeTag: tag, activeKinds: [], favoritesOnly: false });
    await get().refresh(false);
  },

  setTagColor: (tag, color) => {
    set((state) => {
      const key = tagColorKey(tag);
      const tagColors = { ...state.tagColors };
      if (color && /^#[0-9a-f]{6}$/i.test(color)) tagColors[key] = color;
      else delete tagColors[key];
      saveTagColors(tagColors);
      return { tagColors };
    });
  },

  loadKnownSources: async () => {
    const sources = await api.knownSources();
    set((state) => ({
      sources,
      activeSourceExe: state.activeSourceExe !== null
        && sources.some((source) => source.exePath.toLowerCase() === state.activeSourceExe?.toLowerCase())
        ? state.activeSourceExe
        : null,
    }));
  },

  setSource: async (sourceExe) => {
    set({ activeSourceExe: sourceExe });
    await get().refresh(false);
  },

  applyFilterAction: async (scope, action) => {
    await api.applyFilterAction(scope, action);
    await Promise.all([
      get().refresh(),
      get().loadKnownTags(),
      get().loadCollections(),
      get().loadKnownDevices(),
      get().loadKnownSources(),
    ]);
  },

  toggleSidebar: () => set((state) => ({ sidebarOpen: !state.sidebarOpen })),

  select: (id) => {
    if (id === null) {
      set({ selectedId: null, selectedIds: [], selectionAnchor: null });
      return;
    }
    set({ selectedId: id, selectedIds: [id], selectionAnchor: id });
  },

  selectOnly: (id) => set({ selectedId: id, selectedIds: [id], selectionAnchor: id }),

  selectToggle: (id) => {
    const state = get();
    const isSelected = state.selectedIds.includes(id);
    const nextSelectedIds = isSelected
      ? state.selectedIds.filter((existing) => existing !== id)
      : [...state.selectedIds, id];
    // When toggling on, anchor follows the new item so a subsequent Shift+arrow
    // extends from it. When toggling off, keep the existing anchor if it still
    // points to a selected item — otherwise the just-deselected item would be
    // silently re-included in the next Shift+arrow range.
    const nextSelectedId = isSelected
      ? (nextSelectedIds[nextSelectedIds.length - 1] ?? null)
      : id;
    const anchorStillSelected = state.selectionAnchor !== null
      && nextSelectedIds.includes(state.selectionAnchor);
    const nextAnchor = isSelected
      ? (anchorStillSelected ? state.selectionAnchor : id)
      : id;
    set({
      selectedIds: nextSelectedIds,
      selectedId: nextSelectedId,
      selectionAnchor: nextAnchor,
    });
  },

  selectRange: (id) => {
    const state = get();
    const items = state.items;
    // If the anchor was filtered out between the last selection and
    // this click, fall back to the current focus or the clicked item
    // so the shift-range doesn't silently collapse to a single row.
    const anchorCandidate = state.selectionAnchor ?? state.selectedId ?? id;
    const fromIndex = items.findIndex((item) => item.id === anchorCandidate);
    const toIndex = items.findIndex((item) => item.id === id);
    if (fromIndex < 0 || toIndex < 0) {
      const fallback = items.find((item) => item.id === id)
        ? id
        : items[0]?.id ?? null;
      if (fallback === null) {
        set({ selectedId: null, selectedIds: [], selectionAnchor: null });
        return;
      }
      set({ selectedId: fallback, selectedIds: [fallback], selectionAnchor: fallback });
      return;
    }
    const [start, end] = fromIndex < toIndex ? [fromIndex, toIndex] : [toIndex, fromIndex];
    const rangeIds = items.slice(start, end + 1).map((item) => item.id);
    set({
      selectedIds: rangeIds,
      selectedId: id,
      selectionAnchor: anchorCandidate,
    });
  },

  selectAll: () => {
    // Empty list → empty selection. Never preserve a stale selection
    // when there is nothing to select.
    const ids = get().items.map((item) => item.id);
    set({
      selectedIds: ids,
      selectedId: ids[0] ?? null,
      selectionAnchor: ids[0] ?? null,
    });
  },

  toggleFavorite: async (id) => {
    const item = get().items.find((i) => i.id === id);
    if (!item) return;
    await api.setFavorite(id, !item.favorite);
    await get().refresh();
  },

  setItemTags: async (id, tags) => {
    await api.setItemTags(id, tags);
    await get().loadKnownTags();
    await get().refresh();
  },

  editItem: async (id, content) => {
    await api.editItem(id, content);
    await get().refresh();
  },

  deleteItem: async (id) => {
    const items = get().items;
    const index = items.findIndex((item) => item.id === id);
    // Prefer the row that will occupy the deleted position next; fall back
    // to the previous row if the deleted item was last, otherwise nothing.
    const successor = index >= 0 ? items[index + 1] ?? items[index - 1] ?? null : null;
    // Hold the successor in a closure so a concurrent `clip-updated` event
    // (which fires `refresh()` and clears `pendingSelection`) can't overwrite
    // the destination before our final refresh runs.
    const preserveSuccessor = () => set({ pendingSelection: successor?.id ?? null });
    preserveSuccessor();
    await api.deleteItem(id);
    await get().refresh();
    preserveSuccessor();
    await get().refresh();
  },

  deleteSelected: async () => {
    const ids = get().selectedIds;
    if (ids.length === 0) return;
    const items = get().items;
    const lastIndex = items.reduce(
      (max, item, currentIndex) => (ids.includes(item.id) ? currentIndex : max),
      -1,
    );
    const successor = lastIndex >= 0
      ? items
          .slice(lastIndex + 1)
          .find((item) => !ids.includes(item.id)) ?? items.slice(0, lastIndex).reverse().find((item) => !ids.includes(item.id)) ?? null
      : null;
    const preserveSuccessor = () => set({ pendingSelection: successor?.id ?? null });
    preserveSuccessor();
    const failed: number[] = [];
    for (const id of ids) {
      try {
        await api.deleteItem(id);
      } catch (error) {
        console.error('Failed to delete item', id, error);
        failed.push(id);
      }
    }
    if (failed.length > 0) {
      try {
        const { toast } = await import('./toast');
        toast(
          `Couldn't delete ${failed.length} item${failed.length === 1 ? '' : 's'} — see console.`,
          'error',
        );
      } catch {
        // Toast surface is optional — never let a UI affordance block a delete.
      }
    }
    await get().refresh();
    preserveSuccessor();
    await get().refresh();
  },

  clearHistory: async (includeFavorites) => {
    await api.clearHistory(includeFavorites);
    await get().refresh();
  },

  clearCategory: async (kind, includeFavorites = false) => {
    await api.clearCategory(kind, includeFavorites);
    await get().refresh();
  },

  loadSettings: async () => {
    const settings = await api.loadSettings();
    set({ settings, showPreview: previewFor(get().mode, settings) });
  },

  loadSyncState: async () => {
    const sync = await api.syncState();
    set({ sync });
  },

  saveSettings: async (settings) => {
    const next = await api.saveSettings(settings);
    set({ settings: next, showPreview: previewFor(get().mode, next) });
    await get().loadSyncState();
    return next;
  },

  setLaunchAtLogin: async (enabled) => {
    const next = await api.setLaunchAtLogin(enabled);
    set({ settings: next });
    return next;
  },

  setIgnoredApps: async (ignoredApps) => {
    const next = await api.setIgnoredApps(ignoredApps);
    set({ settings: next });
    return next;
  },

  regeneratePairingCode: async () => {
    const next = await api.regeneratePairingCode();
    set({ settings: next });
    await get().loadSyncState();
    return next;
  },

  changeStorageLocation: async (path) => {
    const next = await api.changeStorageLocation(path);
    set({ settings: next });
    await get().refresh();
    return next;
  },

  /** Applies and persists the preview preference belonging to this window. */
  setShowPreview: async (show) => {
    const state = get();
    const previous = state.showPreview;
    set({ showPreview: show });
    try {
      await api.setPreviewVisible(show);
      if (state.mode === 'full' && state.settings) {
        await get().saveSettings({ ...state.settings, showPreview: show });
      }
    } catch (error) {
      set({ showPreview: previous });
      console.error('Failed to apply the preview layout', error);
      try {
        const { toast } = await import('./toast');
        toast('The preview layout could not be changed.', 'error');
      } catch {
        // The toast surface is optional; state rollback is the important part.
      }
    }
  },

  setShowDetails: (show) => set({ showDetails: show }),
  setShowCommands: (show) => set({ showCommands: show }),

  applyAppearance: (appearance) => set({ appearance }),
}));

/** Picks the preview preference that belongs to a given window. */
export function previewFor(mode: WindowMode, settings: Settings): boolean {
  return mode === 'quick' ? settings.quickPreviewExpanded : settings.showPreview;
}

function buildQuery(s: State, offset: number): ListQuery {
  return {
    search: s.search.trim() || null,
    kinds: s.activeKinds,
    favoritesOnly: s.favoritesOnly,
    deviceIds: s.activeDeviceId ? [s.activeDeviceId] : [],
    tags: s.activeTag ? [s.activeTag] : [],
    sourceExes: s.activeSourceExe ? [s.activeSourceExe] : [],
    limit: HISTORY_PAGE_SIZE,
    offset,
  };
}

/** Boots event subscriptions. Call once from the root component. */
export async function bootStore() {
  const refresh = () =>
    useStore.getState().refresh().catch((error: unknown) => {
      console.error('Failed to refresh clipboard history', error);
    });

  const scheduleMetadataRefresh = () => {
    if (metadataRefreshTimer !== null) clearTimeout(metadataRefreshTimer);
    metadataRefreshTimer = setTimeout(() => {
      metadataRefreshTimer = null;
      void useStore.getState().loadKnownDevices();
      void useStore.getState().loadKnownTags();
      void useStore.getState().loadCollections();
      void useStore.getState().loadKnownSources();
    }, 350);
  };

  // Subscribe before the initial fetch so a clipboard update that lands while
  // either webview is starting cannot be missed. One failed startup request
  // must not disable all later real-time updates.
  const listenerResults = await Promise.allSettled([
    on<ClipItem>('clip-updated', () => {
      void refresh();
      scheduleMetadataRefresh();
    }),
    on<string>('clip-touched', () => void refresh()),
    on<Settings>('settings-updated', (settings) => {
      useStore.setState((state) => ({
        settings,
        showPreview: previewFor(state.mode, settings),
      }));
    }),
    on<void>('sync-peers-updated', () => {
      void useStore.getState().loadSyncState();
      void useStore.getState().loadKnownDevices();
    }),
    on<SystemAppearance>('appearance-changed', (appearance) => {
      useStore.getState().applyAppearance(appearance);
    }),
  ]);
  const listenerFailure = listenerResults.find(
    (result): result is PromiseRejectedResult => result.status === 'rejected',
  );
  // This is the runtime readiness boundary: React is mounted and native event
  // listeners are installed. Initial data may still be loading, which is why
  // ItemList has a visible loading state and the native show path waits for
  // the dedicated `hydrated` flag before revealing the Quick palette.
  useStore.setState({
    bootstrapped: true,
    bootError: listenerFailure ? String(listenerFailure.reason) : null,
  });

  const syncAppearance = async () => {
    try {
      const appearance = await api.syncNativeAppearance();
      useStore.getState().applyAppearance(appearance);
    } catch (error) {
      console.error('Failed to read system appearance', error);
    }
  };
  // The first refresh must finish before we tell the native side that the
  // Quick View is allowed to reveal itself. A failure is still reported as
  // hydrated (so the user sees the error surface rather than a blank window),
  // but `bootError` is set so the UI can render the recovery copy.
  const initialRefresh = useStore.getState().refresh();
  const loadResults = await Promise.allSettled([
    initialRefresh,
    useStore.getState().loadSettings(),
    useStore.getState().loadSyncState(),
    useStore.getState().loadKnownDevices(),
    useStore.getState().loadKnownTags(),
    useStore.getState().loadCollections(),
    useStore.getState().loadKnownSources(),
    syncAppearance(),
  ]);
  const loadFailure = loadResults.find(
    (result): result is PromiseRejectedResult => result.status === 'rejected',
  );
  useStore.setState((state) => ({
    bootError: state.bootError ?? (loadFailure ? String(loadFailure.reason) : null),
    loading: false,
    hydrated: true,
  }));
  // Tell the native side the Quick View can now be revealed if it has been
  // asked to open during the boot window. The command is a no-op on the full
  // window label, so the same boot path can be reused by both webviews.
  if (useStore.getState().mode === 'quick') {
    try {
      await api.signalQuickDataHydrated(true);
    } catch (error) {
      console.error('Failed to signal Quick View data hydration', error);
    }
  }
  window.addEventListener('focus', () => {
    void syncAppearance();
    // Refocusing the webview is a strong hint that the user came back to it
    // — the list may have been edited elsewhere in the meantime, so resync
    // before they re-engage. Coalesced by `requestResync` + `historyGeneration`.
    void useStore.getState().requestResync('focus').catch((error: unknown) => {
      console.error('Failed to resync on focus', error);
    });
  });
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'visible') {
      void syncAppearance();
      // Hidden → visible is the moment a Quick View that has been
      // backgrounded for a while needs to catch up. The native open event
      // would not fire on a normal reveal (the window was never closed), so
      // we trigger a resync here. `requestResync` is no-op-safe for the
      // open case because the open path also bumps the same generation.
      void useStore.getState().requestResync('visible').catch((error: unknown) => {
        console.error('Failed to resync on visibility change', error);
      });
    }
  });
}
