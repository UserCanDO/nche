"use client";

import { Suspense, useState } from "react";
import { useSearchParams, useRouter } from "next/navigation";
import Link from "next/link";
import { useGetApprovalQuery, useDecideApprovalMutation } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Textarea } from "@/components/ui/textarea";
import { Label } from "@/components/ui/label";
import { ToolBadge } from "@/components/common/tool-badge";
import { ActionStatusBadge } from "@/components/common/action-status-badge";
import { JsonViewer } from "@/components/common/json-viewer";
import { EventTimeline } from "@/components/common/event-timeline";
import { formatDateTime } from "@/lib/utils";
import { toast } from "sonner";

function ApprovalDetailContent() {
  const searchParams = useSearchParams();
  const router = useRouter();
  const id = searchParams.get("id");

  const { data, isLoading, error } = useGetApprovalQuery(id ?? "", { skip: !id });
  const [decideApproval, { isLoading: isDeciding }] = useDecideApprovalMutation();
  const [note, setNote] = useState("");

  const handleDecision = async (approved: boolean) => {
    if (!id) return;
    if (!approved && !note.trim()) {
      toast.error("Please provide a reason for denial");
      return;
    }

    try {
      await decideApproval({
        id,
        decision: { approved, note: note || undefined },
      }).unwrap();

      toast.success(approved ? "Action approved" : "Action denied");
      router.push("/approvals/");
    } catch {
      toast.error("Failed to process decision");
    }
  };

  if (!id) {
    return (
      <div className="text-center py-12">
        <p className="text-red-600">No approval ID provided</p>
        <Link href="/approvals/">
          <Button variant="outline" className="mt-4">
            Back to Approvals
          </Button>
        </Link>
      </div>
    );
  }

  if (isLoading) {
    return (
      <div className="text-center py-12 text-gray-500">Loading...</div>
    );
  }

  if (error || !data) {
    return (
      <div className="text-center py-12">
        <p className="text-red-600">Failed to load approval details</p>
        <Link href="/approvals/">
          <Button variant="outline" className="mt-4">
            Back to Approvals
          </Button>
        </Link>
      </div>
    );
  }

  const { approval, action, events } = data;
  const isPending = approval.status === "pending";
  const isDecided = approval.status !== "pending";

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <Link
            href="/approvals/"
            className="text-sm text-gray-500 hover:text-gray-700"
          >
            &larr; Back to Approvals
          </Link>
          <h1 className="text-2xl font-bold text-gray-900 mt-2">
            Approval Detail
          </h1>
        </div>
      </div>

      {isDecided && (
        <div
          className={`p-4 rounded-md ${
            approval.status === "approved"
              ? "bg-green-50 text-green-800"
              : "bg-red-50 text-red-800"
          }`}
        >
          <p className="font-medium">
            This action has been{" "}
            {approval.status === "approved" ? "approved" : "denied"}
            {approval.approver_id && ` by ${approval.approver_id}`}
            {approval.decided_at && ` on ${formatDateTime(approval.decided_at)}`}
          </p>
          {approval.approver_note && (
            <p className="mt-1 text-sm">{approval.approver_note}</p>
          )}
        </div>
      )}

      <div className="grid gap-6 lg:grid-cols-3">
        <div className="lg:col-span-2 space-y-6">
          <Card>
            <CardHeader>
              <CardTitle className="flex items-center justify-between">
                <span>Action Summary</span>
                <ActionStatusBadge state={action.state} />
              </CardTitle>
            </CardHeader>
            <CardContent className="space-y-4">
              <div className="grid gap-4 sm:grid-cols-2">
                <div>
                  <Label className="text-xs text-gray-500">Tool</Label>
                  <div className="mt-1">
                    <ToolBadge tool={action.tool} />
                  </div>
                </div>
                <div>
                  <Label className="text-xs text-gray-500">Created</Label>
                  <p className="text-sm mt-1">{formatDateTime(action.created_at)}</p>
                </div>
                <div>
                  <Label className="text-xs text-gray-500">Session ID</Label>
                  <p className="text-sm mt-1 font-mono text-xs truncate">
                    {action.session_id}
                  </p>
                </div>
                <div>
                  <Label className="text-xs text-gray-500">Action ID</Label>
                  <p className="text-sm mt-1 font-mono text-xs truncate">
                    {action.id}
                  </p>
                </div>
              </div>
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Policy</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="flex items-center gap-2">
                <span className="text-sm font-medium">
                  Result: {action.policy_result}
                </span>
              </div>
              {action.policy_reason && (
                <p className="text-sm text-gray-600 mt-2">
                  {action.policy_reason}
                </p>
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <CardTitle>Parameters</CardTitle>
            </CardHeader>
            <CardContent>
              <JsonViewer data={action.params} />
            </CardContent>
          </Card>
        </div>

        <div className="space-y-6">
          {isPending && (
            <Card>
              <CardHeader>
                <CardTitle>Decision</CardTitle>
              </CardHeader>
              <CardContent className="space-y-4">
                <div>
                  <Label htmlFor="decision-note">Note / Reason</Label>
                  <Textarea
                    id="decision-note"
                    value={note}
                    onChange={(e) => setNote(e.target.value)}
                    placeholder="Required for denial, optional for approval..."
                    className="mt-1"
                    rows={3}
                  />
                </div>
                <div className="flex gap-2">
                  <Button
                    variant="outline"
                    className="flex-1"
                    onClick={() => handleDecision(false)}
                    disabled={isDeciding}
                  >
                    Deny
                  </Button>
                  <Button
                    className="flex-1"
                    onClick={() => handleDecision(true)}
                    disabled={isDeciding}
                  >
                    Approve
                  </Button>
                </div>
              </CardContent>
            </Card>
          )}

          <Card>
            <CardHeader>
              <CardTitle>Timeline</CardTitle>
            </CardHeader>
            <CardContent>
              <EventTimeline events={events} />
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  );
}

export default function ApprovalDetailPage() {
  return (
    <Suspense fallback={<div className="text-center py-12 text-gray-500">Loading...</div>}>
      <ApprovalDetailContent />
    </Suspense>
  );
}
