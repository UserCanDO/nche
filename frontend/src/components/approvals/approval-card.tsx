"use client";

import Link from "next/link";
import { Card, CardContent, CardFooter } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { ToolBadge } from "@/components/common/tool-badge";
import { formatRelativeTime } from "@/lib/utils";
import type { ApprovalWithAction } from "@/types";
import { getSafetyPreview } from "@/types";

interface ApprovalCardProps {
  approval: ApprovalWithAction;
  onApprove: () => void;
  onDeny: () => void;
}

export function ApprovalCard({ approval, onApprove, onDeny }: ApprovalCardProps) {
  const action = approval.action;

  return (
    <Card>
      <CardContent className="pt-6">
        <div className="flex items-center justify-between">
          <ToolBadge tool={action.tool} />
          <span className="text-sm text-gray-500">
            {formatRelativeTime(approval.created_at)}
          </span>
        </div>

        <p className="mt-3 text-sm text-gray-700">
          {getSafetyPreview(action)}
        </p>

        {action.policy_reason && (
          <p className="mt-2 text-sm text-gray-500">
            {action.policy_reason}
          </p>
        )}
      </CardContent>

      <CardFooter className="gap-2 pt-0">
        <Button variant="outline" size="sm" onClick={onDeny}>
          Deny
        </Button>
        <Button size="sm" onClick={onApprove}>
          Approve
        </Button>
        <Link href={`/approvals/detail/?id=${approval.id}`} className="ml-auto">
          <Button variant="ghost" size="sm">
            View Details
          </Button>
        </Link>
      </CardFooter>
    </Card>
  );
}
