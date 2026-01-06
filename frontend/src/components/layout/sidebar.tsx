"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { cn } from "@/lib/utils";
import { Badge } from "@/components/ui/badge";
import { useGetApprovalsQuery } from "@/lib/api";

interface NavItem {
  href: string;
  label: string;
  badge?: number;
}

export function Sidebar() {
  const pathname = usePathname();
  const { data: approvals } = useGetApprovalsQuery({ status: "pending" });
  const pendingCount = approvals?.length ?? 0;

  const navItems: NavItem[] = [
    { href: "/approvals", label: "Approvals", badge: pendingCount },
    { href: "/actions", label: "Actions" },
    { href: "/audit", label: "Audit" },
    { href: "/settings", label: "Settings" },
  ];

  return (
    <aside className="w-64 border-r bg-gray-50/50 min-h-screen">
      <div className="p-6">
        <Link href="/approvals" className="flex items-center gap-2">
          <span className="text-xl font-bold text-gray-900">Nche</span>
        </Link>
      </div>
      <nav className="px-4 space-y-1">
        {navItems.map((item) => {
          const isActive = pathname.startsWith(item.href);
          return (
            <Link
              key={item.href}
              href={item.href}
              className={cn(
                "flex items-center justify-between px-3 py-2 rounded-md text-sm font-medium transition-colors",
                isActive
                  ? "bg-gray-200 text-gray-900"
                  : "text-gray-600 hover:bg-gray-100 hover:text-gray-900"
              )}
            >
              <span>{item.label}</span>
              {item.badge !== undefined && item.badge > 0 && (
                <Badge variant="secondary" className="ml-auto">
                  {item.badge}
                </Badge>
              )}
            </Link>
          );
        })}
      </nav>
    </aside>
  );
}
