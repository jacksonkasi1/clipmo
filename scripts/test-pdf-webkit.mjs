// Build the real preview component, then render a two-page fixture in WKWebView.
// No mocks for PDF.js, its worker, canvas, or the browser engine.
import { build, mergeConfig } from 'vite';
import baseConfig from '../vite.config.ts';
import { existsSync, mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawnSync } from 'node:child_process';

const temporary = mkdtempSync(join(tmpdir(), 'clipmo-pdf-webkit-'));
const entry = resolve('pdf-webkit.html');
if (existsSync(entry)) throw new Error('Refusing to overwrite an existing PDF test entry');
const stream = '0 0 0 rg 40 40 150 150 re f BT /F1 14 Tf 20 360 Td (WebKit PDF preview) Tj ET';
const objects = [
  '<< /Type /Catalog /Pages 2 0 R >>',
  '<< /Type /Pages /Kids [3 0 R 5 0 R] /Count 2 >>',
  '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 400] /Resources << /Font << /F1 6 0 R >> >> /Contents 4 0 R >>',
  `<< /Length ${stream.length} >>\nstream\n${stream}\nendstream`,
  '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 400] >>',
  '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',
];
let pdf = '%PDF-1.4\n';
const offsets = [0];
objects.forEach((object, index) => { offsets.push(pdf.length); pdf += `${index + 1} 0 obj\n${object}\nendobj\n`; });
const xref = pdf.length;
pdf += `xref\n0 7\n0000000000 65535 f \n${offsets.slice(1).map(offset => `${String(offset).padStart(10, '0')} 00000 n \n`).join('')}trailer\n<< /Size 7 /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF`;
try {
  writeFileSync(entry, `<html><head><meta http-equiv="Content-Security-Policy" content="default-src 'self' customprotocol: asset:; connect-src 'self' ipc: http://ipc.localhost; img-src 'self' asset: http://asset.localhost blob: data:; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self' blob:; style-src 'self' 'unsafe-inline'"></head><body><div id="root"></div><script type="module">
import React from 'react';
import { createRoot } from 'react-dom/client';
import { PdfPreview } from '/src/components/PdfPreview.tsx';
import { api } from '/src/lib/tauri.ts';
const nativeIterator = typeof globalThis.Iterator;
// Also guard against regression on newer CI hosts which already implement it.
delete globalThis.Iterator;
delete Promise.withResolvers;
// Reproduce sandboxed WKWebView's SecurityError during worker creation.
globalThis.Worker = class { constructor() { throw new DOMException('Worker blocked by app sandbox', 'SecurityError'); } };
api.readPdfPreview = async () => Uint8Array.from(atob('${Buffer.from(pdf).toString('base64')}'), char => char.charCodeAt(0)).buffer;
const report = value => window.webkit.messageHandlers.result.postMessage(value);
new MutationObserver(() => {
  const failure = document.querySelector('[role="status"]');
  if (failure?.textContent.includes('unavailable')) report({ pass: false, error: failure.textContent, nativeIterator });
  if (document.body.textContent.includes('Page 1 of 2')) {
    const canvas = document.querySelector('canvas');
    const pixels = canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height).data;
    let ink = 0;
    let textInk = 0;
    for (let i = 0; i < pixels.length; i += 4) if (pixels[i] < 50 && pixels[i + 3] === 255) { ink++; if (i / 4 < canvas.width * 160) textInk++; }
    report({ pass: ink > 1000 && textInk > 100 && !canvas.hidden, ink, textInk, width: canvas.width, height: canvas.height, nativeIterator, restoredIterator: typeof globalThis.Iterator });
  }
}).observe(document.body, { subtree: true, childList: true, attributes: true });
createRoot(document.getElementById('root')).render(React.createElement(PdfPreview, { itemId: 1, index: 0, name: 'WebKit regression.pdf', path: '/WebKit regression.pdf' }));
</script></body></html>`);
  // Replace the normal entry points so the test bundle stays outside the app.
  await build(mergeConfig(baseConfig, { configFile: false, build: { outDir: temporary, emptyOutDir: false, rollupOptions: { input: { main: entry, settings: entry } } } }));
  const binary = join(temporary, 'test-pdf-webkit');
  const compile = spawnSync('clang', ['-fobjc-arc', '-framework', 'AppKit', '-framework', 'WebKit', 'scripts/test-pdf-webkit.m', '-o', binary], { stdio: 'inherit' });
  if (compile.status !== 0) throw new Error('WebKit test compilation failed');
  const test = spawnSync(binary, [temporary], { stdio: 'inherit', timeout: 65000 });
  if (test.status !== 0) throw new Error('PDF preview failed in native WebKit');
} finally {
  rmSync(entry, { force: true });
  if (!process.env.PDF_TEST_KEEP) rmSync(temporary, { recursive: true, force: true });
}
