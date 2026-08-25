import { useState } from "react";
import { Moon, Sun } from "lucide-react";

import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { isDark, setDark } from "@/lib/theme";

// Menus are empty until there are commands to put in them.
const MENUS = ["File", "Edit"];

export default function MenuBar() {
  const [dark, setDarkState] = useState(isDark);

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
