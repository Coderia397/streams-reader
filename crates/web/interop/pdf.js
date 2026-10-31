// Text extraction from PDF files with the vendored pdf.js legacy build (assets/vendor/pdfjs, pinned 6.4.299).
// Loaded lazily on the first PDF import: the library is ~0.5 MB and its worker ~1.3 MB.
// isEvalSupported:false keeps pdf.js from compiling code at runtime, which the CSP forbids anyway.

const BASE = "/app/assets/vendor/pdfjs/";
let pdfjs = null;

async function load() {
  if (!pdfjs) {
    const lib = await import(BASE + "pdf.min.mjs");
    lib.GlobalWorkerOptions.workerSrc = BASE + "pdf.worker.min.mjs";
    pdfjs = lib;
  }
  return pdfjs;
}

// Groups text items into lines by baseline. Items on the same line share (almost) the same y.
function toLines(items) {
  const lines = [];
  let cur = null;
  const flush = () => {
    if (cur && cur.text.trim()) lines.push(cur);
    cur = null;
  };
  for (const it of items) {
    if (typeof it.str !== "string") continue; // marked-content markers
    const y = it.transform[5];
    const h = it.height || Math.abs(it.transform[3]) || 0;
    if (cur && Math.abs(cur.y - y) <= Math.max(1, h * 0.3)) {
      cur.text += it.str;
      cur.height = Math.max(cur.height, h);
    } else {
      flush();
      cur = { text: it.str, y, height: h };
    }
    if (it.hasEOL) flush();
  }
  flush();
  return lines;
}

// bytes: Uint8Array (transferred to the pdf.js worker, so pass a copy).
// Returns { title: string|null, pages: [[{ text, y, height }]] }.
export async function extractPdf(bytes) {
  const lib = await load();
  const task = lib.getDocument({
    data: bytes,
    isEvalSupported: false,
    disableFontFace: true,
    enableXfa: false,
    stopAtErrors: false,
  });
  const doc = await task.promise;
  try {
    let title = null;
    try {
      const meta = await doc.getMetadata();
      title = (meta && meta.info && typeof meta.info.Title === "string" && meta.info.Title.trim()) || null;
    } catch (_) {
      title = null;
    }
    const pages = [];
    for (let n = 1; n <= doc.numPages; n++) {
      const page = await doc.getPage(n);
      const content = await page.getTextContent();
      pages.push(toLines(content.items));
      page.cleanup();
    }
    return { title, pages };
  } finally {
    // The loading task owns the document and its worker port (pdf.js 6 has no PDFDocumentProxy.destroy()).
    await task.destroy();
  }
}
