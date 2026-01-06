"use client";

import { Suspense } from "react";
import { useSearchParams } from "next/navigation";
import Link from "next/link";
import { useGetActionQuery } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { ToolBadge } from "@/components/common/tool-badge";
import { ActionStatusBadge } from "@/components/common/action-status-badge";
import { JsonViewer } from "@/components/common/json-viewer";
import { EventTimeline } from "@/components/common/event-timeline";
import { formatDateTime } from "@/lib/utils";

function ActionDetailContent() {
  const searchParams = useSearchParams();
  const id = searchParams.get("id");

  const { data, isLoading, error } = useGetActionQuery(id ?? "", { skip: !id });

  if (!id) {
    return (
      <div className="text-center py-12">
        <p className="text-red-600">No action ID provided</p>
        <Link href="/actions/">
          <Button variant="outline" className="mt-4">
            Back to Actions
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
        <p className="text-red-600">Failed to load action details</p>
        <Link href="/actions/">
          <Button variant="outline" className="mt-4">
            Back to Actions
          </Button>
        </Link>
      </div>
    );
  }

  const { action, approval, events } = data;

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <Link
            href="/actions/"
            className="text-sm text-gray-500 hover:text-gray-700"
          >
            &larr; Back to Actions
          </Link>
          <h1 className="text-2xl font-bold text-gray-900 mt-2">
            Action Detail
          </h1>
        </div>
        <ActionStatusBadge state={action.state} />
      </div>

      {action.state === "failed" && action.error && (
        <div className="bg-red-50 border border-red-200 p-4 rounded-md">
          <p className="font-medium text-red-800">Action Failed</p>
          <p className="text-sm text-red-700 mt-1">{action.error}</p>
        </div>
      )}

      <div className="grid gap-6 lg:grid-cols-3">
        <div className="lg:col-span-2 space-y-6">
          <Card>
            <CardHeader>
              <CardTitle>Summary</CardTitle>
            </CardHeader>
            <CardContent>
              <div className="grid gap-4 sm:grid-cols-2">
                <div>
                  <Label className="text-xs text-gray-500">Tool</Label>
                  <div className="mt-1">
                    <ToolBadge tool={action.tool} />
                  </div>
                </div>
                <div>
                  <Label className="text-xs text-gray-500">State</Label>
                  <div className="mt-1">
                    <ActionStatusBadge state={action.state} />
                  </div>
                </div>
                <div>
                  <Label className="text-xs text-gray-500">Created</Label>
                  <p className="text-sm mt-1">{formatDateTime(action.created_at)}</p>
                </div>
                {action.executed_by && (
                  <div>
                    <Label className="text-xs text-gray-500">Executed By</Label>
                    <p className="text-sm mt-1">{action.executed_by}</p>
                  </div>
                )}
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
              <CardTitle>Parameters</CardTitle>
            </CardHeader>
            <CardContent>
              <JsonViewer data={action.params} />
            </CardContent>
          </Card>

          {action.result && (
            <Card>
              <CardHeader>
                <CardTitle>Result</CardTitle>
              </CardHeader>
              <CardContent>
                <JsonViewer data={action.result} />
              </CardContent>
            </Card>
          )}

          {action.execution_result && (
            <Card>
              <CardHeader>
                <CardTitle>Execution Result</CardTitle>
              </CardHeader>
              <CardContent>
                <JsonViewer data={action.execution_result} />
              </CardContent>
            </Card>
          )}

          {action.error && (
            <Card>
              <CardHeader>
                <CardTitle className="text-red-700">Error</CardTitle>
              </CardHeader>
              <CardContent>
                <p className="text-sm text-red-700 font-mono">{action.error}</p>
              </CardContent>
            </Card>
          )}
        </div>

        <div className="space-y-6">
          {approval && (
            <Card>
              <CardHeader>
                <CardTitle>Approval</CardTitle>
              </CardHeader>
              <CardContent className="space-y-3">
                <div>
                  <Label className="text-xs text-gray-500">Status</Label>
                  <p className="text-sm mt-1 capitalize">{approval.status}</p>
                </div>
                {approval.approver_id && (
                  <div>
                    <Label className="text-xs text-gray-500">Approver</Label>
                    <p className="text-sm mt-1">{approval.approver_id}</p>
                  </div>
                )}
                {approval.decided_at && (
                  <div>
                    <Label className="text-xs text-gray-500">Decided</Label>
                    <p className="text-sm mt-1">{formatDateTime(approval.decided_at)}</p>
                  </div>
                )}
                {approval.approver_note && (
                  <div>
                    <Label className="text-xs text-gray-500">Note</Label>
                    <p className="text-sm mt-1">{approval.approver_note}</p>
                  </div>
                )}
                {approval.status === "pending" && (
                  <Link href={`/approvals/detail/?id=${approval.id}`}>
                    <Button size="sm" className="w-full mt-2">
                      Review Approval
                    </Button>
                  </Link>
                )}
              </CardContent>
            </Card>
          )}

          <Card>
            <CardHeader>
              <CardTitle>Policy</CardTitle>
            </CardHeader>
            <CardContent className="space-y-3">
              <div>
                <Label className="text-xs text-gray-500">Result</Label>
                <p className="text-sm mt-1 capitalize">{action.policy_result}</p>
              </div>
              {action.policy_reason && (
                <div>
                  <Label className="text-xs text-gray-500">Reason</Label>
                  <p className="text-sm mt-1">{action.policy_reason}</p>
                </div>
              )}
            </CardContent>
          </Card>

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

export default function ActionDetailPage() {
  return (
    <Suspense fallback={<div className="text-center py-12 text-gray-500">Loading...</div>}>
      <ActionDetailContent />
    </Suspense>
  );
}
