// Action states matching the backend
export type ActionState =
  | "proposed"
  | "paused_for_approval"
  | "ready_to_execute"
  | "pending_execution"
  | "executed"
  | "denied"
  | "failed";

// Approval status
export type ApprovalStatus = "pending" | "approved" | "denied";

// Tool types
export type ToolType = "send_email" | "http_request" | string;

// Base entity with timestamps
export interface Timestamps {
  created_at: string;
  updated_at: string;
}

// Action entity
export interface Action extends Timestamps {
  id: string;
  tenant_id: string;
  session_id: string;
  tool: ToolType;
  params: Record<string, unknown>;
  state: ActionState;
  policy_result: "allow" | "deny" | "require_approval" | null;
  policy_reason: string | null;
  result: Record<string, unknown> | null;
  error: string | null;
  execution_result: Record<string, unknown> | null;
  executed_by: string | null;
}

// Approval entity
export interface Approval extends Timestamps {
  id: string;
  tenant_id: string;
  action_id: string;
  status: ApprovalStatus;
  approver_id: string | null;
  approver_note: string | null;
  decided_at: string | null;
}

// Approval with action details (for list view)
export interface ApprovalWithAction extends Approval {
  action: Action;
}

// Event entity
export interface Event {
  id: string;
  tenant_id: string;
  session_id: string | null;
  action_id: string | null;
  event_type: string;
  payload: Record<string, unknown>;
  created_at: string;
}

// Session entity
export interface Session extends Timestamps {
  id: string;
  tenant_id: string;
  agent_id: string;
  actor_id: string | null;
  metadata: Record<string, unknown> | null;
  ended_at: string | null;
}

// Agent entity (list view - no sensitive data)
export interface AgentListItem {
  id: string;
  tenant_id: string;
  name: string;
  created_at: string;
}

// Dashboard user
export interface DashboardUser {
  id: string;
  tenant_id: string;
  email: string;
  name: string | null;
}

// Dashboard stats
export interface DashboardStats {
  pending_approvals: number;
  total_agents: number;
  active_sessions: number;
  actions_today: number;
  actions_by_state: Record<ActionState, number>;
}

// Tenant configuration (for settings page)
export interface TenantConfig {
  execution_webhook_url: string | null;
  execution_webhook_timeout_ms: number | null;
  policy_mode: "builtin" | "webhook" | null;
  policy_webhook_url: string | null;
  policy_webhook_timeout_ms: number | null;
}

export interface UpdateTenantConfigRequest {
  execution_webhook_url?: string;
  execution_webhook_secret?: string;
  execution_webhook_timeout_ms?: number;
  policy_mode?: "builtin" | "webhook";
  policy_webhook_url?: string;
  policy_webhook_secret?: string;
  policy_webhook_timeout_ms?: number;
}

// Paginated response wrapper (matches backend PaginatedResponse<T>)
export interface PaginatedResponse<T> {
  data: T[];
  limit: number;
  offset: number;
  has_more: boolean;
}

// API Request/Response types
export interface LoginRequest {
  email: string;
  password: string;
}

export interface LoginResponse {
  user: DashboardUser;
}

export interface ApprovalDecisionRequest {
  approved: boolean;
  note?: string;
}

export interface ApprovalDetail {
  approval: Approval;
  action: Action;
  events: Event[];
}

export interface ActionDetail {
  action: Action;
  approval: Approval | null;
  events: Event[];
}

// Filter params
export interface ApprovalsFilter {
  status?: ApprovalStatus;
  tool?: ToolType;
  limit?: number;
}

export interface ActionsFilter {
  session_id?: string;
  state?: ActionState;
  limit?: number;
  offset?: number;
}

export interface EventsFilter {
  action_id?: string;
  session_id?: string;
  event_type?: string;
  limit?: number;
}

// Helper to get safety preview from action params
export function getSafetyPreview(action: Action): string {
  const { tool, params } = action;

  if (tool === "send_email") {
    const to = params.to as string | undefined;
    const subject = params.subject as string | undefined;
    return `To: ${to || "unknown"} | Subject: ${subject || "no subject"}`;
  }

  if (tool === "http_request") {
    const method = (params.method as string | undefined) || "GET";
    const url = params.url as string | undefined;
    if (url) {
      try {
        const hostname = new URL(url).hostname;
        return `${method} ${hostname}`;
      } catch {
        return `${method} ${url}`;
      }
    }
    return method;
  }

  return `${tool} action`;
}
