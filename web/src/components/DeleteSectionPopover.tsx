import { type RefObject } from "react";

import { Button } from "@/components/ui/button";
import { Popover, PopoverContent } from "@/components/ui/popover";

/// Confirms deleting one section, anchored to the row's own delete button.
export default function DeleteSectionPopover({
  section,
  anchor,
  open,
  busy,
  onOpenChange,
  onDelete,
}: {
  section: string;
  anchor: RefObject<HTMLElement | null>;
  open: boolean;
  busy: boolean;
  onOpenChange: (open: boolean) => void;
  onDelete: () => void;
}) {
  return (
    <Popover open={open} onOpenChange={onOpenChange}>
      <PopoverContent anchor={anchor} side="bottom" align="end" className="w-auto p-2">
        <div className="flex items-center gap-2">
          <span className="text-xs whitespace-nowrap">Delete {section}?</span>
          <Button size="xs" variant="ghost" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button size="xs" variant="destructive" disabled={busy} onClick={onDelete}>
            {busy ? "Deleting…" : "Delete"}
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  );
}
