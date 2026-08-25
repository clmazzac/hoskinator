import { useState } from "react";
import {
  ChevronDown,
  ChevronUp,
  Contrast,
  Download,
  FileText,
  Minus,
  PanelRightClose,
  Plus,
} from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Separator } from "@/components/ui/separator";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";

// Zoom levels the percentage dropdown offers.
const ZOOM_STEPS = [50, 75, 100, 125, 150, 200];

const MIN_ZOOM = 25;
const MAX_ZOOM = 400;

function ToolbarButton({
  label,
  className,
  ...props
}: React.ComponentProps<typeof Button> & { label: string }) {
  return (
    <Button
      variant="ghost"
      size="icon"
      className={cn("size-7", className)}
      aria-label={label}
      title={label}
      {...props}
    />
  );
}

export default function RenderToolbar({
  onCollapse,
  problemCount = 0,
}: {
  onCollapse: () => void;
  problemCount?: number;
}) {
  const [autoCompile, setAutoCompile] = useState(false);
  const [zoom, setZoom] = useState(100);
  const [page, setPage] = useState(1);
  const pageCount = 1;

  const stepZoom = (by: number) =>
    setZoom((current) => Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, current + by)));

  return (
    <div className="flex h-10 shrink-0 items-center gap-1 border-b px-2">
      <div className="flex items-center">
        <Button size="sm" className="h-7 rounded-r-none px-3">
          Compile
        </Button>
        <DropdownMenu>
          <DropdownMenuTrigger
            render={
              <Button
                size="sm"
                className="h-7 rounded-l-none border-l border-background/25 px-1.5"
                aria-label="Compile options"
              >
                <ChevronDown className="size-3.5" />
              </Button>
            }
          />
          <DropdownMenuContent align="start" className="w-56">
            <div className="flex items-center justify-between gap-3 px-2 py-1.5 text-sm">
              <label htmlFor="auto-compile">Auto compile</label>
              <Switch
                id="auto-compile"
                size="sm"
                checked={autoCompile}
                onCheckedChange={setAutoCompile}
              />
            </div>
            <DropdownMenuSeparator />
            <DropdownMenuItem>Compile from scratch</DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>

      <div className="relative">
        <ToolbarButton label="Compile output" disabled={problemCount === 0}>
          <FileText className="size-4" />
        </ToolbarButton>
        {problemCount > 0 && (
          <span className="pointer-events-none absolute -top-0.5 -right-0.5 flex h-4 min-w-4 items-center justify-center rounded-full bg-destructive px-1 text-[10px] leading-none font-medium text-white">
            {problemCount}
          </span>
        )}
      </div>

      <ToolbarButton label="Download PDF">
        <Download className="size-4" />
      </ToolbarButton>

      <div className="flex-1" />

      <ToolbarButton label="Invert colours">
        <Contrast className="size-4" />
      </ToolbarButton>

      <ToolbarButton
        label="Previous page"
        disabled={page <= 1}
        onClick={() => setPage((p) => p - 1)}
      >
        <ChevronUp className="size-4" />
      </ToolbarButton>
      <ToolbarButton
        label="Next page"
        disabled={page >= pageCount}
        onClick={() => setPage((p) => p + 1)}
      >
        <ChevronDown className="size-4" />
      </ToolbarButton>

      <div className="flex items-center gap-1.5 text-xs tabular-nums">
        <input
          aria-label="Page"
          value={page}
          onChange={(event) => {
            const next = Number(event.target.value);
            if (Number.isInteger(next) && next >= 1 && next <= pageCount) {
              setPage(next);
            }
          }}
          className="h-6 w-9 rounded-sm border bg-transparent text-center focus-visible:ring-1 focus-visible:ring-ring focus-visible:outline-none"
        />
        <span className="text-muted-foreground">/ {pageCount}</span>
      </div>

      <Separator orientation="vertical" className="mx-1 h-5" />

      <ToolbarButton label="Zoom out" onClick={() => stepZoom(-25)}>
        <Minus className="size-4" />
      </ToolbarButton>
      <ToolbarButton label="Zoom in" onClick={() => stepZoom(25)}>
        <Plus className="size-4" />
      </ToolbarButton>

      <DropdownMenu>
        <DropdownMenuTrigger
          render={
            <Button
              variant="ghost"
              size="sm"
              className="h-7 gap-1 px-2 text-xs tabular-nums"
            >
              {zoom}%
              <ChevronDown className="size-3" />
            </Button>
          }
        />
        <DropdownMenuContent align="end">
          {ZOOM_STEPS.map((step) => (
            <DropdownMenuItem key={step} onClick={() => setZoom(step)}>
              {step}%
            </DropdownMenuItem>
          ))}
        </DropdownMenuContent>
      </DropdownMenu>

      <Separator orientation="vertical" className="mx-1 h-5" />

      <ToolbarButton label="Hide the rendered resume" onClick={onCollapse}>
        <PanelRightClose className="size-4" />
      </ToolbarButton>
    </div>
  );
}
