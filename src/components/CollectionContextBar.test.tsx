/** @vitest-environment jsdom */
// ** import lib
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { CollectionContextBar } from './CollectionContextBar';

afterEach(cleanup);

describe('CollectionContextBar', () => {
  it('identifies the opened collection and returns to the folder grid', () => {
    const onBack = vi.fn();
    render(
      <CollectionContextBar
        collection={{ name: 'work', itemCount: 4 }}
        tone={2}
        onBack={onBack}
      />,
    );

    expect(screen.getByRole('banner', { name: 'Inside work' })).toBeTruthy();
    expect(screen.getByText('4 items')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Back to collections' }));
    expect(onBack).toHaveBeenCalledTimes(1);
  });
});
