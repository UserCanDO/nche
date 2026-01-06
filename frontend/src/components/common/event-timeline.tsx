import type { Event } from "@/types";
import { formatRelativeTime } from "@/lib/utils";

interface EventTimelineProps {
  events: Event[];
}

export function EventTimeline({ events }: EventTimelineProps) {
  if (events.length === 0) {
    return (
      <p className="text-sm text-gray-500 italic">No events recorded</p>
    );
  }

  return (
    <div className="space-y-3">
      {events.map((event) => (
        <div
          key={event.id}
          className="flex items-start gap-3 text-sm"
        >
          <div className="w-2 h-2 mt-2 rounded-full bg-gray-400 flex-shrink-0" />
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2">
              <span className="font-medium text-gray-900">
                {formatEventType(event.event_type)}
              </span>
              <span className="text-gray-500">
                {formatRelativeTime(event.created_at)}
              </span>
            </div>
            {event.payload && Object.keys(event.payload).length > 0 && (
              <p className="text-gray-600 mt-0.5 truncate">
                {formatEventPayload(event.payload)}
              </p>
            )}
          </div>
        </div>
      ))}
    </div>
  );
}

function formatEventType(type: string): string {
  return type
    .split(".")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function formatEventPayload(payload: Record<string, unknown>): string {
  const entries = Object.entries(payload);
  if (entries.length === 0) return "";

  const preview = entries
    .slice(0, 2)
    .map(([key, value]) => {
      const strValue = typeof value === "object" ? JSON.stringify(value) : String(value);
      const truncated = strValue.length > 30 ? strValue.slice(0, 30) + "..." : strValue;
      return `${key}: ${truncated}`;
    })
    .join(", ");

  return entries.length > 2 ? `${preview}, ...` : preview;
}
