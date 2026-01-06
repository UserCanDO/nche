"use client";

import { useState } from "react";
import Link from "next/link";
import { useGetEventsQuery } from "@/lib/api";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Button } from "@/components/ui/button";
import { formatDateTime } from "@/lib/utils";

const EVENT_TYPE_OPTIONS = [
  { value: "all", label: "All Events" },
  { value: "action.proposed", label: "Action Proposed" },
  { value: "approval.required", label: "Approval Required" },
  { value: "approval.approved", label: "Approved" },
  { value: "approval.denied", label: "Denied" },
  { value: "action.executed", label: "Executed" },
  { value: "action.failed", label: "Failed" },
];

export default function AuditPage() {
  const [eventType, setEventType] = useState<string>("all");

  const { data: events, isLoading, error } = useGetEventsQuery({
    event_type: eventType === "all" ? undefined : eventType,
    limit: 100,
  });

  const handleExport = () => {
    if (!events) return;

    const csv = [
      ["Timestamp", "Event Type", "Action ID", "Session ID", "Payload"],
      ...events.map((e) => [
        e.created_at,
        e.event_type,
        e.action_id ?? "",
        e.session_id ?? "",
        JSON.stringify(e.payload),
      ]),
    ]
      .map((row) => row.map((cell) => `"${cell}"`).join(","))
      .join("\n");

    const blob = new Blob([csv], { type: "text/csv" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `audit-log-${new Date().toISOString().split("T")[0]}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  };

  if (error) {
    return (
      <div className="text-center py-12">
        <p className="text-red-600">Failed to load audit log</p>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-gray-900">Audit Log</h1>
        <Button variant="outline" onClick={handleExport} disabled={!events?.length}>
          Export CSV
        </Button>
      </div>

      <div className="flex items-center gap-4">
        <Select value={eventType} onValueChange={setEventType}>
          <SelectTrigger className="w-56">
            <SelectValue placeholder="Filter by event type" />
          </SelectTrigger>
          <SelectContent>
            {EVENT_TYPE_OPTIONS.map((opt) => (
              <SelectItem key={opt.value} value={opt.value}>
                {opt.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      {isLoading ? (
        <div className="text-center py-12 text-gray-500">Loading...</div>
      ) : events?.length === 0 ? (
        <div className="text-center py-12">
          <p className="text-gray-500">No events found</p>
        </div>
      ) : (
        <div className="border rounded-md">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="w-[180px]">Timestamp</TableHead>
                <TableHead className="w-[160px]">Event Type</TableHead>
                <TableHead>Action / Session</TableHead>
                <TableHead>Summary</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {events?.map((event) => (
                <TableRow key={event.id}>
                  <TableCell className="text-sm">
                    {formatDateTime(event.created_at)}
                  </TableCell>
                  <TableCell>
                    <span className="inline-flex items-center px-2 py-1 rounded-full text-xs font-medium bg-gray-100 text-gray-800">
                      {event.event_type}
                    </span>
                  </TableCell>
                  <TableCell className="text-xs font-mono text-gray-500">
                    {event.action_id ? (
                      <Link
                        href={`/actions/detail/?id=${event.action_id}`}
                        className="hover:underline"
                      >
                        {event.action_id.slice(0, 8)}...
                      </Link>
                    ) : event.session_id ? (
                      <span>{event.session_id.slice(0, 8)}...</span>
                    ) : (
                      <span className="text-gray-400">-</span>
                    )}
                  </TableCell>
                  <TableCell className="text-sm text-gray-600 max-w-[300px] truncate">
                    {formatEventSummary(event.payload)}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
      )}
    </div>
  );
}

function formatEventSummary(payload: Record<string, unknown>): string {
  const entries = Object.entries(payload);
  if (entries.length === 0) return "-";

  return entries
    .slice(0, 3)
    .map(([key, value]) => {
      const strValue = typeof value === "object" ? JSON.stringify(value) : String(value);
      const truncated = strValue.length > 20 ? strValue.slice(0, 20) + "..." : strValue;
      return `${key}: ${truncated}`;
    })
    .join(", ");
}
