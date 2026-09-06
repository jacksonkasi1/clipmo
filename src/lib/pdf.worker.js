// Workers have their own globals: install the same compatibility support here.
import 'core-js/actual';
export { WorkerMessageHandler } from 'pdfjs-dist/legacy/build/pdf.worker.mjs';
