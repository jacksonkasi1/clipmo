/** @vitest-environment jsdom */
// ** import types
import type { ClipItem } from '../lib/types';

// ** import lib
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { useStore } from '../lib/store';
import { SelectionContextMenu } from './SelectionContextMenu';

const item = (id: number, favorite = false, tags: string[] = []): ClipItem => ({
  id,
  kind: 'text',
  preview: `Item ${id}`,
  content: `Item ${id}`,
  hasHtml: false,
  hasRtf: false,
  image: null,
  files: [],
  fileAssets: [],
  sizeBytes: 6,
  tags,
  source: null,
  favorite,
  copyCount: 1,
  device: { id: 'local', name: 'This device', platform: 'windows', color: '#000000' },
  syncStatus: 'local',
  firstCopiedAt: 1,
  lastCopiedAt: 1,
});

beforeEach(() => {
  useStore.setState({
    items: [item(1, false, ['work']), item(2, true, ['personal'])],
    selectedId: 1,
    selectedIds: [1],
    collections: [
      { name: 'work', itemCount: 1 },
      { name: 'personal', itemCount: 1 },
    ],
    addSelectedToCollection: vi.fn().mockResolvedValue(undefined),
    removeSelectedFromCollection: vi.fn().mockResolvedValue(undefined),
    setSelectedFavorites: vi.fn().mockResolvedValue(undefined),
    deleteSelected: vi.fn().mockResolvedValue(undefined),
  });
});

afterEach(cleanup);

describe('SelectionContextMenu', () => {
  it('uses singular labels for one selected item', () => {
    render(<SelectionContextMenu x={40} y={40} onClose={vi.fn()} />);

    expect(screen.getByRole('menu', { name: 'Item actions' })).toBeTruthy();
    expect(screen.queryByText('1 selected')).toBeNull();
    expect(screen.getByRole('menuitem', { name: 'Star' })).toBeTruthy();
    expect(screen.getByRole('menuitem', { name: 'Delete' })).toBeTruthy();
    expect(screen.queryByText(/all/i)).toBeNull();
  });

  it('uses compact multi-selection labels and reports the selected count once', () => {
    useStore.setState({ selectedId: 1, selectedIds: [1, 2] });
    render(<SelectionContextMenu x={40} y={40} onClose={vi.fn()} />);

    expect(screen.getByRole('menu', { name: 'Actions for 2 selected items' })).toBeTruthy();
    expect(screen.getByText('2 selected')).toBeTruthy();
    expect(screen.getByRole('menuitem', { name: 'Star' })).toBeTruthy();
    expect(screen.getByRole('menuitem', { name: 'Delete 2 items' })).toBeTruthy();
  });

  it('dismisses on outside pointer input and Escape', () => {
    const onClose = vi.fn();
    const { rerender } = render(<SelectionContextMenu x={40} y={40} onClose={onClose} />);

    fireEvent.pointerDown(document.body);
    expect(onClose).toHaveBeenCalledTimes(1);

    onClose.mockClear();
    rerender(<SelectionContextMenu x={40} y={40} onClose={onClose} />);
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('only offers collections that contain a selected item for removal', () => {
    render(<SelectionContextMenu x={40} y={40} onClose={vi.fn()} />);

    const remove = screen.getByText('Remove from collection').closest('details');
    expect(remove?.textContent).toContain('work');
    expect(remove?.textContent).not.toContain('personal');
  });
});
