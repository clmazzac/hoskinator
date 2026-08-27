import { FolderGit2 } from "lucide-react";

import RepositorySetup from "@/components/RepositorySetup";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { repositorySlug } from "@/lib/utils";
import type { WorkspaceStatus } from "@/rpc";

/// The one place to see which repository is active and to switch to another.
export default function RepositoryDialog({
  status,
  open,
  onOpenChange,
  onReady,
}: {
  status: WorkspaceStatus;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onReady: (status: WorkspaceStatus) => void;
}) {
  const slug = repositorySlug(status.remote_url);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>Repository</DialogTitle>
        </DialogHeader>

        {status.repository_ready && (
          <div className="flex flex-wrap items-center gap-1.5 rounded-md border bg-muted/40 px-3 py-2 text-xs text-muted-foreground">
            <FolderGit2 className="size-3.5 shrink-0" />
            <span className="font-medium text-foreground">{slug ?? "Connected"}</span>
            {status.repository_path && (
              <>
                <span className="text-muted-foreground/50">·</span>
                <span className="truncate font-mono text-[11px]">
                  {status.repository_path}
                </span>
              </>
            )}
          </div>
        )}

        <RepositorySetup
          status={status}
          onReady={(next) => {
            onReady(next);
            onOpenChange(false);
          }}
        />
      </DialogContent>
    </Dialog>
  );
}
