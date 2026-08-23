/** @vitest-environment jsdom */
import type { ClipItem } from '../lib/types';

import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const apiMock = vi.hoisted(() => ({
  copyToClipboard: vi.fn(),
  copyMultipleToClipboard: vi.fn(),
  pasteActive: vi.fn(),
  pasteMultipleActive: vi.fn(),
  editItem: vi.fn(),
  setFavorite: vi.fn(),
  deleteItem: vi.fn(),
  deleteSelected: vi.fn(),
  revealItem: vi.fn(),
  openExternalUrl: vi.fn(),
  // The store calls `refresh()` after every mutation; if the list endpoint
  // is unmocked the test surfaces an unhandled rejection. Returning an empty
  // list keeps the test focused on the action the toolbar actually fires.
  listItems: vi.fn(),
  counts: vi.fn(),
}));

const toastMock = vi.hoisted(() => ({
  toast: vi.fn(),
}));

vi.mock('../lib/tauri', () => ({
  api: apiMock,
  fileSrc: (path: string) => `asset://${path}`,
}));

vi.mock('../lib/toast', () => ({
  toast: toastMock.toast,
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ label: 'quick' }),
}));

import { PreviewPane } from './PreviewPane';
import { useStore } from '../lib/store';

const TEXT_ITEM: ClipItem = {
  id: 1,
  kind: 'text',
  preview: 'hello world',
  content: 'hello world',
  hasHtml: false,
  hasRtf: false,
  image: null,
  files: [],
  fileAssets: [],
  sizeBytes: 11,
  tags: [],
  source: { name: 'Notepad', exePath: 'C:/Windows/notepad.exe', iconPath: null },
  favorite: false,
  copyCount: 1,
  device: { id: 'local', name: 'This device', platform: 'windows', color: '#000' },
  syncStatus: 'local',
  firstCopiedAt: 1,
  lastCopiedAt: 1,
};

const LINK_ITEM: ClipItem = {
  ...TEXT_ITEM,
  id: 2,
  kind: 'link',
  preview: 'https://example.com',
  content: 'https://example.com',
};

const EMAIL_ITEM: ClipItem = {
  ...TEXT_ITEM,
  id: 3,
  kind: 'email',
  preview: 'foo@example.com',
  content: 'foo@example.com',
};

const COLOR_ITEM: ClipItem = {
  ...TEXT_ITEM,
  id: 4,
  kind: 'color',
  preview: '#39b9e8',
  content: '#39b9e8',
};

const FILE_ITEM: ClipItem = {
  ...TEXT_ITEM,
  id: 5,
  kind: 'files',
  preview: 'OG_images.png',
  content: 'C:/fake/OG_images.png',
  files: ['C:/fake/OG_images.png'],
  fileAssets: [
    {
      originalPath: 'C:/fake/OG_images.png',
      storedPath: 'C:/fake/OG_images.png',
      sizeBytes: 1234,
      isDirectory: false,
      status: 'ready',
      message: null,
      thumbPath: '',
    },
  ],
  source: { name: 'Explorer', exePath: 'C:/Windows/explorer.exe', iconPath: null },
};

const IMAGE_ITEM: ClipItem = {
  ...TEXT_ITEM,
  id: 6,
  kind: 'image',
  preview: 'screenshot.png',
  content: 'C:/fake/screenshot.png',
  image: {
    path: 'C:/fake/screenshot.png',
    thumbPath: '',
    width: 800,
    height: 600,
  },
};

function selectItem(item: ClipItem, extras: Partial<Pick<ClipItem, 'id'>>[] = []) {
  const ids = [item.id, ...extras.map((entry) => entry.id ?? item.id)];
  useStore.setState({ items: [item], selectedId: item.id, selectedIds: ids });
}

