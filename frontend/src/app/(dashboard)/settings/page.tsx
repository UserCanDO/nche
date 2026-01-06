"use client";

import { useState, useEffect } from "react";
import { useGetTenantConfigQuery, useUpdateTenantConfigMutation } from "@/lib/api";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { toast } from "sonner";

export default function SettingsPage() {
  const { data: config, isLoading, error } = useGetTenantConfigQuery();
  const [updateConfig, { isLoading: isSaving }] = useUpdateTenantConfigMutation();

  // Form state
  const [executionWebhookUrl, setExecutionWebhookUrl] = useState("");
  const [executionWebhookSecret, setExecutionWebhookSecret] = useState("");
  const [executionWebhookTimeoutMs, setExecutionWebhookTimeoutMs] = useState("30000");
  const [policyMode, setPolicyMode] = useState<"builtin" | "webhook">("builtin");
  const [policyWebhookUrl, setPolicyWebhookUrl] = useState("");
  const [policyWebhookSecret, setPolicyWebhookSecret] = useState("");
  const [policyWebhookTimeoutMs, setPolicyWebhookTimeoutMs] = useState("500");

  // Sync form state with loaded config
  useEffect(() => {
    if (config) {
      setExecutionWebhookUrl(config.execution_webhook_url ?? "");
      setExecutionWebhookTimeoutMs(String(config.execution_webhook_timeout_ms ?? 30000));
      setPolicyMode(config.policy_mode ?? "builtin");
      setPolicyWebhookUrl(config.policy_webhook_url ?? "");
      setPolicyWebhookTimeoutMs(String(config.policy_webhook_timeout_ms ?? 500));
    }
  }, [config]);

  const handleSave = async () => {
    try {
      await updateConfig({
        execution_webhook_url: executionWebhookUrl || undefined,
        execution_webhook_secret: executionWebhookSecret || undefined,
        execution_webhook_timeout_ms: executionWebhookTimeoutMs ? parseInt(executionWebhookTimeoutMs, 10) : undefined,
        policy_mode: policyMode,
        policy_webhook_url: policyMode === "webhook" ? policyWebhookUrl || undefined : undefined,
        policy_webhook_secret: policyMode === "webhook" ? policyWebhookSecret || undefined : undefined,
        policy_webhook_timeout_ms: policyMode === "webhook" && policyWebhookTimeoutMs ? parseInt(policyWebhookTimeoutMs, 10) : undefined,
      }).unwrap();

      // Clear secrets after successful save
      setExecutionWebhookSecret("");
      setPolicyWebhookSecret("");

      toast.success("Settings saved successfully");
    } catch {
      toast.error("Failed to save settings");
    }
  };

  if (error) {
    return (
      <div className="text-center py-12">
        <p className="text-red-600">Failed to load settings</p>
      </div>
    );
  }

  if (isLoading) {
    return (
      <div className="text-center py-12 text-gray-500">Loading...</div>
    );
  }

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold text-gray-900">Settings</h1>

      <Card>
        <CardHeader>
          <CardTitle>Execution Webhook</CardTitle>
          <CardDescription>
            Configure where Nche sends actions for execution after approval.
            If not configured, actions will remain in &quot;ready to execute&quot; state.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="execution-webhook-url">Webhook URL</Label>
            <Input
              id="execution-webhook-url"
              type="url"
              placeholder="https://your-app.com/nche/execute"
              value={executionWebhookUrl}
              onChange={(e) => setExecutionWebhookUrl(e.target.value)}
            />
            <p className="text-xs text-gray-500">
              Nche will POST action details to this URL for execution
            </p>
          </div>

          <div className="space-y-2">
            <Label htmlFor="execution-webhook-secret">Webhook Secret</Label>
            <Input
              id="execution-webhook-secret"
              type="password"
              placeholder="Enter new secret to update"
              value={executionWebhookSecret}
              onChange={(e) => setExecutionWebhookSecret(e.target.value)}
            />
            <p className="text-xs text-gray-500">
              Used to sign webhook payloads (HMAC-SHA256). Leave blank to keep existing secret.
            </p>
          </div>

          <div className="space-y-2">
            <Label htmlFor="execution-webhook-timeout">Timeout (ms)</Label>
            <Input
              id="execution-webhook-timeout"
              type="number"
              min="1000"
              max="120000"
              value={executionWebhookTimeoutMs}
              onChange={(e) => setExecutionWebhookTimeoutMs(e.target.value)}
            />
            <p className="text-xs text-gray-500">
              How long to wait for your webhook to acknowledge the request (1000-120000ms)
            </p>
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle>Policy Mode</CardTitle>
          <CardDescription>
            Choose how Nche evaluates action policies.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="policy-mode">Mode</Label>
            <Select value={policyMode} onValueChange={(v) => setPolicyMode(v as "builtin" | "webhook")}>
              <SelectTrigger id="policy-mode" className="w-48">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="builtin">Built-in Policies</SelectItem>
                <SelectItem value="webhook">Custom Webhook</SelectItem>
              </SelectContent>
            </Select>
            <p className="text-xs text-gray-500">
              {policyMode === "builtin"
                ? "Uses Nche's 20 semantic tool policies with configurable autonomy levels"
                : "Your webhook decides allow/deny/require_approval for each action"}
            </p>
          </div>

          {policyMode === "webhook" && (
            <>
              <div className="space-y-2">
                <Label htmlFor="policy-webhook-url">Policy Webhook URL</Label>
                <Input
                  id="policy-webhook-url"
                  type="url"
                  placeholder="https://your-app.com/nche/policy"
                  value={policyWebhookUrl}
                  onChange={(e) => setPolicyWebhookUrl(e.target.value)}
                />
                <p className="text-xs text-gray-500">
                  Nche will POST action details to this URL for policy evaluation
                </p>
              </div>

              <div className="space-y-2">
                <Label htmlFor="policy-webhook-secret">Policy Webhook Secret</Label>
                <Input
                  id="policy-webhook-secret"
                  type="password"
                  placeholder="Enter new secret to update"
                  value={policyWebhookSecret}
                  onChange={(e) => setPolicyWebhookSecret(e.target.value)}
                />
                <p className="text-xs text-gray-500">
                  Leave blank to keep existing secret
                </p>
              </div>

              <div className="space-y-2">
                <Label htmlFor="policy-webhook-timeout">Policy Timeout (ms)</Label>
                <Input
                  id="policy-webhook-timeout"
                  type="number"
                  min="100"
                  max="5000"
                  value={policyWebhookTimeoutMs}
                  onChange={(e) => setPolicyWebhookTimeoutMs(e.target.value)}
                />
                <p className="text-xs text-gray-500">
                  Policy evaluation timeout (100-5000ms). On timeout, defaults to require_approval.
                </p>
              </div>
            </>
          )}
        </CardContent>
      </Card>

      <div className="flex justify-end">
        <Button onClick={handleSave} disabled={isSaving}>
          {isSaving ? "Saving..." : "Save Settings"}
        </Button>
      </div>
    </div>
  );
}
