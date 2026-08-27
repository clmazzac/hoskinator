import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { PanelBottomOpen, PanelRightOpen } from "lucide-react";
import { useDefaultLayout, usePanelRef } from "react-resizable-panels";

import MasterStore from "@/components/MasterStore";
import ResumeOutline from "@/components/ResumeOutline";
import MenuBar from "@/components/MenuBar";
import RenderPanel, { type RenderHandle } from "@/components/RenderPanel";
import TailoringPanel from "@/components/TailoringPanel";
import { Button } from "@/components/ui/button";
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from "@/components/ui/resizable";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useReloadOnHistory } from "@/lib/history";
import { cn } from "@/lib/utils";

// Identifies the saved layout in localStorage. Changing it discards saved sizes.
const LAYOUT_ID = "resume-editor";
// The outer top/bottom split holding the tailoring panel. A separate id keeps its saved
// size independent of the three-column layout above it.
const OUTER_LAYOUT_ID = "resume-editor-outer";

// Smallest share of the window a panel keeps while dragging, as a percentage.
const MIN_PANEL_SIZE = "15%";

// Floor on how often an edit can trigger a render. A render takes over a second
// (it shells out to rendercv), so re-rendering on every keystroke would queue up
// requests behind a preview that's already stale by the time it lands.
const RENDER_COOLDOWN_MS = 2000;

function Column({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-full min-w-0 flex-col">
      <ScrollArea className="min-h-0 flex-1">{children}</ScrollArea>
    </div>
  );
}

// Shown in the space a collapsed panel frees up, so it can be expanded again.
function CollapseExpander({
  side,
  icon,
  label,
  onExpand,
}: {
  side: "left" | "top";
  icon: ReactNode;
  label: string;
  onExpand: () => void;
}) {
  return (
    <div
      className={cn(
        "flex shrink-0 items-center",
        side === "left" ? "w-9 flex-col border-l pt-1.5" : "h-8 border-t pl-1.5",
      )}
    >
      <Button
        variant="ghost"
        size="icon"
        className="size-7"
        onClick={onExpand}
        aria-label={label}
        title={label}
      >
        {icon}
      </Button>
    </div>
  );
}

export default function ResumeEditor() {
  const renderPanel = usePanelRef();
  const tailoringPanel = usePanelRef();
  const render = useRef<RenderHandle>(null);
  const [renderCollapsed, setRenderCollapsed] = useState(false);
  const [tailoringCollapsed, setTailoringCollapsed] = useState(false);
  const { defaultLayout, onLayoutChanged } = useDefaultLayout({
    id: LAYOUT_ID,
    storage: localStorage,
  });
  const { defaultLayout: outerDefaultLayout, onLayoutChanged: onOuterLayoutChanged } =
    useDefaultLayout({
      id: OUTER_LAYOUT_ID,
      storage: localStorage,
    });

  // Runs at most once per cooldown window; an edit that lands mid-window is
  // picked up by the trailing render rather than dropped.
  const lastRenderAt = useRef(0);
  const cooldown = useRef<number | undefined>(undefined);
  const scheduleRender = useCallback(() => {
    const wait = RENDER_COOLDOWN_MS - (Date.now() - lastRenderAt.current);
    if (wait <= 0) {
      lastRenderAt.current = Date.now();
      render.current?.run();
      return;
    }
    if (cooldown.current !== undefined) return;
    cooldown.current = window.setTimeout(() => {
      cooldown.current = undefined;
      lastRenderAt.current = Date.now();
      render.current?.run();
    }, wait);
  }, []);
  useEffect(() => () => window.clearTimeout(cooldown.current), []);
  useReloadOnHistory(scheduleRender);

  return (
    <div className="flex h-dvh flex-col bg-background text-foreground">
      <MenuBar onThemeChanged={() => render.current?.run()} />
      <ResizablePanelGroup
        id={OUTER_LAYOUT_ID}
        orientation="vertical"
        defaultLayout={outerDefaultLayout}
        onLayoutChanged={onOuterLayoutChanged}
        className="min-h-0 flex-1"
      >
        <ResizablePanel id="workspace" defaultSize="72" minSize={MIN_PANEL_SIZE}>
          <div className="flex h-full min-h-0">
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
                <RenderPanel
                  handle={render}
                  onCollapse={() => renderPanel.current?.collapse()}
                />
              </ResizablePanel>
            </ResizablePanelGroup>

            {renderCollapsed && (
              <CollapseExpander
                side="left"
                icon={<PanelRightOpen className="size-4" />}
                label="Show the rendered resume"
                onExpand={() => renderPanel.current?.expand()}
              />
            )}
          </div>
        </ResizablePanel>

        <ResizableHandle withHandle className={cn(tailoringCollapsed && "hidden")} />

        <ResizablePanel
          id="tailoring"
          defaultSize="28"
          minSize={MIN_PANEL_SIZE}
          collapsible
          collapsedSize={0}
          panelRef={tailoringPanel}
          onResize={(size) => setTailoringCollapsed(size.asPercentage === 0)}
        >
          <TailoringPanel />
        </ResizablePanel>
      </ResizablePanelGroup>

      {tailoringCollapsed && (
        <CollapseExpander
          side="top"
          icon={<PanelBottomOpen className="size-4" />}
          label="Show the tailoring panel"
          onExpand={() => tailoringPanel.current?.expand()}
        />
      )}
    </div>
  );
}
