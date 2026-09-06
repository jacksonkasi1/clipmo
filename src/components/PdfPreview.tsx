import { useEffect, useRef, useState } from 'react';
import type { PDFDocumentLoadingTask, PDFWorker, RenderTask } from 'pdfjs-dist';
import { FolderOpen, LoaderCircle } from 'lucide-react';
import { api } from '../lib/tauri';
import { IconButton } from './IconButton';

export function PdfPreview({ itemId, index, name, path }: {
  itemId: number; index: number; name: string; path: string;
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [status, setStatus] = useState('Loading PDF preview…');
  const [ready, setReady] = useState(false);

  useEffect(() => {
    let disposed = false;
    let loading: PDFDocumentLoadingTask | undefined;
    let workerPort: Worker | undefined;
    let worker: PDFWorker | undefined;
    let rendering: RenderTask | undefined;
    let passwordProtected = false;
    setReady(false);
    setStatus('Loading PDF preview…');
    void (async () => {
      const [pdfjs, bytes] = await Promise.all([
        // WKWebView may lack Iterator helpers even on supported macOS versions.
        // Keep both the renderer and its worker on PDF.js's compatibility build.
        import('../lib/pdf-renderer'), api.readPdfPreview(itemId, index),
      ]);
      if (disposed) return;
      // Bundle the worker into a blob: WKWebView cannot import tauri:// modules
      // inside a worker, even though the main document can load them.
      workerPort = new pdfjs.CompatWorker();
      // @ts-expect-error PDF.js declares port as null, but accepts a Worker at runtime.
      worker = new pdfjs.PDFWorker({ port: workerPort });
      loading = pdfjs.getDocument({
        worker,
        data: new Uint8Array(bytes),
        cMapUrl: '/pdfjs/cmaps/',
        cMapPacked: true,
        standardFontDataUrl: '/pdfjs/standard_fonts/',
        wasmUrl: '/pdfjs/wasm/',
      });
      loading.onPassword = () => {
        passwordProtected = true;
        if (!disposed) setStatus('This PDF is password protected. Open the original file to view it.');
        void loading?.destroy();
      };
      const document = await loading.promise;
      const page = await document.getPage(1);
      if (disposed || !canvasRef.current) return;
      const natural = page.getViewport({ scale: 1 });
      const viewport = page.getViewport({ scale: Math.min(2, 1600 / Math.max(natural.width, natural.height)) });
      const canvas = canvasRef.current;
      canvas.width = Math.ceil(viewport.width);
      canvas.height = Math.ceil(viewport.height);
      rendering = page.render({ canvas, viewport, background: 'white' });
      await rendering.promise;
      if (!disposed) {
        setReady(true);
        setStatus(`Page 1 of ${document.numPages}`);
      }
    })().catch((error: unknown) => {
      if (!disposed && !passwordProtected) setStatus(`PDF preview unavailable. ${String(error)}`);
    });
    return () => {
      disposed = true;
      rendering?.cancel();
      void loading?.destroy().catch(() => undefined);
      worker?.destroy();
      workerPort?.terminate();
    };
  }, [itemId, index, path]);

  return (
    <article className="pdf-preview" aria-label={`PDF preview: ${name}`}>
      <div className="pdf-preview-page">
        <canvas ref={canvasRef} role="img" aria-label={`First page of ${name}`} hidden={!ready} />
        {!ready && <div role="status">{status === 'Loading PDF preview…' && <LoaderCircle className="spin" size={20} aria-hidden />}<span>{status}</span></div>}
      </div>
      <div className="preview-caption">
        <div className="pdf-preview-caption"><strong>{name}</strong>{ready && <span>{status}</span>}</div>
        <IconButton label={`Show ${name} in folder`} onClick={() => void api.revealItem(path)}><FolderOpen size={17} aria-hidden /></IconButton>
      </div>
    </article>
  );
}
