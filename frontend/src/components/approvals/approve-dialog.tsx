"use client";

import { useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import { JsonViewer } from "@/components/common/json-viewer";
import { ToolBadge } from "@/components/common/tool-badge";
import type { ApprovalWithAction } from "@/types";
import { getSafetyPreview } from "@/types";

interface ApproveDialogProps {
  approval: ApprovalWithAction | null;
  mode: "approve" | "deny" | null;
  onClose: () => void;
  onConfirm: (note: string) => void;
  isLoading: boolean;
}

export function ApproveDialog({
  approval,
  mode,
  onClose,
  onConfirm,
  isLoading,
}: ApproveDialogProps) {
  const [note, setNote] = useState("");

  const handleConfirm = () => {
    onConfirm(note);
    setNote("");
  };

  const handleClose = () => {
    setNote("");
    onClose();
  };

  if (!approval || !mode) return null;

  const isApprove = mode === "approve";
  const action = approval.action;

  return (
    <Dialog open={!!approval && !!mode} onOpenChange={handleClose}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle>
            {isApprove ? "Approve Action" : "Deny Action"}
          </DialogTitle>
          <DialogDescription>
            {isApprove
              ? "This action will be queued for execution after approval."
              : "This action will be permanently denied."}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-4">
          <div className="flex items-center gap-2">
            <ToolBadge tool={action.tool} />
            <span className="text-sm text-gray-600">
              {getSafetyPreview(action)}
            </span>
          </div>

          {action.policy_reason && (
            <div>
              <Label className="text-xs text-gray-500">Policy Reason</Label>
              <p className="text-sm mt-1">{action.policy_reason}</p>
            </div>
          )}

          <div>
            <Label className="text-xs text-gray-500">Parameters</Label>
            <JsonViewer data={action.params} collapsed className="mt-1" />
          </div>

          <div>
            <Label htmlFor="note">
              {isApprove ? "Note (optional)" : "Reason (recommended)"}
            </Label>
            <Textarea
              id="note"
              value={note}
              onChange={(e) => setNote(e.target.value)}
              placeholder={
                isApprove
                  ? "Optional note for audit trail..."
                  : "Explain why this action is being denied..."
              }
              className="mt-1"
              rows={2}
            />
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={handleClose} disabled={isLoading}>
            Cancel
          </Button>
          <Button
            variant={isApprove ? "default" : "destructive"}
            onClick={handleConfirm}
            disabled={isLoading}
          >
            {isLoading
              ? "Processing..."
              : isApprove
              ? "Approve"
              : "Deny"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
