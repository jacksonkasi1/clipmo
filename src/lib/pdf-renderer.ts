// PDF.js 6's legacy build still assumes newer baseline APIs exist.
// Feature-detected polyfills also prevent font parsing from silently failing in WKWebView.
import 'core-js/actual';
export { getDocument, PDFWorker } from 'pdfjs-dist/legacy/build/pdf.mjs';
export { default as CompatWorker } from './pdf.worker.js?worker&inline';

// The sandboxed Mac webview may reject Worker creation with SecurityError.
// PDF.js's built-in loopback worker uses no browser Worker or blob URL.
export async function prepareMainThreadWorker() {
  await import('pdfjs-dist/legacy/build/pdf.worker.mjs');
}
