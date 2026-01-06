import { createApi, fetchBaseQuery } from "@reduxjs/toolkit/query/react";
import type {
  LoginRequest,
  LoginResponse,
  DashboardUser,
  DashboardStats,
  ApprovalWithAction,
  ApprovalDetail,
  ApprovalDecisionRequest,
  Action,
  ActionDetail,
  Event,
  ApprovalsFilter,
  ActionsFilter,
  EventsFilter,
  PaginatedResponse,
  TenantConfig,
  UpdateTenantConfigRequest,
} from "@/types";

// Helper to get session ID from cookie for CSRF protection
function getSessionIdFromCookie(): string | null {
  if (typeof document === "undefined") return null;
  const match = document.cookie.match(/nche_session=([^;]+)/);
  return match ? match[1] : null;
}

// Custom baseQuery that handles 401 by redirecting to login
// Use empty baseUrl for same-origin requests (embedded dashboard)
const baseQuery = fetchBaseQuery({
  baseUrl: process.env.NEXT_PUBLIC_API_URL || "",
  credentials: "include", // Important for cookie-based auth
  prepareHeaders: (headers) => {
    // Add X-Session-Id header for CSRF protection on mutations
    const sessionId = getSessionIdFromCookie();
    if (sessionId) {
      headers.set("X-Session-Id", sessionId);
    }
    return headers;
  },
});

const baseQueryWithAuth: typeof baseQuery = async (args, api, extraOptions) => {
  const result = await baseQuery(args, api, extraOptions);

  // On 401, redirect to login
  if (result.error?.status === 401) {
    if (typeof window !== "undefined" && !window.location.pathname.includes("/login")) {
      window.location.href = "/login";
    }
  }

  return result;
};

export const api = createApi({
  reducerPath: "api",
  baseQuery: baseQueryWithAuth,
  tagTypes: ["Approval", "Action", "Event", "Stats", "User", "TenantConfig"],
  endpoints: (builder) => ({
    // ============ Auth ============
    login: builder.mutation<LoginResponse, LoginRequest>({
      query: (credentials) => ({
        url: "/dashboard/login",
        method: "POST",
        body: credentials,
      }),
      invalidatesTags: ["User", "Approval", "Action", "Stats"],
    }),

    logout: builder.mutation<void, void>({
      query: () => ({
        url: "/dashboard/api/logout",
        method: "POST",
      }),
      invalidatesTags: ["User", "Approval", "Action", "Stats"],
    }),

    getMe: builder.query<DashboardUser, void>({
      query: () => "/dashboard/api/me",
      providesTags: ["User"],
    }),

    // ============ Approvals ============
    getApprovals: builder.query<ApprovalWithAction[], ApprovalsFilter | void>({
      query: (filters) => {
        const params = new URLSearchParams();
        if (filters?.status) params.append("status", filters.status);
        if (filters?.tool) params.append("tool", filters.tool);
        if (filters?.limit) params.append("limit", String(filters.limit));
        const queryString = params.toString();
        return `/dashboard/api/approvals${queryString ? `?${queryString}` : ""}`;
      },
      transformResponse: (response: PaginatedResponse<ApprovalWithAction>) => response.data,
      providesTags: (result) =>
        result
          ? [
              ...result.map(({ id }) => ({ type: "Approval" as const, id })),
              { type: "Approval", id: "LIST" },
            ]
          : [{ type: "Approval", id: "LIST" }],
    }),

    getApproval: builder.query<ApprovalDetail, string>({
      query: (id) => `/dashboard/api/approvals/${id}`,
      providesTags: (result, error, id) => [{ type: "Approval", id }],
    }),

    decideApproval: builder.mutation<
      { approval: ApprovalWithAction; new_state: string },
      { id: string; decision: ApprovalDecisionRequest }
    >({
      query: ({ id, decision }) => ({
        url: `/dashboard/api/approvals/${id}`,
        method: "PATCH",
        body: decision,
      }),
      invalidatesTags: (result, error, { id }) => [
        { type: "Approval", id },
        { type: "Approval", id: "LIST" },
        { type: "Action", id: "LIST" },
        { type: "Stats" },
      ],
    }),

    // ============ Actions ============
    getActions: builder.query<Action[], ActionsFilter | void>({
      query: (filters) => {
        const params = new URLSearchParams();
        if (filters?.session_id) params.append("session_id", filters.session_id);
        if (filters?.state) params.append("state", filters.state);
        if (filters?.limit) params.append("limit", String(filters.limit));
        if (filters?.offset) params.append("offset", String(filters.offset));
        const queryString = params.toString();
        return `/dashboard/api/actions${queryString ? `?${queryString}` : ""}`;
      },
      transformResponse: (response: PaginatedResponse<Action>) => response.data,
      providesTags: (result) =>
        result
          ? [
              ...result.map(({ id }) => ({ type: "Action" as const, id })),
              { type: "Action", id: "LIST" },
            ]
          : [{ type: "Action", id: "LIST" }],
    }),

    getAction: builder.query<ActionDetail, string>({
      query: (id) => `/dashboard/api/actions/${id}`,
      providesTags: (result, error, id) => [
        { type: "Action", id },
        { type: "Event", id: `action-${id}` },
      ],
    }),

    // ============ Events / Audit ============
    getEvents: builder.query<Event[], EventsFilter | void>({
      query: (filters) => {
        const params = new URLSearchParams();
        if (filters?.action_id) params.append("action_id", filters.action_id);
        if (filters?.session_id) params.append("session_id", filters.session_id);
        if (filters?.event_type) params.append("event_type", filters.event_type);
        if (filters?.limit) params.append("limit", String(filters.limit));
        const queryString = params.toString();
        return `/dashboard/api/events${queryString ? `?${queryString}` : ""}`;
      },
      transformResponse: (response: PaginatedResponse<Event>) => response.data,
      providesTags: [{ type: "Event", id: "LIST" }],
    }),

    // ============ Stats ============
    getStats: builder.query<DashboardStats, void>({
      query: () => "/dashboard/api/stats",
      providesTags: ["Stats"],
    }),

    // ============ Tenant Config ============
    getTenantConfig: builder.query<TenantConfig, void>({
      query: () => "/dashboard/api/tenant/config",
      providesTags: ["TenantConfig"],
    }),

    updateTenantConfig: builder.mutation<TenantConfig, UpdateTenantConfigRequest>({
      query: (config) => ({
        url: "/dashboard/api/tenant/config",
        method: "PATCH",
        body: config,
      }),
      invalidatesTags: ["TenantConfig"],
    }),
  }),
});

export const {
  // Auth
  useLoginMutation,
  useLogoutMutation,
  useGetMeQuery,
  // Approvals
  useGetApprovalsQuery,
  useGetApprovalQuery,
  useDecideApprovalMutation,
  // Actions
  useGetActionsQuery,
  useGetActionQuery,
  // Events
  useGetEventsQuery,
  // Stats
  useGetStatsQuery,
  // Tenant Config
  useGetTenantConfigQuery,
  useUpdateTenantConfigMutation,
} = api;
