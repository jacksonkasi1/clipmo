/** @vitest-environment jsdom */
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ApplicationInfo, Settings as SettingsType } from './lib/types';

const apiMock = vi.hoisted(() => ({
  listInstalledApps: vi.fn(),
  listRunningApps: vi.fn(),
  chooseApplications: vi.fn(),
  resolveApplicationIdentity: vi.fn(),
  extractApplicationIcon: vi.fn(),
  chooseStorageFolder: vi.fn(),
  confirm: vi.fn(),
  openStorageFolder: vi.fn(),
  setLaunchAtLogin: vi.fn(),
}));

vi.mock('./lib/tauri', () => ({ api: apiMock, fileSrc: (path: string) => `asset://${path}` }));

import Settings, {
  ApplicationPicker,
  clearApplicationDiscoveryCache,
  dedupeApplications,
  ExtensionInput,
  Row,
} from './Settings';
import { useStore } from './lib/store';

const baseSettings: SettingsType = {
  settingsVersion: 2,
  hotkey: 'Ctrl+Shift+V',
  fullWindowHotkey: 'Ctrl+Alt+Shift+V',
  maxItems: 1000,
  retentionDays: 30,
  captureImages: true,
  captureFiles: true,
  storeFileSnapshots: true,
  maxSnapshotSizeMb: 512,
  fileFilterMode: 'include',
  fileIncludeExtensions: ['.txt'],
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
  showPreview: true,
  quickPreviewExpanded: false,
  syncEnabled: false,
  syncDeviceId: 'device',
  syncDeviceName: 'Desktop',
  syncDeviceColor: '#39b9e8',
  syncPairingCode: 'ABC123',
};

const runningApp: ApplicationInfo = {
  id: 'notepad', displayName: 'Notepad', executablePath: 'C:\\Windows\\notepad.exe', executableName: 'notepad.exe',
  publisher: 'Microsoft', running: true, installed: true, recentlyUsed: true, iconPath: 'C:\\icons\\notepad.png',
};
const installedApp: ApplicationInfo = {
  id: 'paint', displayName: 'Paint', executablePath: 'C:\\Windows\\paint.exe', executableName: 'paint.exe',
  publisher: 'Microsoft', running: false, installed: true, recentlyUsed: false,
};

beforeEach(() => {
  clearApplicationDiscoveryCache();
  apiMock.listRunningApps.mockReset().mockResolvedValue([runningApp]);
  apiMock.listInstalledApps.mockReset().mockResolvedValue([installedApp, { ...runningApp, running: false }]);
  apiMock.chooseApplications.mockReset().mockResolvedValue(null);
  apiMock.resolveApplicationIdentity.mockReset();
  apiMock.extractApplicationIcon.mockReset().mockResolvedValue(null);
  apiMock.setLaunchAtLogin.mockReset().mockImplementation(async (enabled: boolean) => ({
    ...baseSettings,
    launchAtLogin: enabled,
  }));
  useStore.setState({
    settings: baseSettings,
    appearance: { accent: '#39b9e8', dark: true },
    sync: null,
    counts: { total: 23, favorites: 4, pinned: 0, text: 10, images: 3, files: 2, links: 5, colors: 1, emails: 2, storageBytes: 2048 },
  });
});

afterEach(() => cleanup());

