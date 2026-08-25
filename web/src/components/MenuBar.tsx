import { useEffect, useState } from "react";
import { Moon, Sun } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { isDark, setDark } from "@/lib/theme";
import { resumeTheme, resumeThemes, setResumeTheme } from "@/rpc";

// Menus are empty until there are commands to put in them.
const MENUS = ["File", "Edit"];

export default function MenuBar({
  onThemeChanged,
}: {
  onThemeChanged?: () => void;
}) {
  const [dark, setDarkState] = useState(isDark);
  const [style, setStyle] = useState<string | null>(null);
  const [styles, setStyles] = useState<string[]>([]);

  useEffect(() => {
    resumeThemes().then(setStyles, () => setStyles([]));
    resumeTheme().then(setStyle, () => setStyle(null));
  }, []);

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

      {styles.length > 0 && (
        <Select
          value={style ?? ""}
          onValueChange={(next) => {
            if (!next) return;
            const previous = style;
            setStyle(next);
            setResumeTheme(next).then(
              () => onThemeChanged?.(),
              () => setStyle(previous),
            );
          }}
        >
          <SelectTrigger
            size="sm"
            className="h-6 w-40 border-transparent text-xs shadow-none hover:border-border"
            aria-label="Resume style"
          >
            <SelectValue placeholder="style" />
          </SelectTrigger>
          <SelectContent align="end">
            {styles.map((name) => (
              <SelectItem key={name} value={name} className="text-xs">
                {name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
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
