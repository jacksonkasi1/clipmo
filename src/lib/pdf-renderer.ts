// PDF.js 6's legacy build still assumes newer baseline APIs exist.
// Feature-detected polyfills also prevent font parsing from silently failing in WKWebView.
import 'core-js/actual';
export { getDocument, PDFWorker } from 'pdfjs-dist/legacy/build/pdf.mjs';
export { default as CompatWorker } from './pdf.worker.js?worker&inline';