beforeEach(() => {
  apiMock.copyToClipboard.mockReset().mockResolvedValue(undefined);
  apiMock.pasteActive.mockReset().mockResolvedValue(undefined);
  apiMock.editItem.mockReset().mockResolvedValue(TEXT_ITEM);
  apiMock.setFavorite.mockReset().mockResolvedValue(undefined);
  apiMock.deleteItem.mockReset().mockResolvedValue(undefined);
  apiMock.deleteSelected.mockReset().mockResolvedValue(undefined);
  apiMock.revealItem.mockReset().mockResolvedValue(undefined);
  apiMock.openExternalUrl.mockReset().mockResolvedValue(undefined);
  apiMock.listItems.mockReset().mockResolvedValue([]);
  apiMock.counts.mockReset().mockResolvedValue({
    total: 0,
    favorites: 0,
    pinned: 0,
    text: 0,
    images: 0,
    files: 0,
    links: 0,
    colors: 0,
    emails: 0,
    storageBytes: 0,
  });
  toastMock.toast.mockReset();
  useStore.setState({
    items: [],
    selectedId: null,
    selectedIds: [],
    showDetails: true,
    mode: 'full',
  });
});

afterEach(() => {
  // Drain any document-level listeners left behind by OverflowMenu's
  // open state, so the next test starts with a clean DOM.
  document.body.innerHTML = '';
});

describe('PreviewPane SourceIndicator', () => {
  it('shows "From <source>" with a window glyph when the source is known', () => {
    selectItem(TEXT_ITEM);
    render(<PreviewPane />);
    // The source name is split across React text nodes, so query by a
    // regex that tolerates whitespace between "From" and the name.
    expect(screen.getByText(/From\s+Notepad/)).toBeDefined();
  });

  it('renders only the window glyph when the source attribution is missing', () => {
    const orphan: ClipItem = { ...TEXT_ITEM, source: null };
    selectItem(orphan);
    render(<PreviewPane />);
    // The chip is `aria-hidden` when no source is known — it is a decorative
    // placeholder, not a label the user can read.
    const chip = document.querySelector('.source-indicator');
    expect(chip).not.toBeNull();
    expect(chip?.getAttribute('aria-hidden')).toBe('true');
    expect(screen.queryByText(/From\s+/)).toBeNull();
  });
});

describe('PreviewPane context action', () => {
  it('renders an Open-in-browser button for link items', () => {
    selectItem(LINK_ITEM);
    render(<PreviewPane />);
    expect(screen.getByLabelText('Open in browser')).toBeDefined();
  });

  it('renders an Open-in-browser button for email items', () => {
    selectItem(EMAIL_ITEM);
    render(<PreviewPane />);
    expect(screen.getByLabelText('Open in browser')).toBeDefined();
  });

  it('renders a Reveal-in-File-Explorer button for files items', () => {
    selectItem(FILE_ITEM);
    render(<PreviewPane />);
    expect(screen.getByLabelText('Reveal in File Explorer')).toBeDefined();
  });

  it('renders a Reveal-in-File-Explorer button for image items', () => {
    selectItem(IMAGE_ITEM);
    render(<PreviewPane />);
    expect(screen.getByLabelText('Reveal in File Explorer')).toBeDefined();
  });

  it('does not render any context action for plain text items', () => {
    selectItem(TEXT_ITEM);
    render(<PreviewPane />);
    expect(screen.queryByLabelText('Open in browser')).toBeNull();
    expect(screen.queryByLabelText('Reveal in File Explorer')).toBeNull();
  });

  it('does not render any context action for color items', () => {
    selectItem(COLOR_ITEM);
    render(<PreviewPane />);
    expect(screen.queryByLabelText('Open in browser')).toBeNull();
    expect(screen.queryByLabelText('Reveal in File Explorer')).toBeNull();
  });

  it('routes a link click to api.openExternalUrl with a normalised URL', async () => {
    selectItem(LINK_ITEM);
    render(<PreviewPane />);
    fireEvent.click(screen.getByLabelText('Open in browser'));
    await waitFor(() => {
      expect(apiMock.openExternalUrl).toHaveBeenCalledWith('https://example.com');
    });
  });

  it('routes a file click to api.revealItem with the file path', async () => {
    selectItem(FILE_ITEM);
    render(<PreviewPane />);
    fireEvent.click(screen.getByLabelText('Reveal in File Explorer'));
    await waitFor(() => {
      expect(apiMock.revealItem).toHaveBeenCalledWith('C:/fake/OG_images.png');
    });
  });

  it('surfaces a toast when the link scheme cannot be parsed', async () => {
    const bad: ClipItem = { ...LINK_ITEM, content: '!!! not a url !!!', preview: '!!! not a url !!!' };
    selectItem(bad);
    render(<PreviewPane />);
    fireEvent.click(screen.getByLabelText('Open in browser'));
    await waitFor(() => {
      expect(toastMock.toast).toHaveBeenCalledWith(expect.stringContaining('not a URL'), 'error');
    });
    expect(apiMock.openExternalUrl).not.toHaveBeenCalled();
  });
});

