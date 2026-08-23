// ** import types
import type { ChangeEvent, ReactNode } from 'react';

// ** import lib
import { invoke } from '@tauri-apps/api/core';
import { Check, FileImage, Files, LoaderCircle, Save, Type } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';

export type SyncFileMode = 'allowlist' | 'blocklist' | 'all';

export interface SyncPreferences {
  syncText: boolean;
  syncImages: boolean;
  syncFiles: boolean;
  syncFileMode: SyncFileMode;
  syncFileExtensions: string[];
  syncMaxFileSizeMb: number;
  syncMaxTotalSizeMb: number;
}

const SAFE_DEFAULTS: SyncPreferences = {
  syncText: true,
  syncImages: true,
  syncFiles: false,
  syncFileMode: 'blocklist',
  syncFileExtensions: [
    '.exe', '.bat', '.cmd', '.msi', '.scr', '.com', '.cpl', '.dll', '.sys', '.inf',
    '.vbs', '.js', '.jse', '.wsf', '.ps1', '.reg', '.mp4', '.mov', '.avi', '.mkv',
    '.webm', '.flv', '.wmv', '.iso', '.vhd', '.vhdx', '.img', '.dmg', '.zip',
    '.rar', '.7z', '.tar', '.gz',
  ],
  syncMaxFileSizeMb: 25,
  syncMaxTotalSizeMb: 100,
};

type SyncTab = 'content' | 'files';

export function SyncPreferencesPanel() {
  const [preferences, setPreferences] = useState<SyncPreferences | null>(null);
  const [tab, setTab] = useState<SyncTab>('content');
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    void invoke<SyncPreferences>('load_sync_preferences')
      .then((value) => {
        if (active) setPreferences(normalize(value));
      })
      .catch((reason: unknown) => {
        if (!active) return;
        setPreferences(SAFE_DEFAULTS);
        setError(errorMessage('Sync controls could not be loaded.', reason));
      });
    return () => { active = false; };
  }, []);

  const extensionText = useMemo(
    () => preferences?.syncFileExtensions.join(', ') ?? '',
    [preferences?.syncFileExtensions],
  );

  const update = <Key extends keyof SyncPreferences>(
    key: Key,
    value: SyncPreferences[Key],
  ) => {
    setSaved(false);
    setError(null);
    setPreferences((current) => current ? { ...current, [key]: value } : current);
  };

  const save = async () => {
    if (!preferences) return;
    setSaving(true);
    setSaved(false);
    setError(null);
    try {
      const next = await invoke<SyncPreferences>('save_sync_preferences', {
        preferences: normalize(preferences),
      });
      setPreferences(normalize(next));
      setSaved(true);
    } catch (reason) {
      setError(errorMessage('Sync controls could not be saved.', reason));
    } finally {
      setSaving(false);
    }
  };

  if (!preferences) {
    return (
      <div className="sync-preferences is-loading" role="status">
        <LoaderCircle className="is-spinning" size={17} aria-hidden /> Loading sync controls…
      </div>
    );
  }

  return (
    <div className="sync-preferences">
      <div className="sync-preferences-tabs" role="tablist" aria-label="Cross-device sync controls">
        <button
          type="button"
          role="tab"
          aria-selected={tab === 'content'}
          className={tab === 'content' ? 'is-active' : ''}
          onClick={() => setTab('content')}
        >
          Content
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={tab === 'files'}
          className={tab === 'files' ? 'is-active' : ''}
          onClick={() => setTab('files')}
        >
          File filters
        </button>
      </div>

      {tab === 'content' ? (
        <div className="sync-preferences-grid" role="tabpanel">
          <SyncToggle
            icon={<Type size={17} aria-hidden />}
            title="Text, links, email, and colors"
            detail="Enabled by default. Edits, favorites, and deletes also follow paired devices."
            checked={preferences.syncText}
            onChange={(value) => update('syncText', value)}
          />
          <SyncToggle
            icon={<FileImage size={17} aria-hidden />}
            title="Images"
            detail="Sync image and thumbnail bytes up to 8 MB per clipboard item."
            checked={preferences.syncImages}
            onChange={(value) => update('syncImages', value)}
          />
          <SyncToggle
            icon={<Files size={17} aria-hidden />}
            title="Files"
            detail="Disabled by default. Enable only for trusted devices on your local network."
            checked={preferences.syncFiles}
            onChange={(value) => update('syncFiles', value)}
          />
        </div>
      ) : (
        <div className="sync-file-controls" role="tabpanel">
          <label className="sync-field">
            <span>Extension policy</span>
            <select
              value={preferences.syncFileMode}
              onChange={(event) => update('syncFileMode', event.target.value as SyncFileMode)}
            >
              <option value="blocklist">Sync all except listed</option>
              <option value="allowlist">Sync only listed</option>
              <option value="all">Sync every extension</option>
            </select>
          </label>

          {preferences.syncFileMode !== 'all' && (
            <label className="sync-field is-wide">
              <span>{preferences.syncFileMode === 'allowlist' ? 'Allowed extensions' : 'Blocked extensions'}</span>
              <textarea
                rows={3}
                value={extensionText}
                placeholder=".txt, .pdf, .exe"
                onChange={(event) => update('syncFileExtensions', parseExtensions(event.target.value))}
              />
              <small>Separate extensions with commas. These are user controls, not permanent bans.</small>
            </label>
          )}

          <NumberField
            label="Maximum per file"
            value={preferences.syncMaxFileSizeMb}
            min={1}
            max={1024}
            onChange={(value) => update('syncMaxFileSizeMb', value)}
          />
          <NumberField
            label="Maximum queued batch"
            value={preferences.syncMaxTotalSizeMb}
            min={preferences.syncMaxFileSizeMb}
            max={4096}
            onChange={(value) => update('syncMaxTotalSizeMb', value)}
          />
        </div>
      )}

      <div className="sync-preferences-footer">
        <span className={error ? 'is-error' : ''} role="status" aria-live="polite">
          {error ?? (saved ? <><Check size={14} aria-hidden /> Sync controls saved</> : 'Changes apply after saving.')}
        </span>
        <button type="button" className="secondary-button" disabled={saving} onClick={() => void save()}>
          <Save size={15} aria-hidden /> {saving ? 'Saving…' : 'Save sync controls'}
        </button>
      </div>
    </div>
  );
}

