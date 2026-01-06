"use client";

import { useState } from "react";
import Link from "next/link";
import { useGetActionsQuery } from "@/lib/api";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { ToolBadge } from "@/components/common/tool-badge";
import { ActionStatusBadge } from "@/components/common/action-status-badge";
import { formatRelativeTime } from "@/lib/utils";
import type { ActionState } from "@/types";

const STATE_OPTIONS: Array<{ value: ActionState | "all"; label: string }> = [
  { value: "all", label: "All States" },
  { value: "proposed", label: "Proposed" },
  { value: "paused_for_approval", label: "Awaiting Approval" },
  { value: "ready_to_execute", label: "Ready to Execute" },
  { value: "pending_execution", label: "Pending Execution" },
  { value: "executed", label: "Executed" },
  { value: "denied", label: "Denied" },
  { value: "failed", label: "Failed" },
];

const TOOL_OPTIONS = [
  { value: "all", label: "All Tools" },
  { value: "email_send", label: "email_send" },
  { value: "slack_message", label: "slack_message" },
  { value: "sms_send", label: "sms_send" },
  { value: "http_request", label: "http_request" },
  { value: "payment_charge", label: "payment_charge" },
  { value: "database_query", label: "database_query" },
  { value: "ticket_create", label: "ticket_create" },
  { value: "calendar_event_create", label: "calendar_event_create" },
];

export default function ActionsPage() {
  const [stateFilter, setStateFilter] = useState<ActionState | "all">("all");
  const [toolFilter, setToolFilter] = useState<string>("all");
  const [search, setSearch] = useState("");
  const [page, setPage] = useState(0);
  const limit = 20;

  const { data: actions, isLoading, error } = useGetActionsQuery({
    state: stateFilter === "all" ? undefined : stateFilter,
    limit,
    offset: page * limit,
  });

  const filteredActions = actions?.filter((action) => {
    if (toolFilter !== "all" && action.tool !== toolFilter) return false;
    if (search && !action.id.toLowerCase().includes(search.toLowerCase())) return false;
    return true;
  });

  if (error) {
    return (
      <div className="text-center py-12">
        <p className="text-red-600">Failed to load actions</p>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold text-gray-900">Actions</h1>

      <div className="flex flex-wrap items-center gap-4">
        <Select
          value={stateFilter}
          onValueChange={(v) => {
            setStateFilter(v as ActionState | "all");
            setPage(0);
          }}
        >
          <SelectTrigger className="w-48">
            <SelectValue placeholder="Filter by state" />
          </SelectTrigger>
          <SelectContent>
            {STATE_OPTIONS.map((opt) => (
              <SelectItem key={opt.value} value={opt.value}>
                {opt.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>

        <Select
          value={toolFilter}
          onValueChange={(v) => {
            setToolFilter(v);
            setPage(0);
          }}
        >
          <SelectTrigger className="w-48">
            <SelectValue placeholder="Filter by tool" />
          </SelectTrigger>
          <SelectContent>
            {TOOL_OPTIONS.map((opt) => (
              <SelectItem key={opt.value} value={opt.value}>
                {opt.label}
              </SelectItem>
            ))}
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
      ) : filteredActions?.length === 0 ? (
        <div className="text-center py-12">
          <p className="text-gray-500">No actions found</p>
        </div>
      ) : (
        <>
          <div className="border rounded-md">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Created</TableHead>
                  <TableHead>Tool</TableHead>
                  <TableHead>State</TableHead>
                  <TableHead>Policy</TableHead>
                  <TableHead>Session</TableHead>
                  <TableHead>Action ID</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {filteredActions?.map((action) => (
                  <TableRow key={action.id}>
                    <TableCell className="text-sm">
                      {formatRelativeTime(action.created_at)}
                    </TableCell>
                    <TableCell>
                      <ToolBadge tool={action.tool} />
                    </TableCell>
                    <TableCell>
                      <ActionStatusBadge state={action.state} />
                    </TableCell>
                    <TableCell className="text-sm">
                      {action.policy_result}
                    </TableCell>
                    <TableCell className="text-xs font-mono text-gray-500 max-w-[120px] truncate">
                      {action.session_id.slice(0, 8)}...
                    </TableCell>
                    <TableCell>
                      <Link href={`/actions/detail/?id=${action.id}`}>
                        <Button variant="ghost" size="sm">
                          {action.id.slice(0, 8)}...
                        </Button>
                      </Link>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </div>

          <div className="flex items-center justify-between">
            <p className="text-sm text-gray-500">
              Showing {filteredActions?.length ?? 0} actions
            </p>
            <div className="flex gap-2">
              <Button
                variant="outline"
                size="sm"
                onClick={() => setPage((p) => Math.max(0, p - 1))}
                disabled={page === 0}
              >
                Previous
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() => setPage((p) => p + 1)}
                disabled={(actions?.length ?? 0) < limit}
              >
                Next
              </Button>
            </div>
          </div>
        </>
      )}
    </div>
  );
}