describe('Settings interactions', () => {
  it('keeps an extension draft raw until blur, then normalises it', async () => {
    const changed = vi.fn();
    const user = userEvent.setup();
    render(<Row id="extensions" label="Extensions" description="A list"><ExtensionInput value={['.txt']} onChange={changed} /></Row>);
    const input = screen.getByRole('textbox');
    await user.clear(input);
    await user.type(input, 'TXT, .Pdf; exe txt');
    expect((input as HTMLInputElement).value).toBe('TXT, .Pdf; exe txt');
    expect(changed).not.toHaveBeenCalled();
    await user.tab();
    expect(changed).toHaveBeenLastCalledWith(['.txt', '.pdf', '.exe']);
    expect((input as HTMLInputElement).value).toBe('.txt, .pdf, .exe');
  });

  it('adopts the authoritative settings returned by save', async () => {
    const user = userEvent.setup();
    const saveSettings = vi.fn().mockResolvedValue({ ...baseSettings, theme: 'dark' });
    useStore.setState({ saveSettings });
    render(<Settings />);
    await user.click(screen.getByRole('radio', { name: 'Light' }));
    await user.click(screen.getByRole('button', { name: /Save changes/ }));
    await waitFor(() => expect(saveSettings).toHaveBeenCalled());
    await waitFor(() => expect(screen.getByRole('radio', { name: 'Dark' }).getAttribute('aria-checked')).toBe('true'));
    expect(screen.getByRole('status').textContent).toContain('Settings saved');
  });

  it('uses the exact compact history naming and metric layout', async () => {
    const user = userEvent.setup();
    render(<Settings />);
    await user.click(screen.getByRole('button', { name: /History and storage/ }));
    expect(screen.getByRole('heading', { name: 'History and storage' })).toBeTruthy();
    const metrics = document.querySelector('.history-summary');
    expect(metrics?.children).toHaveLength(3);
    expect(metrics?.textContent).toContain('All items');
    expect(screen.getByRole('button', { name: /Delete all history/ })).toBeTruthy();
  });

  it('applies launch at login immediately without the save button', async () => {
    const user = userEvent.setup();
    const setLaunchAtLogin = vi.fn().mockResolvedValue({ ...baseSettings, launchAtLogin: true });
    useStore.setState({ setLaunchAtLogin });
    render(<Settings />);

    await user.click(screen.getByRole('button', { name: /Advanced/ }));
    await user.click(screen.getByRole('switch', { name: 'Launch at login' }));

    await waitFor(() => expect(setLaunchAtLogin).toHaveBeenCalledWith(true));
    expect(screen.getByRole('switch', { name: 'Launch at login' }).getAttribute('aria-checked')).toBe('true');
  });

  it('persists ignored applications immediately after confirmation', async () => {
    const user = userEvent.setup();
    const setIgnoredApps = vi.fn().mockImplementation(async (ignoredApps: SettingsType['ignoredApps']) => ({
      ...baseSettings,
      ignoredApps,
    }));
    useStore.setState({ setIgnoredApps });
    render(<Settings />);

    await user.click(screen.getByRole('button', { name: /Advanced/ }));
    await user.click(screen.getByRole('button', { name: 'Choose applications' }));
    await user.click(await screen.findByRole('option', { name: /Paint/ }));
    await user.click(screen.getByRole('button', { name: 'Confirm selection' }));

    await waitFor(() => expect(setIgnoredApps).toHaveBeenCalledWith([
      expect.objectContaining({ id: 'paint', executableName: 'paint.exe' }),
    ]));
  });
});

describe('Application picker', () => {
  it('deduplicates identities and sorts running and recent applications first', () => {
    const result = dedupeApplications([installedApp, { ...runningApp, running: false }, runningApp]);
    expect(result.map((app) => app.id)).toEqual(['notepad', 'paint']);
    expect(result[0]?.running).toBe(true);
  });

  it('discovers once, searches, multi-selects, removes chips, and confirms', async () => {
    const user = userEvent.setup();
    const confirm = vi.fn();
    render(<ApplicationPicker selected={[]} onClose={vi.fn()} onConfirm={confirm} />);
    expect(document.activeElement).toBe(screen.getByRole('searchbox'));
    await screen.findByRole('option', { name: /Notepad/ });
    expect(apiMock.extractApplicationIcon).not.toHaveBeenCalled();
    expect(apiMock.listInstalledApps).toHaveBeenCalledTimes(1);
    expect(apiMock.listRunningApps).toHaveBeenCalledTimes(1);
    await user.type(screen.getByRole('searchbox'), 'paint');
    expect(screen.queryByRole('option', { name: /Notepad/ })).toBeNull();
    await user.click(screen.getByRole('option', { name: /Paint/ }));
    expect(screen.getByLabelText('1 selected applications')).toBeTruthy();
    await user.clear(screen.getByRole('searchbox'));
    await user.click(screen.getByRole('option', { name: /Notepad/ }));
    expect(screen.getByText('2 selected')).toBeTruthy();
    await user.click(screen.getByTitle('Remove Paint'));
    await user.click(screen.getByRole('button', { name: 'Confirm selection' }));
    expect(confirm).toHaveBeenCalledWith([expect.objectContaining({ id: 'notepad' })]);
  });

  it('supports arrow navigation, Enter confirmation, and Escape close', async () => {
    const confirm = vi.fn();
    const close = vi.fn();
    render(<ApplicationPicker selected={[runningApp]} onClose={close} onConfirm={confirm} />);
    const search = screen.getByRole('searchbox');
    await screen.findByRole('option', { name: /Notepad/ });
    fireEvent.keyDown(search, { key: 'ArrowDown' });
    expect(search.getAttribute('aria-activedescendant')).toContain('paint');
    fireEvent.keyDown(search, { key: 'Enter' });
    expect(confirm).toHaveBeenCalledWith([runningApp]);
    fireEvent.keyDown(search, { key: 'Escape' });
    expect(close).toHaveBeenCalled();
  });

  it('shows error, retry, empty, and refresh states', async () => {
    apiMock.listRunningApps.mockRejectedValueOnce(new Error('discovery failed')).mockResolvedValue([]);
    apiMock.listInstalledApps.mockResolvedValue([]);
    const user = userEvent.setup();
    render(<ApplicationPicker selected={[]} onClose={vi.fn()} onConfirm={vi.fn()} />);
    expect(await screen.findByText('Applications could not be loaded')).toBeTruthy();
    await user.click(screen.getByRole('button', { name: 'Try again' }));
    expect(await screen.findByText('No applications found')).toBeTruthy();
    await user.click(screen.getByRole('button', { name: 'Refresh' }));
    expect(apiMock.listInstalledApps).toHaveBeenCalledTimes(3);
  });
});