function SyncToggle({ icon, title, detail, checked, onChange }: {
  icon: ReactNode;
  title: string;
  detail: string;
  checked: boolean;
  onChange: (value: boolean) => void;
}) {
  return (
    <label className="sync-content-toggle">
      <span className="sync-content-icon">{icon}</span>
      <span><strong>{title}</strong><small>{detail}</small></span>
      <input type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} />
    </label>
  );
}

function NumberField({ label, value, min, max, onChange }: {
  label: string;
  value: number;
  min: number;
  max: number;
  onChange: (value: number) => void;
}) {
  const change = (event: ChangeEvent<HTMLInputElement>) => {
    const next = Number.parseInt(event.target.value, 10);
    onChange(Number.isFinite(next) ? Math.max(min, Math.min(max, next)) : min);
  };
  return (
    <label className="sync-field">
      <span>{label}</span>
      <span className="sync-number"><input type="number" min={min} max={max} value={value} onChange={change} /><em>MB</em></span>
    </label>
  );
}

function parseExtensions(value: string): string[] {
  return [...new Set(value
    .split(/[,;\s]+/)
    .map((extension) => extension.trim().replace(/^\*?\.?/, '.').toLowerCase())
    .filter((extension) => extension.length > 1 && extension.length <= 32))]
    .sort();
}

function normalize(value: SyncPreferences): SyncPreferences {
  const maxFile = clamp(value.syncMaxFileSizeMb, 1, 1024);
  return {
    ...SAFE_DEFAULTS,
    ...value,
    syncFileExtensions: parseExtensions((value.syncFileExtensions ?? SAFE_DEFAULTS.syncFileExtensions).join(',')),
    syncMaxFileSizeMb: maxFile,
    syncMaxTotalSizeMb: clamp(value.syncMaxTotalSizeMb, maxFile, 4096),
  };
}

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, Number.isFinite(value) ? Math.round(value) : min));
}

function errorMessage(prefix: string, reason: unknown): string {
  if (reason instanceof Error && reason.message) return `${prefix} ${reason.message}`;
  if (typeof reason === 'string' && reason.trim()) return `${prefix} ${reason}`;
  return prefix;
}
