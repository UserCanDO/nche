import { Badge } from "@/components/ui/badge";
import type { ActionState } from "@/types";

const STATUS_CONFIG: Record<ActionState, { label: string; variant: "default" | "secondary" | "destructive" | "outline" }> = {
  proposed: { label: "Proposed", variant: "secondary" },
  paused_for_approval: { label: "Awaiting Approval", variant: "default" },
  ready_to_execute: { label: "Ready", variant: "outline" },
  pending_execution: { label: "Pending Execution", variant: "secondary" },
  executed: { label: "Executed", variant: "outline" },
  denied: { label: "Denied", variant: "destructive" },
  failed: { label: "Failed", variant: "destructive" },
};

interface ActionStatusBadgeProps {
  state: ActionState;
}

export function ActionStatusBadge({ state }: ActionStatusBadgeProps) {
  const config = STATUS_CONFIG[state] ?? { label: state, variant: "secondary" as const };

  return (
    <Badge variant={config.variant}>
      {config.label}
    </Badge>
  );
}
