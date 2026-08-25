import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";

// Menus are empty until there are commands to put in them.
const MENUS = ["File", "Edit"];

export default function MenuBar() {
  return (
    <div className="flex h-8 shrink-0 items-center gap-0.5 border-b px-1">
      {MENUS.map((name) => (
        <DropdownMenu key={name}>
          <DropdownMenuTrigger
            render={
              <Button variant="ghost" size="sm" className="h-6 px-2 text-sm font-normal">
                {name}
              </Button>
            }
          />
          <DropdownMenuContent align="start" className="min-w-40" />
        </DropdownMenu>
      ))}
    </div>
  );
}
