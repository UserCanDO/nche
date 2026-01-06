"use client";

import { useState } from "react";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";

interface JsonViewerProps {
  data: unknown;
  collapsed?: boolean;
  className?: string;
}

export function JsonViewer({ data, collapsed = false, className }: JsonViewerProps) {
  const [isCollapsed, setIsCollapsed] = useState(collapsed);
  const formatted = JSON.stringify(data, null, 2);
  const lines = formatted.split("\n");
  const isLarge = lines.length > 10;

  return (
    <div className={cn("relative", className)}>
      {isLarge && (
        <Button
          variant="ghost"
          size="sm"
          className="absolute top-2 right-2 text-xs"
          onClick={() => setIsCollapsed(!isCollapsed)}
        >
          {isCollapsed ? "Expand" : "Collapse"}
        </Button>
      )}
      <pre
        className={cn(
          "bg-gray-50 rounded-md p-4 text-sm overflow-auto font-mono text-gray-800",
          isCollapsed && isLarge && "max-h-40"
        )}
      >
        {formatted}
      </pre>
    </div>
  );
}
