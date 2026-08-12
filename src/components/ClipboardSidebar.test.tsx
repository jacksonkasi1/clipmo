/** @vitest-environment jsdom */
// ** import lib
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { useStore } from '../lib/store';
import { ClipboardSidebar } from './ClipboardSidebar';

beforeEach(() => {
  localStorage.clear();
  useStore.setState({
    sidebarOpen: true,
    tags: ['work'],
    tagColors: {},
    activeTag: null,
    devices: [],
    activeDeviceId: null,
    sources: [],
    activeSourceExe: null,
    settings: null,
  });
});

afterEach(cleanup);

describe('ClipboardSidebar', () => {
  it('portals the context menu outside the clipped rail and keeps it in the viewport', () => {
    render(<ClipboardSidebar />);

    fireEvent.contextMenu(screen.getByRole('button', { name: 'Filter by tag work' }), {
      clientX: window.innerWidth,
      clientY: window.innerHeight,
    });

    const menu = screen.getByRole('menu', { name: 'Actions for #work' });
    expect(screen.getByRole('navigation', { name: 'Clipboard filters' }).contains(menu)).toBe(false);
    expect(Number.parseInt(menu.style.left, 10)).toBeLessThan(window.innerWidth);
    expect(Number.parseInt(menu.style.top, 10)).toBeLessThan(window.innerHeight);
  });

  it('persists a custom tag color and can restore automatic color', () => {
    render(<ClipboardSidebar />);
    fireEvent.contextMenu(screen.getByRole('button', { name: 'Filter by tag work' }));

    fireEvent.change(screen.getByLabelText('Custom color for #work'), {
      target: { value: '#123456' },
    });

    expect(useStore.getState().tagColors.work).toBe('#123456');
    expect(localStorage.getItem('clipmo.tag-colors')).toContain('#123456');

    fireEvent.click(screen.getByRole('button', { name: 'Auto' }));
    expect(useStore.getState().tagColors.work).toBeUndefined();
  });
});
