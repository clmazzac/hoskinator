import { useCallback, useEffect, useImperativeHandle, useState } from "react";

import RenderToolbar from "@/components/RenderToolbar";
import { renderAvailable, renderPreview } from "@/rpc";

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

  useEffect(() => {
    renderAvailable().then(setAvailable, () => setAvailable(false));
  }, []);

  const run = useCallback(() => {
    setRendering(true);
    renderPreview().then(
      () => {
        setError(null);
        setStamp(Date.now());
        setRendering(false);
      },
      (failure: Error) => {
        setError(failure.message);
        setRendering(false);
      },
    );
  }, []);

  useImperativeHandle(handle, () => ({ run }), [run]);

  return (
    <div className="flex h-full min-w-0 flex-col">
      <RenderToolbar
        onCollapse={onCollapse}
        onRender={run}
        rendering={rendering}
        disabled={available === false}
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
        <iframe
          key={stamp}
          title="Rendered resume"
          src={`/preview.pdf?t=${stamp}`}
          className="min-h-0 flex-1 border-0 bg-muted"
        />
      )}
    </div>
  );
}
