import { Minus } from "lucide-react";

/// A hover-revealed remove affordance for a row inside a `group` container.
export default function RemoveButton({
  label,
  onClick,
}: {
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      className="grid size-4 shrink-0 place-items-center rounded-sm text-muted-foreground opacity-0 hover:bg-destructive/15 hover:text-destructive group-hover:opacity-100 focus-visible:opacity-100"
      onClick={(event) => {
        event.stopPropagation();
        onClick();
      }}
    >
      <Minus className="size-3" />
    </button>
  );
}
