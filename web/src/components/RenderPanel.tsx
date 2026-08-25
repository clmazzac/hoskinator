import { useCallback, useEffect, useImperativeHandle, useRef, useState } from "react";
import * as pdfjsLib from "pdfjs-dist";
import type { PDFDocumentProxy } from "pdfjs-dist";
import pdfjsWorkerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";

import RenderToolbar from "@/components/RenderToolbar";
import { renderAvailable, renderPreview } from "@/rpc";

pdfjsLib.GlobalWorkerOptions.workerSrc = pdfjsWorkerUrl;

export interface RenderHandle {
  /// Renders the current branch and shows the result.
  run: () => void;
}

function Note({ children }: { children: React.ReactNode }) {
  return <p className="p-3 font-mono text-xs text-muted-foreground">{children}</p>;
}

export default function RenderPanel({
  onCollapse,
  handle,
}: {
  onCollapse: () => void;
  handle: React.Ref<RenderHandle>;
}) {
  const [available, setAvailable] = useState<boolean | null>(null);
  const [rendering, setRendering] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Bumped on every successful render, to defeat the browser's cache of the same URL.
  const [stamp, setStamp] = useState(0);
  // Mirrors `rendering` for `run`, which closes over it before a state update can land.
  const renderingRef = useRef(false);

  const [doc, setDoc] = useState<PDFDocumentProxy | null>(null);
  const [pageCount, setPageCount] = useState(1);
  const [page, setPage] = useState(1);
  const [zoom, setZoom] = useState(100);
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    renderAvailable().then(setAvailable, () => setAvailable(false));
  }, []);

  const run = useCallback(() => {
    // A second render while one is in flight would race the first to the same PDF path.
    if (renderingRef.current) return;
    renderingRef.current = true;
    setRendering(true);
    renderPreview().then(
      () => {
        setError(null);
        setStamp(Date.now());
        renderingRef.current = false;
        setRendering(false);
      },
      (failure: Error) => {
        setError(failure.message);
        renderingRef.current = false;
        setRendering(false);
      },
    );
  }, []);

  useImperativeHandle(handle, () => ({ run }), [run]);

  // Loads the document itself whenever a render lands. The loading task (not the resolved
  // document) is what pdf.js wants closed, so a load superseded before it resolves is aborted.
  useEffect(() => {
    if (stamp === 0) return;
    const loading = pdfjsLib.getDocument({ url: `/preview.pdf?t=${stamp}` });
    loading.promise.then(
      (loaded) => {
        setDoc(loaded);
        setPageCount(loaded.numPages);
        setPage((current) => Math.min(Math.max(current, 1), loaded.numPages));
      },
      (failure: Error) => setError(String(failure.message ?? failure)),
    );
    return () => {
      void loading.destroy();
    };
  }, [stamp]);

  // Draws the current page at the current zoom. Sized for the display's actual pixel density,
  // not just the PDF's own coordinate space, so it reads crisply on a retina screen.
  useEffect(() => {
    if (!doc || !canvasRef.current) return;
    let cancelled = false;
    doc.getPage(page).then((pdfPage) => {
      if (cancelled) return;
      const canvas = canvasRef.current;
      if (!canvas) return;

      const viewport = pdfPage.getViewport({ scale: zoom / 100 });
      const pixelRatio = window.devicePixelRatio || 1;
      canvas.width = Math.floor(viewport.width * pixelRatio);
      canvas.height = Math.floor(viewport.height * pixelRatio);
      canvas.style.width = `${Math.floor(viewport.width)}px`;
      canvas.style.height = `${Math.floor(viewport.height)}px`;

      pdfPage.render({
        canvas,
        viewport,
        transform: pixelRatio !== 1 ? [pixelRatio, 0, 0, pixelRatio, 0, 0] : undefined,
      });
    });
    return () => {
      cancelled = true;
    };
  }, [doc, page, zoom]);

  return (
    <div className="flex h-full min-w-0 flex-col">
      <RenderToolbar
        onCollapse={onCollapse}
        onRender={run}
        rendering={rendering}
        disabled={available === false}
        zoom={zoom}
        onZoomChange={setZoom}
        page={page}
        pageCount={pageCount}
        onPageChange={setPage}
      />
      {available === false ? (
        <Note>rendercv is not on PATH, so nothing can be rendered.</Note>
      ) : error ? (
        <pre className="overflow-auto p-3 font-mono text-xs whitespace-pre-wrap text-destructive">
          {error}
        </pre>
      ) : stamp === 0 ? (
        <Note>Not rendered yet.</Note>
      ) : (
        <div className="min-h-0 flex-1 overflow-auto bg-muted">
          <canvas ref={canvasRef} className="mx-auto my-4 block shadow-sm" />
        </div>
      )}
    </div>
  );
}