describe('PreviewPane OverflowMenu', () => {
  it('renders the kebab trigger with the right ARIA attributes', () => {
    selectItem(TEXT_ITEM);
    render(<PreviewPane />);
    const trigger = screen.getByLabelText('More actions');
    expect(trigger.getAttribute('aria-haspopup')).toBe('menu');
    expect(trigger.getAttribute('aria-expanded')).toBe('false');
    // The popover is not in the DOM until the trigger is clicked.
    expect(document.querySelector('.toolbar-overflow-menu')).toBeNull();
  });

  it('opens the popover on click and surfaces the secondary actions', () => {
    selectItem(TEXT_ITEM);
    const { container } = render(<PreviewPane />);
    fireEvent.click(screen.getByLabelText('More actions'));
    const menu = container.querySelector('.toolbar-overflow-menu');
    expect(menu).not.toBeNull();
    expect(menu?.getAttribute('role')).toBe('menu');
    // showDetails starts true in the store, so the menu item reads
    // "Hide details" until the user toggles it.
    expect(screen.getByText('Hide details')).toBeDefined();
    expect(screen.queryByText('Show details')).toBeNull();
    expect(screen.getByText('Delete item')).toBeDefined();
  });

  it('closes the popover when clicking outside the menu', async () => {
    selectItem(TEXT_ITEM);
    const { container } = render(<PreviewPane />);
    fireEvent.click(screen.getByLabelText('More actions'));
    expect(container.querySelector('.toolbar-overflow-menu')).not.toBeNull();
    // Simulate a click on document body that the container does not contain.
    fireEvent.mouseDown(document.body);
    await waitFor(() => {
      expect(container.querySelector('.toolbar-overflow-menu')).toBeNull();
    });
  });

  it('closes the popover on Escape', async () => {
    selectItem(TEXT_ITEM);
    const { container } = render(<PreviewPane />);
    fireEvent.click(screen.getByLabelText('More actions'));
    expect(container.querySelector('.toolbar-overflow-menu')).not.toBeNull();
    fireEvent.keyDown(document, { key: 'Escape' });
    await waitFor(() => {
      expect(container.querySelector('.toolbar-overflow-menu')).toBeNull();
    });
  });

  it('toggles showDetails via the Show/Hide details menu item', async () => {
    selectItem(TEXT_ITEM);
    useStore.setState({ showDetails: true });
    const { container } = render(<PreviewPane />);
    fireEvent.click(screen.getByLabelText('More actions'));
    fireEvent.click(screen.getByText('Hide details'));
    await waitFor(() => {
      expect(useStore.getState().showDetails).toBe(false);
    });
    expect(container.querySelector('.toolbar-overflow-menu')).toBeNull();
  });

  it('uses the plural label and deleteSelected when multiple items are selected', async () => {
    selectItem(TEXT_ITEM);
    // A real second item isn't needed — the OverflowMenu only reads
    // `selectedIds.length` to decide between the singular and plural
    // label, and which store action to fire. Pin both so the test does
    // not have to mock out a second `listItems` payload.
    useStore.setState({ selectedIds: [TEXT_ITEM.id, 4242] });
    render(<PreviewPane />);
    fireEvent.click(screen.getByLabelText('More actions'));
    fireEvent.click(screen.getByText(/Delete 2 items/));
    // `deleteSelected` is the store action; the underlying Tauri call
    // it fans out to is `delete_item`, so the test asserts on that
    // mock to confirm the plural path actually reached the native side.
    await waitFor(() => {
      expect(apiMock.deleteItem).toHaveBeenCalled();
    });
  });

  it('uses the singular label and deleteItem when only the active row is selected', async () => {
    selectItem(TEXT_ITEM);
    render(<PreviewPane />);
    fireEvent.click(screen.getByLabelText('More actions'));
    fireEvent.click(screen.getByText('Delete item'));
    await waitFor(() => {
      expect(apiMock.deleteItem).toHaveBeenCalledWith(TEXT_ITEM.id);
    });
    expect(apiMock.deleteSelected).not.toHaveBeenCalled();
  });

  it('renders MultiItemPreview and invokes multi copy/paste actions when multiple items are selected', async () => {
    useStore.setState({
      items: [TEXT_ITEM, LINK_ITEM],
      selectedId: TEXT_ITEM.id,
      selectedIds: [TEXT_ITEM.id, LINK_ITEM.id],
    });
    render(<PreviewPane />);

    expect(screen.getByText('2 items selected')).toBeDefined();
    expect(screen.getByText('hello world')).toBeDefined();
    expect(screen.getByText('https://example.com')).toBeDefined();

    // Copy multiple
    const copyBtn = screen.getByRole('button', { name: /Copy 2 items/i });
    fireEvent.click(copyBtn);
    await waitFor(() => {
      expect(apiMock.copyMultipleToClipboard).toHaveBeenCalledWith(
        [TEXT_ITEM.id, LINK_ITEM.id],
        'original',
      );
    });

    // Paste multiple
    const pasteBtn = screen.getByRole('button', { name: /Paste 2 items/i });
    fireEvent.click(pasteBtn);
    await waitFor(() => {
      expect(apiMock.pasteMultipleActive).toHaveBeenCalledWith(
        [TEXT_ITEM.id, LINK_ITEM.id],
        'original',
      );
    });
  });
});

