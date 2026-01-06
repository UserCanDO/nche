"use client";

import { useState } from "react";
import { useGetApprovalsQuery, useDecideApprovalMutation } from "@/lib/api";
import { ApprovalCard } from "@/components/approvals/approval-card";
import { ApproveDialog } from "@/components/approvals/approve-dialog";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Input } from "@/components/ui/input";
import { toast } from "sonner";
import type { ApprovalWithAction, ToolType } from "@/types";

export default function ApprovalsPage() {
  const [toolFilter, setToolFilter] = useState<ToolType | "all">("all");
  const [search, setSearch] = useState("");
  const [dialogState, setDialogState] = useState<{
    approval: ApprovalWithAction | null;
    mode: "approve" | "deny" | null;
  }>({ approval: null, mode: null });

  const { data: approvals, isLoading, error } = useGetApprovalsQuery({ status: "pending" });
  const [decideApproval, { isLoading: isDeciding }] = useDecideApprovalMutation();

  const filteredApprovals = approvals?.filter((a) => {
    if (toolFilter !== "all" && a.action.tool !== toolFilter) return false;
    if (search && !a.action_id.toLowerCase().includes(search.toLowerCase())) return false;
    return true;
  });

  const handleDecision = async (note: string) => {
    if (!dialogState.approval || !dialogState.mode) return;

    try {
      await decideApproval({
        id: dialogState.approval.id,
        decision: {
          approved: dialogState.mode === "approve",
          note: note || undefined,
        },
      }).unwrap();

      toast.success(
        dialogState.mode === "approve"
          ? "Action approved successfully"
          : "Action denied"
      );
      setDialogState({ approval: null, mode: null });
    } catch {
      toast.error("Failed to process decision");
    }
  };

  if (error) {
    return (
      <div className="text-center py-12">
        <p className="text-red-600">Failed to load approvals</p>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-gray-900">
          Pending Approvals
          {filteredApprovals && (
            <span className="ml-2 text-lg font-normal text-gray-500">
              ({filteredApprovals.length})
            </span>
          )}
        </h1>
      </div>

      <div className="flex items-center gap-4">
        <Select
          value={toolFilter}
          onValueChange={(v) => setToolFilter(v as ToolType | "all")}
        >
          <SelectTrigger className="w-48">
            <SelectValue placeholder="Filter by tool" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="all">All Tools</SelectItem>
            <SelectItem value="send_email">send_email</SelectItem>
            <SelectItem value="http_request">http_request</SelectItem>
          </SelectContent>
        </Select>

        <Input
          placeholder="Search by action ID..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="max-w-xs"
        />
      </div>

      {isLoading ? (
        <div className="text-center py-12 text-gray-500">Loading...</div>
      ) : filteredApprovals?.length === 0 ? (
        <div className="text-center py-12">
          <p className="text-gray-500">No pending approvals</p>
          <p className="text-sm text-gray-400 mt-1">
            New actions requiring approval will appear here
          </p>
        </div>
      ) : (
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {filteredApprovals?.map((approval) => (
            <ApprovalCard
              key={approval.id}
              approval={approval}
              onApprove={() =>
                setDialogState({ approval, mode: "approve" })
              }
              onDeny={() =>
                setDialogState({ approval, mode: "deny" })
              }
            />
          ))}
        </div>
      )}

      <ApproveDialog
        approval={dialogState.approval}
        mode={dialogState.mode}
        onClose={() => setDialogState({ approval: null, mode: null })}
        onConfirm={handleDecision}
        isLoading={isDeciding}
      />
    </div>
  );
}
