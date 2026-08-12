/** @vitest-environment jsdom */
// ** import types
import type { Settings } from '../lib/types';

// ** import lib
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { useStore } from '../lib/store';
import { PairDeviceDialog } from './PairDeviceDialog';

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

const saveSettings = vi.fn(async (next: Settings) => next);
const regeneratePairingCode = vi.fn(async () => undefined);

beforeEach(() => {
  saveSettings.mockClear();
  regeneratePairingCode.mockClear();
  useStore.setState({
    settings,
    sync: {
      enabled: false,
      device: { id: 'windows-1', name: 'Office PC', platform: 'windows', color: '#39b9e8' },
      pairingCode: '123456',
      peers: [],
    },
    saveSettings,
    regeneratePairingCode,
  });
});

afterEach(() => cleanup());

describe('PairDeviceDialog', () => {
  it('joins another desktop by enabling sync and saving its six-digit code', async () => {
    const user = userEvent.setup();
    render(<PairDeviceDialog open onClose={vi.fn()} />);

    await user.type(screen.getByLabelText('Pairing code from another device'), '654321');
    await user.click(screen.getByRole('button', { name: 'Connect' }));

    await waitFor(() => expect(saveSettings).toHaveBeenCalledWith({
      ...settings,
      syncEnabled: true,
      syncPairingCode: '654321',
    }));
  });

  it('shows discovered peers and closes with Escape', () => {
    const close = vi.fn();
    useStore.setState({
      settings: { ...settings, syncEnabled: true },
      sync: {
        enabled: true,
        device: { id: 'windows-1', name: 'Office PC', platform: 'windows', color: '#39b9e8' },
        pairingCode: '123456',
        peers: [{
          device: { id: 'windows-2', name: 'Laptop', platform: 'windows', color: '#62c68b' },
          lastSeenAt: Date.now(),
          status: 'synced',
        }],
      },
    });
    render(<PairDeviceDialog open onClose={close} />);

    expect(screen.getByText('Connected: Laptop')).toBeTruthy();
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(close).toHaveBeenCalledOnce();
  });
});
