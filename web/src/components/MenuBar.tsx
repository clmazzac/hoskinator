import { useEffect, useState } from "react";
import { ChevronDown, Moon, Sun } from "lucide-react";

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
import { isDark, setDark } from "@/lib/theme";
import {
  resumeDesign,
  resumeThemes,
  setResumeTheme,
  setTopNote,
  type Design,
} from "@/rpc";

// Menus are empty until there are commands to put in them.
const MENUS = ["File", "Edit"];

export default function MenuBar({
  onThemeChanged,
}: {
  onThemeChanged?: () => void;
}) {
  const [dark, setDarkState] = useState(isDark);
  const [design, setDesign] = useState<Design | null>(null);
  const [styles, setStyles] = useState<string[]>([]);

  useEffect(() => {
    resumeThemes().then(setStyles, () => setStyles([]));
    resumeDesign().then(setDesign, () => setDesign(null));
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
      {MENUS.map((name) => (
        <DropdownMenu key={name}>
          <DropdownMenuTrigger
            render={
              <Button
                variant="ghost"
                size="sm"
                className="h-6 px-2 text-sm font-normal"
              >
                {name}
              </Button>
            }
          />
          <DropdownMenuContent align="start" className="min-w-40" />
        </DropdownMenu>
      ))}

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
