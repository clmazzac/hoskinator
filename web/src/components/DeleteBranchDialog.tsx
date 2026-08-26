import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";

/// Confirms the deletion of one branch. `branch` being null keeps the dialog closed.
export default function DeleteBranchDialog({
  branch,
  busy,
  onCancel,
  onDelete,
}: {
  branch: string | null;
  busy: boolean;
  onCancel: () => void;
  onDelete: () => void;
}) {
  return (
    <Dialog open={branch !== null} onOpenChange={(open) => !open && onCancel()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Delete {branch}?</DialogTitle>
        </DialogHeader>

        <p className="text-xs text-muted-foreground">
          The branch leaves the list. Commits that no other branch reaches are lost.
        </p>

        <DialogFooter>
          <DialogClose render={<Button variant="ghost">Cancel</Button>} />
          <Button variant="destructive" disabled={busy} onClick={onDelete}>
            {busy ? "Deleting…" : "Delete"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
