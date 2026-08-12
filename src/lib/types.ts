// TypeScript mirror of the Rust models exposed by `commands.rs`.
// Field names are camelCase on the wire; serde's `rename_all = "camelCase"`
// is configured globally in `error.rs` via the `Serialize` implementation.

export type ItemKind = 'text' | 'link' | 'email' | 'color' | 'image' | 'files';

export type PasteFlavor = 'original' | 'plainText';

export type Backdrop = 'acrylic' | 'mica' | 'solid';

export type FileFilterMode = 'all' | 'include' | 'exclude';
export type ImageFormat = 'original' | 'png' | 'jpeg' | 'webp';
export type ImageCompression = 'none' | 'normal' | 'best' | 'manual';

export type ThemeMode = 'system' | 'light' | 'dark';

export interface ImageMeta {
  path: string;
  thumbPath: string;
  width: number;
  height: number;
}

export type StoredFileStatus = 'pending' | 'ready' | 'skipped' | 'failed';

export interface StoredFile {
  originalPath: string;
  storedPath: string | null;
  sizeBytes: number;
  isDirectory: boolean;
  status: StoredFileStatus;
  message: string | null;
  /**
   * Absolute path to a downscaled PNG preview of the file. Only populated
   * when the original file is a raster image and the managed snapshot worker
   * was able to decode it. Used by the Quick View row and the details pane
   * to render a real thumbnail instead of a generic icon.
   */
  thumbPath?: string | null;
}

export interface SourceApp {
  name: string;
  exePath: string;
  iconPath: string | null;
}

export type PlatformKind = 'windows' | 'macos' | 'linux' | 'android' | 'ios' | 'unknown';

export type SyncStatus = 'local' | 'synced' | 'pending' | 'offline';

export interface DeviceIdentity {
  id: string;
  name: string;
  platform: PlatformKind;
  color: string;
}

export interface SyncPeer {
  device: DeviceIdentity;
  lastSeenAt: number;
  status: SyncStatus;
}

export interface SyncState {
  enabled: boolean;
  device: DeviceIdentity;
  pairingCode: string;
  peers: SyncPeer[];
}

export interface ClipItem {
  id: number;
  kind: ItemKind;
  preview: string;
  content: string;
  hasHtml: boolean;
  hasRtf: boolean;
  image: ImageMeta | null;
  files: string[];
  fileAssets: StoredFile[];
  sizeBytes: number;
  tags: string[];
  source: SourceApp | null;
  favorite: boolean;
  copyCount: number;
  device: DeviceIdentity;
  syncStatus: SyncStatus;
  firstCopiedAt: number;
  lastCopiedAt: number;
}

export interface ListQuery {
  search?: string | null;
  kinds?: ItemKind[];
  deviceIds?: string[];
  tags?: string[];
  favoritesOnly?: boolean;
  limit?: number;
  offset?: number;
}

export interface FlavorBundle {
  text: string | null;
  html: string | null;
  rtf: string | null;
  files: string[];
  image: ImageMeta | null;
}

export interface Counts {
  total: number;
  favorites: number;
  pinned: number;
  text: number;
  images: number;
  files: number;
  links: number;
  colors: number;
  emails: number;
  storageBytes: number;
}

export interface IgnoredApp {
  id: string;
  displayName: string;
  executablePath: string;
  executableName: string;
  appUserModelId?: string | null;
  packageFamilyName?: string | null;
  iconPath?: string | null;
}

export interface ApplicationInfo extends IgnoredApp {
  publisher?: string | null;
  running: boolean;
  installed: boolean;
  recentlyUsed?: boolean;
}

export interface Settings {
  settingsVersion: number;
  /** Accelerator that toggles the frameless quick clipboard palette. */
  hotkey: string;
  /** Accelerator that opens the full, decorated application window. */
  fullWindowHotkey: string;
  /** Navigation + category shortcuts, ordered by FILTER_SHORTCUTS. */
  filterShortcuts?: string[];
  maxItems: number;
  retentionDays: number;
  captureImages: boolean;
  captureFiles: boolean;
  storeFileSnapshots: boolean;
  maxSnapshotSizeMb: number;
  fileFilterMode: FileFilterMode;
  fileIncludeExtensions: string[];
  fileExcludeExtensions: string[];
  imageFormat: ImageFormat;
  imageCompression: ImageCompression;
  imageQuality: number;
  storagePath: string | null;
  ignoredApps: IgnoredApp[];
  backdrop: Backdrop;
  theme: ThemeMode;
  pasteOnEnter: boolean;
  launchAtLogin: boolean;
  /** Preview visibility for the full application window. */
  showPreview: boolean;
  /** Compact (false) vs expanded (true) layout for the quick palette. */
  quickPreviewExpanded: boolean;
  syncEnabled: boolean;
  syncDeviceId: string;
  syncDeviceName: string;
  syncDeviceColor: string;
  syncPairingCode: string;
}

export interface SystemAppearance {
  accent: string;
  dark: boolean;
}

/**
 * Snapshot of the Quick palette's native readiness state. Both flags must
 * be true before the Quick View is allowed to reveal itself, so the user
 * never sees an empty list while the first SQLite read is still in flight.
 */
export interface QuickReadinessState {
  frontendReady: boolean;
  dataHydrated: boolean;
  openPending: boolean;
}