describe('PreviewPane ImagePreview and Fullscreen Modal', () => {
  it('renders image dimensions in caption and an expand button', () => {
    selectItem(IMAGE_ITEM);
    render(<PreviewPane />);
    expect(screen.getByText('800 × 600 pixels')).toBeDefined();
    expect(screen.getByLabelText('View full screen')).toBeDefined();
    expect(screen.getByLabelText('Click image to view full screen')).toBeDefined();
  });

  it('opens fullscreen modal on canvas click and closes on Close button click', async () => {
    selectItem(IMAGE_ITEM);
    render(<PreviewPane />);
    expect(screen.queryByRole('dialog')).toBeNull();

    // Click on canvas
    fireEvent.click(screen.getByLabelText('Click image to view full screen'));
    expect(screen.getByRole('dialog')).toBeDefined();
    expect(screen.getByText('800 × 600 px')).toBeDefined();

    // Click on close button
    fireEvent.click(screen.getByRole('button', { name: 'Close full screen' }));
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
  });

  it('closes fullscreen modal when Escape key is pressed', async () => {
    selectItem(IMAGE_ITEM);
    render(<PreviewPane />);

    // Open fullscreen
    fireEvent.click(screen.getByLabelText('View full screen'));
    expect(screen.getByRole('dialog')).toBeDefined();

    // Press Escape
    fireEvent.keyDown(window, { key: 'Escape' });
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
  });

  it('copies image from fullscreen modal toolbar', async () => {
    selectItem(IMAGE_ITEM);
    render(<PreviewPane />);

    fireEvent.click(screen.getByLabelText('View full screen'));
    const dialog = screen.getByRole('dialog');
    const copyBtn = dialog.querySelector('button[aria-label*="Copy"]');
    expect(copyBtn).not.toBeNull();
    fireEvent.click(copyBtn!);
    await waitFor(() => {
      expect(apiMock.copyToClipboard).toHaveBeenCalledWith(IMAGE_ITEM.id, 'original');
    });
  });
});
