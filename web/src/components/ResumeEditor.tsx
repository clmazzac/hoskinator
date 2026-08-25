import { useState, type ReactNode } from "react";
import { PanelRightOpen } from "lucide-react";
import { useDefaultLayout, usePanelRef } from "react-resizable-panels";

import MasterStore from "@/components/MasterStore";
import ResumeOutline from "@/components/ResumeOutline";
import MenuBar from "@/components/MenuBar";
import RenderToolbar from "@/components/RenderToolbar";
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

function Column({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-full min-w-0 flex-col">
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

export default function ResumeEditor() {
  const renderPanel = usePanelRef();
  const [renderCollapsed, setRenderCollapsed] = useState(false);
  const { defaultLayout, onLayoutChanged } = useDefaultLayout({
    id: LAYOUT_ID,
    storage: localStorage,
  });

  return (
    <div className="flex h-dvh flex-col bg-background text-foreground">
      <MenuBar />
      <div className="flex min-h-0 flex-1">
        <ResizablePanelGroup
          id={LAYOUT_ID}
          defaultLayout={defaultLayout}
          onLayoutChanged={onLayoutChanged}
          className="min-w-0 flex-1"
        >
          <ResizablePanel id="store" defaultSize="28" minSize={MIN_PANEL_SIZE}>
            <Column>
              <MasterStore />
            </Column>
          </ResizablePanel>

          <ResizableHandle withHandle />

          <ResizablePanel id="resume" defaultSize="28" minSize={MIN_PANEL_SIZE}>
            <Column>
              <ResumeOutline />
            </Column>
          </ResizablePanel>

          <ResizableHandle
            withHandle
            className={cn(renderCollapsed && "hidden")}
          />

          <ResizablePanel
            id="render"
            defaultSize="44"
            minSize={MIN_PANEL_SIZE}
            collapsible
            collapsedSize={0}
            panelRef={renderPanel}
            onResize={(size) => setRenderCollapsed(size.asPercentage === 0)}
          >
            <div className="flex h-full min-w-0 flex-col">
              <RenderToolbar onCollapse={() => renderPanel.current?.collapse()} />
              <ScrollArea className="min-h-0 flex-1">
                <Unbuilt>The rendercv output for the current branch.</Unbuilt>
              </ScrollArea>
            </div>
          </ResizablePanel>
        </ResizablePanelGroup>

        {renderCollapsed && (
          <div className="flex w-9 shrink-0 flex-col items-center border-l pt-1.5">
            <Button
              variant="ghost"
              size="icon"
              className="size-7"
              onClick={() => renderPanel.current?.expand()}
              aria-label="Show the rendered resume"
              title="Show the rendered resume"
            >
              <PanelRightOpen className="size-4" />
            </Button>
          </div>
        )}
      </div>
    </div>
  );
}
