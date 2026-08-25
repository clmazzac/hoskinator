import { useEffect, useState } from "react";
import { ChevronDown, GitBranch, Home, Moon, Redo2, Sun, Undo2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  DropdownMenuCheckboxItem,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuSeparator,
} from "@/components/ui/dropdown-menu";
import { useHistory, useHistoryShortcuts } from "@/lib/history";
import { go } from "@/lib/route";
import { isDark, setDark } from "@/lib/theme";
import {
  repositoryState,
  resumeDesign,
  resumeThemes,
  setResumeTheme,
  setTopNote,
  type Design,
} from "@/rpc";

export default function MenuBar({
  onThemeChanged,
}: {
  onThemeChanged?: () => void;
}) {
  const { canUndo, canRedo, undo, redo } = useHistory();
  useHistoryShortcuts();
  const [dark, setDarkState] = useState(isDark);
  const [design, setDesign] = useState<Design | null>(null);
  const [styles, setStyles] = useState<string[]>([]);
  const [branch, setBranch] = useState<string | null>(null);

  useEffect(() => {
    resumeThemes().then(setStyles, () => setStyles([]));
    resumeDesign().then(setDesign, () => setDesign(null));
    repositoryState().then(
      (state) => setBranch(state.head?.branch ?? null),
      () => setBranch(null),
    );
  }, []);

  const revise = (next: Design, write: Promise<unknown>) => {
    const previous = design;
    setDesign(next);
    write.then(
      () => onThemeChanged?.(),
      () => setDesign(previous),
    );
  };

  const toggleTheme = () => {
    const next = !dark;
    setDark(next);
    setDarkState(next);
  };

  return (
    <div className="flex h-8 shrink-0 items-center gap-0.5 border-b px-1">
      <Button
        variant="ghost"
        size="sm"
        className="h-6 gap-1 px-2 text-xs font-normal"
        onClick={() => go("home")}
        title="Back to your resumes"
      >
        <Home className="size-3.5" />
      </Button>

      <Button
        variant="ghost"
        size="sm"
        className="h-6 gap-1 px-2 text-xs font-normal"
        onClick={() => void undo()}
        disabled={!canUndo}
        title="Undo (Ctrl+Z)"
      >
        <Undo2 className="size-3.5" />
        Undo
      </Button>
      <Button
        variant="ghost"
        size="sm"
        className="h-6 gap-1 px-2 text-xs font-normal"
        onClick={() => void redo()}
        disabled={!canRedo}
        title="Redo (Ctrl+Shift+Z)"
      >
        <Redo2 className="size-3.5" />
        Redo
      </Button>

      {branch && (
        <span
          className="ml-2 flex min-w-0 items-center gap-1 text-xs text-muted-foreground"
          title={`Editing ${branch}`}
        >
          <GitBranch className="size-3.5 shrink-0" />
          <span className="max-w-64 truncate font-mono text-[11px]">{branch}</span>
        </span>
      )}

      <div className="flex-1" />

      {design && styles.length > 0 && (
        <DropdownMenu>
          <DropdownMenuTrigger
            render={
              <Button
                variant="ghost"
                size="sm"
                className="h-6 gap-1 px-2 text-xs font-normal"
                aria-label="Resume style"
              >
                {design.theme ?? "style"}
                <ChevronDown className="size-3" />
              </Button>
            }
          />
          <DropdownMenuContent align="end" className="min-w-44">
            <DropdownMenuRadioGroup
              value={design.theme ?? ""}
              onValueChange={(next) => {
                if (!next || next === design.theme) return;
                revise({ ...design, theme: next }, setResumeTheme(next));
              }}
            >
              {styles.map((name) => (
                <DropdownMenuRadioItem key={name} value={name} className="text-xs">
                  {name}
                </DropdownMenuRadioItem>
              ))}
            </DropdownMenuRadioGroup>
            <DropdownMenuSeparator />
            <DropdownMenuCheckboxItem
              checked={design.show_top_note}
              onCheckedChange={(show) =>
                revise({ ...design, show_top_note: show }, setTopNote(show))
              }
              className="text-xs"
            >
              Last-updated note
            </DropdownMenuCheckboxItem>
          </DropdownMenuContent>
        </DropdownMenu>
      )}

      <Button
        variant="ghost"
        size="icon"
        className="size-6"
        onClick={toggleTheme}
        aria-label={dark ? "Switch to day mode" : "Switch to night mode"}
        title={dark ? "Switch to day mode" : "Switch to night mode"}
      >
        {dark ? <Sun className="size-4" /> : <Moon className="size-4" />}
      </Button>
    </div>
  );
}
