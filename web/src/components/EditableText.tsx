import { useEffect, useRef, useState } from "react";

import { cn } from "@/lib/utils";

/// Text that edits in place. Commits on blur and Enter, abandons on Escape.
export default function EditableText({
  value,
  onCommit,
  placeholder,
  className,
  multiline = false,
}: {
  value: string;
  onCommit: (next: string) => void;
  placeholder?: string;
  className?: string;
  multiline?: boolean;
}) {
  const [draft, setDraft] = useState(value);
  const abandoned = useRef(false);

  useEffect(() => setDraft(value), [value]);

  const commit = () => {
    if (abandoned.current) {
      abandoned.current = false;
      return;
    }
    if (draft !== value) onCommit(draft);
  };

  const shared = {
    value: draft,
    placeholder,
    spellCheck: false,
    onChange: (
      event: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>,
    ) => setDraft(event.target.value),
    onBlur: commit,
    onKeyDown: (
      event: React.KeyboardEvent<HTMLInputElement | HTMLTextAreaElement>,
    ) => {
      if (event.key === "Escape") {
        abandoned.current = true;
        setDraft(value);
        event.currentTarget.blur();
      }
      if (event.key === "Enter" && (!multiline || event.metaKey)) {
        event.preventDefault();
        event.currentTarget.blur();
      }
    },
    // A drag started on the row must not begin when the caret is being placed.
    draggable: false,
    onDragStart: (event: React.DragEvent) => event.stopPropagation(),
    onPointerDown: (event: React.PointerEvent) => event.stopPropagation(),
    className: cn(
      "w-full rounded-sm border border-transparent bg-transparent px-1 -mx-1",
      "hover:border-border focus:border-ring focus:bg-background focus:outline-none",
      "placeholder:text-muted-foreground/50",
      className,
    ),
  };

  return multiline ? (
    <textarea {...shared} rows={2} className={cn(shared.className, "resize-y")} />
  ) : (
    <input {...shared} />
  );
}
