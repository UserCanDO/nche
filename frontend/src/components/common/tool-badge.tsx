import { Badge } from "@/components/ui/badge";

const KNOWN_TOOLS = ["send_email", "http_request"];

interface ToolBadgeProps {
  tool: string;
}

export function ToolBadge({ tool }: ToolBadgeProps) {
  const isKnown = KNOWN_TOOLS.includes(tool);

  return (
    <Badge variant={isKnown ? "outline" : "destructive"}>
      {tool}
      {!isKnown && " (unknown)"}
    </Badge>
  );
}
