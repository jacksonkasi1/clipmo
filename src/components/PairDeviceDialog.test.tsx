/** @vitest-environment jsdom */
// ** import types
import type { Settings } from '../lib/types';

// ** import lib
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { useStore } from '../lib/store';
import { PairDeviceDialog } from './PairDeviceDialog';
import QRCode from 'qrcode';

const settings: Settings = {
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
  showPreview: true,
  quickPreviewExpanded: false,
  syncEnabled: false,
  syncDeviceId: 'windows-1',
  syncDeviceName: 'Office PC',
  syncDeviceColor: '#39b9e8',
  syncPairingCode: '123456',
};

vi.mock('qrcode', () => ({ default: { toDataURL: vi.fn(async () => 'data:image/png;base64,test') } }));
const saveSettings = vi.fn(async (next: Settings) => next);
const regeneratePairingCode = vi.fn(async () => ({ ...settings, syncPairingCode: '777777' }));
const joinDevice = vi.fn<() => Promise<void>>();
const closePairing = vi.fn(async () => {});
const forgetSyncDevice = vi.fn(async () => {});
const loadSyncState = vi.fn(async () => {});

beforeEach(() => {
  vi.clearAllMocks();
  joinDevice.mockResolvedValue(undefined);
  useStore.setState({
    settings,
    sync: {
      enabled: true,
      device: { id: 'windows-1', name: 'Office PC', platform: 'windows', color: '#39b9e8' },
      pairingCode: '123456',
      pairingUntil: Date.now() + 120_000,
      peers: [],
    },
    saveSettings, regeneratePairingCode, joinDevice, closePairing, forgetSyncDevice, loadSyncState,
  });
});
afterEach(cleanup);

describe('PairDeviceDialog', () => {
  it('includes the direct LAN endpoint in QR and displays incompatible-device guidance', async () => {
    useStore.setState({ sync: { ...useStore.getState().sync!, localAddress: '192.168.1.16:47634', compatibilityWarning: 'Update the older Windows app.' } });
    render(<PairDeviceDialog open onClose={vi.fn()} />);
    expect(screen.getByText('Update the older Windows app.')).toBeTruthy();
    await waitFor(() => expect(QRCode.toDataURL).toHaveBeenCalledWith(expect.stringContaining('&address=192.168.1.16%3A47634'), expect.anything()));
  });

  it('joins without replacing the local invitation or claiming success before acknowledgement', async () => {
    let finish!: () => void;
    joinDevice.mockImplementationOnce(() => new Promise<void>((resolve) => { finish = resolve; }));
    const user = userEvent.setup();
    render(<PairDeviceDialog open onClose={vi.fn()} />);
    await user.type(screen.getByLabelText('Pairing code from another device'), '654321');
    await user.click(screen.getByRole('button', { name: 'Connect' }));
    expect(joinDevice).toHaveBeenCalledWith('654321');
    expect(saveSettings).not.toHaveBeenCalled();
    expect(useStore.getState().settings?.syncPairingCode).toBe('123456');
    expect(screen.queryByText(/Device connected/)).toBeNull();
    expect(screen.getByRole('button', { name: 'Connecting…' })).toBeTruthy();
    finish();
    await waitFor(() => expect(screen.getByText(/Device connected/)).toBeTruthy());
  });

  it('keeps the entered code and displays a failed handshake', async () => {
    joinDevice.mockRejectedValueOnce(new Error('Code expired'));
    const user = userEvent.setup();
    render(<PairDeviceDialog open onClose={vi.fn()} />);
    await user.type(screen.getByLabelText('Pairing code from another device'), '654321');
    await user.click(screen.getByRole('button', { name: 'Connect' }));
    await waitFor(() => expect(screen.getByRole('alert').textContent).toContain('Code expired'));
    expect((screen.getByLabelText('Pairing code from another device') as HTMLInputElement).value).toBe('654321');
    expect(screen.queryByText(/Device connected/)).toBeNull();
  });

  it('closes invitations without disabling sync', async () => {
    render(<PairDeviceDialog open onClose={vi.fn()} />);
    await userEvent.click(screen.getByRole('button', { name: 'Close pairing' }));
    expect(closePairing).toHaveBeenCalledOnce();
    expect(saveSettings).not.toHaveBeenCalled();
  });

  it('regenerates immediately and offers a QR invitation', async () => {
    render(<PairDeviceDialog open onClose={vi.fn()} />);
    await userEvent.click(screen.getByRole('button', { name: 'New code' }));
    expect(regeneratePairingCode).toHaveBeenCalledOnce();
    expect(forgetSyncDevice).not.toHaveBeenCalled();
    await waitFor(() => expect(screen.getByAltText(/Scan this pairing invitation/)).toBeTruthy());
  });

  it('shows saved offline connections and removes only the selected device', async () => {
    const close = vi.fn();
    const sync = useStore.getState().sync!;
    useStore.setState({ sync: { ...sync, peers: [{
      device: { id: 'android-1', name: 'Phone', platform: 'android', color: '#62c68b' },
      lastSeenAt: 0, status: 'offline',
    }] } });
    render(<PairDeviceDialog open onClose={close} />);
    expect(screen.getByText('Phone · Offline')).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Remove Phone' }));
    expect(forgetSyncDevice).toHaveBeenCalledWith('android-1');
    expect(regeneratePairingCode).not.toHaveBeenCalled();
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(close).toHaveBeenCalledOnce();
  });

  it('hides expired invitations', () => {
    useStore.setState({ sync: { ...useStore.getState().sync!, pairingUntil: Date.now() - 1 } });
    render(<PairDeviceDialog open onClose={vi.fn()} />);
    expect(screen.getByText('------')).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Close pairing' })).toBeNull();
    expect(screen.queryByAltText(/Scan this pairing invitation/)).toBeNull();
  });
});
