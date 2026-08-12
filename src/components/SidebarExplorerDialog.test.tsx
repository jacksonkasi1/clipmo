/** @vitest-environment jsdom */
// ** import lib
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { useStore } from '../lib/store';
import { SidebarExplorerDialog } from './SidebarExplorerDialog';

const setTag = vi.fn(async () => undefined);
const setDevice = vi.fn(async () => undefined);

beforeEach(() => {
  setTag.mockClear();
  setDevice.mockClear();
  useStore.setState({
    tags: ['work', 'personal', 'urgent', 'later'],
    activeTag: null,
    devices: [
      { id: 'local', name: 'Office PC', platform: 'windows', color: '#39b9e8' },
      { id: 'laptop', name: 'Laptop', platform: 'windows', color: '#62c68b' },
    ],
    activeDeviceId: null,
    setTag,
    setDevice,
  });
});

afterEach(cleanup);

describe('SidebarExplorerDialog', () => {
  it('shows every overflow tag and filters by the selected one', async () => {
    const close = vi.fn();
    const user = userEvent.setup();
    render(<SidebarExplorerDialog open onClose={close} />);

    await user.click(screen.getByRole('button', { name: '#later' }));
    expect(setTag).toHaveBeenCalledWith('later');
    expect(close).toHaveBeenCalledOnce();
  });

  it('shows every device and supports clearing the device filter', async () => {
    const user = userEvent.setup();
    render(<SidebarExplorerDialog open onClose={vi.fn()} />);

    expect(screen.getByRole('button', { name: /Laptop/ })).toBeTruthy();
    await user.click(screen.getByRole('button', { name: 'All devices' }));
    expect(setDevice).toHaveBeenCalledWith(null);
  });
});
