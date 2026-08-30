/** @vitest-environment jsdom */
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { useStore } from '../lib/store';
import { CollectionsView } from './CollectionsView';

afterEach(cleanup);

describe('CollectionsView', () => {
  beforeEach(() => {
    useStore.setState({
      collections: [
        { name: 'github', itemCount: 3 },
        { name: 'expenses', itemCount: 13 },
      ],
      items: [],
      tagColors: {},
    });
  });

  it('renders collections with names and item counts', () => {
    const onBack = vi.fn();
    render(<CollectionsView onBack={onBack} />);

    expect(screen.getByRole('heading', { name: 'Collections' })).toBeTruthy();
    expect(screen.getAllByText('github').length).toBeGreaterThan(0);
    expect(screen.getAllByText('3 items').length).toBeGreaterThan(0);
    expect(screen.getAllByText('expenses').length).toBeGreaterThan(0);
    expect(screen.getAllByText('13 items').length).toBeGreaterThan(0);
  });

  it('opens and cancels the creation editor when the folder search is absent', () => {
    render(<CollectionsView onBack={vi.fn()} />);
    expect(screen.queryByRole('searchbox', { name: 'Filter collections' })).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: 'New collection' }));
    expect(document.activeElement).toBe(screen.getByRole('textbox', { name: 'Collection name' }));
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(screen.getByRole('button', { name: 'New collection' })).toBeTruthy();
  });

  it('shows confirmation modal before deleting a collection', async () => {
    const deleteCollectionMock = vi.fn().mockResolvedValue(undefined);
    useStore.setState({ deleteCollection: deleteCollectionMock });

    const onBack = vi.fn();
    render(<CollectionsView onBack={onBack} />);

    // Click delete on inspector
    const deleteButton = screen.getByTitle('Delete collection');
    fireEvent.click(deleteButton);

    // Modal should be open
    expect(screen.getByRole('alertdialog')).toBeTruthy();
    expect(screen.getByText(/Delete collection “github”?/i)).toBeTruthy();
    expect(
      screen.getByText(/Clips will remain in your history, but will be removed from this collection./i),
    ).toBeTruthy();

    // Clicking Cancel closes modal without deleting
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(screen.queryByRole('alertdialog')).toBeNull();
    expect(deleteCollectionMock).not.toHaveBeenCalled();

    // Click delete again and confirm
    fireEvent.click(screen.getByTitle('Delete collection'));
    const confirmDeleteBtn = screen.getByRole('button', { name: 'Delete' });
    fireEvent.click(confirmDeleteBtn);

    expect(deleteCollectionMock).toHaveBeenCalledWith('github');
  });

  it('allows changing collection color from inspector', () => {
    const setTagColorMock = vi.fn();
    useStore.setState({ setTagColor: setTagColorMock });

    const onBack = vi.fn();
    render(<CollectionsView onBack={onBack} />);

    // Click preset color swatch in the inspector
    const swatch = screen.getByLabelText('Set color #62c68b');
    fireEvent.click(swatch);

    expect(setTagColorMock).toHaveBeenCalledWith('github', '#62c68b');
  });
});
