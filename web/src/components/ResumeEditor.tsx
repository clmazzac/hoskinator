import { useState, type ReactNode } from "react";
import { PanelRightClose, PanelRightOpen } from "lucide-react";
import { useDefaultLayout, usePanelRef } from "react-resizable-panels";

import { Button } from "@/components/ui/button";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";

// Identifies the saved layout in localStorage. Changing it discards saved sizes.
const LAYOUT_ID = "resume-editor";

// Smallest share of the window a panel keeps while dragging, as a percentage.
const MIN_PANEL_SIZE = "15%";

function Column({
  title,
  action,
  children,
}: {
  title: string;
  action?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="flex h-full min-w-0 flex-col">
      <header className="flex h-9 shrink-0 items-center justify-between gap-2 border-b pr-1 pl-3">
        <h2 className="truncate text-xs font-medium tracking-wide text-muted-foreground uppercase">
          {title}
        </h2>
        {action}
      </header>
      <ScrollArea className="min-h-0 flex-1">{children}</ScrollArea>
    </div>
  );
}

// Placeholder marking a column whose contents are not built yet.
function Unbuilt({ children }: { children: ReactNode }) {
  return (
    <p className="p-3 font-mono text-xs text-muted-foreground">{children}</p>
  );
}

// The rail that replaces the rendered resume when it is collapsed to the edge.
function CollapsedRail({ onExpand }: { onExpand: () => void }) {
  return (
    <div className="flex w-9 shrink-0 flex-col items-center gap-3 border-l pt-1">
      <Button
        variant="ghost"
        size="icon"
        className="size-7"
        onClick={onExpand}
        aria-label="Show the rendered resume"
      >
        <PanelRightOpen className="size-4" />
      </Button>
      <span
        className="text-xs font-medium tracking-wide text-muted-foreground uppercase"
        style={{ writingMode: "vertical-rl" }}
      >
        Rendered Resume
      </span>
    </div>
  );
}

export default function ResumeEditor() {
  const renderPanel = usePanelRef();
  const [renderCollapsed, setRenderCollapsed] = useState(false);
  const { defaultLayout, onLayoutChanged } = useDefaultLayout({
    id: LAYOUT_ID,
    storage: localStorage,
  });

  return (
    <div className="flex h-dvh bg-background text-foreground">
      <ResizablePanelGroup
        id={LAYOUT_ID}
        defaultLayout={defaultLayout}
        onLayoutChanged={onLayoutChanged}
        className={cn("min-w-0 flex-1", renderCollapsed && "pr-0")}
      >
        <ResizablePanel id="store" defaultSize="28" minSize={MIN_PANEL_SIZE}>
          <Column title="Master Store">
            <Unbuilt>Bullets you select to use in this resume.</Unbuilt>
          </Column>
        </ResizablePanel>

        <ResizableHandle withHandle />

        <ResizablePanel id="resume" defaultSize="28" minSize={MIN_PANEL_SIZE}>
          <Column title="This Resume">
            <Unbuilt>
              The Bullets and elements in use, by part of the resume.
            </Unbuilt>
          </Column>
        </ResizablePanel>

        <ResizableHandle withHandle className={cn(renderCollapsed && "hidden")} />

        <ResizablePanel
          id="render"
          defaultSize="44"
          minSize={MIN_PANEL_SIZE}
          collapsible
          collapsedSize={0}
          panelRef={renderPanel}
          onResize={(size) => setRenderCollapsed(size.asPercentage === 0)}
        >
          <Column
            title="Rendered Resume"
            action={
              <Button
                variant="ghost"
                size="icon"
                className="size-7"
                onClick={() => renderPanel.current?.collapse()}
                aria-label="Hide the rendered resume"
              >
                <PanelRightClose className="size-4" />
              </Button>
            }
          >
            <Unbuilt>The rendercv output for the current branch.</Unbuilt>
          </Column>
        </ResizablePanel>
      </ResizablePanelGroup>

      {renderCollapsed && (
        <CollapsedRail onExpand={() => renderPanel.current?.expand()} />
      )}
    </div>
  );
}
