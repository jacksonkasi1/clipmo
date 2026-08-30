/** @vitest-environment jsdom */
// ** import lib
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { useStore } from '../lib/store';
import { QuickCollectionsView } from './quick-collections-view';

afterEach(cleanup);
beforeEach(() => {
  useStore.setState({
    collections: [{ name: 'work', itemCount: 3 }, { name: 'ideas', itemCount: 0 }],
    activeTag: 'work',
    tagColors: {},
  });
});

describe('QuickCollectionsView', () => {
  it('supports arrow-key navigation and shows the current collection', () => {
    render(<QuickCollectionsView onBack={vi.fn()} />);
    const work = screen.getByRole('button', { name: 'work 3 items' });
    const ideas = screen.getByRole('button', { name: 'ideas 0 items' });
    expect(work.getAttribute('aria-pressed')).toBe('true');
    work.focus();
    fireEvent.keyDown(work, { key: 'ArrowDown' });
    expect(document.activeElement).toBe(ideas);
    fireEvent.keyDown(ideas, { key: 'Home' });
    expect(document.activeElement).toBe(work);
  });

  it('provides a useful empty state and an Escape path back to history', () => {
    useStore.setState({ collections: [] });
    const onBack = vi.fn();
    render(<QuickCollectionsView onBack={onBack} />);
    expect(screen.getByText(/Create a collection in the main Clipmo window/)).toBeTruthy();
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onBack).toHaveBeenCalledOnce();
  });
});
