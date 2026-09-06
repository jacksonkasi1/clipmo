/** @vitest-environment jsdom */
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, expect, it, vi } from 'vitest';
const mocks = vi.hoisted(() => ({ read: vi.fn(), getDocument: vi.fn(), getPage: vi.fn(), render: vi.fn(), destroy: vi.fn(), cancel: vi.fn() }));
vi.mock('../lib/tauri', () => ({ api: { readPdfPreview: mocks.read, revealItem: vi.fn() } }));
vi.mock('pdfjs-dist', () => ({ GlobalWorkerOptions: {}, getDocument: mocks.getDocument }));
import { PdfPreview } from './PdfPreview';
beforeEach(() => {
  vi.resetAllMocks();
  mocks.read.mockResolvedValue(new ArrayBuffer(8));
  mocks.destroy.mockResolvedValue(undefined);
  mocks.render.mockReturnValue({ promise: Promise.resolve(), cancel: mocks.cancel });
  mocks.getPage.mockResolvedValue({ getViewport: ({ scale }: { scale: number }) => ({ width: 600 * scale, height: 800 * scale }), render: mocks.render });
  mocks.getDocument.mockReturnValue({ promise: Promise.resolve({ getPage: mocks.getPage, numPages: 3 }), destroy: mocks.destroy });
});
afterEach(cleanup);
it('renders only the first page and shows a filename instead of the path', async () => {
  render(<PdfPreview itemId={7} index={0} name="Report.PDF" path="/private/Report.PDF" />);
  await screen.findByText('Page 1 of 3');
  expect(mocks.read).toHaveBeenCalledWith(7, 0);
  expect(mocks.getPage).toHaveBeenCalledExactlyOnceWith(1);
  expect(screen.getByRole('img', { name: 'First page of Report.PDF' }).hidden).toBe(false);
  expect(screen.queryByText('/private/Report.PDF')).toBeNull();
});
it('shows a useful fallback when the file cannot be read', async () => {
  mocks.read.mockRejectedValue('File no longer exists');
  render(<PdfPreview itemId={7} index={0} name="Report.pdf" path="/Report.pdf" />);
  await screen.findByText(/PDF preview unavailable.*File no longer exists/);
  expect(mocks.getDocument).not.toHaveBeenCalled();
});
it('does not start rendering a stale selection after unmount', async () => {
  let resolve!: (value: ArrayBuffer) => void;
  mocks.read.mockReturnValue(new Promise<ArrayBuffer>((done) => { resolve = done; }));
  const view = render(<PdfPreview itemId={7} index={0} name="Report.pdf" path="/Report.pdf" />);
  view.unmount();
  resolve(new ArrayBuffer(8));
  await waitFor(() => expect(mocks.read).toHaveBeenCalled());
  expect(mocks.getDocument).not.toHaveBeenCalled();
});
it('releases the document and render task when selection changes', async () => {
  const view = render(<PdfPreview itemId={7} index={0} name="Report.pdf" path="/Report.pdf" />);
  await screen.findByText('Page 1 of 3');
  view.unmount();
  expect(mocks.cancel).toHaveBeenCalledOnce();
  expect(mocks.destroy).toHaveBeenCalledOnce();
});
