# Self-Hosted Mode (Builtin Execution)

Nche supports two execution modes:

1. **Webhook Mode** (default) - Nche delegates tool execution to your application
2. **Builtin Mode** - Nche executes tools directly (self-hosted, all-in-one)

This guide covers builtin mode for teams who want to run everything in one place.

## When to Use Builtin Mode

Use builtin mode if:
- You want a simpler deployment with fewer moving parts
- You're running Nche as a standalone service
- You don't need custom tool implementations
- You're prototyping or evaluating Nche

Use webhook mode if:
- You need custom tool implementations
- You want tools to run in your own infrastructure
- You have existing service integrations
- You need to control credentials and secrets

## Configuration

### Default Modes for New Tenants

In `nche.yaml`:

```yaml
defaults:
  execution_mode: builtin   # or "webhook"
  policy_mode: builtin      # or "webhook"
```

Or via environment variables:

```bash
DEFAULT_EXECUTION_MODE=builtin
DEFAULT_POLICY_MODE=builtin
```

### Per-Tenant Configuration

Override modes for specific tenants via the API:

```bash
# Via Agent API
PATCH /v1/tenant/config
Authorization: Bearer <api_key>
{
  "execution_webhook_url": null,
  "policy_mode": "builtin"
}

# Via Dashboard API
PATCH /dashboard/api/tenant/config
{
  "execution_webhook_url": null,
  "policy_mode": "builtin"
}
```

## Builtin Tool Implementations

In builtin mode, Nche needs tool implementations to be registered. By default, Nche includes:

### Communication Tools

Configure email provider in `nche.yaml`:

```yaml
tools:
  email:
    provider: sendgrid  # or "smtp", "ses", "mailgun"
    api_key: ${SENDGRID_API_KEY}
    from_address: noreply@yourcompany.com
```

### HTTP Tools

HTTP requests are executed directly by Nche:

```yaml
tools:
  http:
    timeout_ms: 30000
    max_redirects: 5
    allowed_hosts:  # Optional allowlist
      - api.yourcompany.com
      - *.internal.com
```

## Adding Custom Tool Implementations

To add custom tools in builtin mode, you'll need to modify the Nche source code.

### 1. Define the Tool Handler

Create a new file in `src/tools/`:

```rust
// src/tools/custom_tool.rs

use async_trait::async_trait;
use serde_json::Value;
use crate::error::Result;

pub struct CustomTool {
    // Configuration fields
}

impl CustomTool {
    pub fn new(config: &ToolConfig) -> Self {
        Self {
            // Initialize from config
        }
    }

    pub async fn execute(&self, params: Value) -> Result<Value> {
        // Implement your tool logic here
        let result = serde_json::json!({
            "status": "success",
            "data": {}
        });
        Ok(result)
    }
}
```

### 2. Register the Tool

Add your tool to the executor in `src/executor/mod.rs`:

```rust
impl Executor {
    async fn execute_tool(&self, action: &Action) -> Result<Value> {
        match action.tool.as_str() {
            "custom_tool" => {
                let tool = CustomTool::new(&self.config);
                tool.execute(action.params.clone()).await
            }
            // ... other tools
            _ => Err(NcheError::ToolExecution {
                message: format!("Unknown tool: {}", action.tool)
            })
        }
    }
}
```

### 3. Add Configuration

Update `src/config.rs` to include your tool's config:

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct ToolsConfig {
    pub custom_tool: Option<CustomToolConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CustomToolConfig {
    pub api_key: String,
    pub endpoint: String,
}
```

## Policy in Builtin Mode

When using `policy_mode: builtin`, Nche uses its built-in 20 semantic tool policies:

| Tool | Auto-Allow | Require Approval | Deny |
|------|------------|------------------|------|
| `email_send` | Internal domain | External domain | Blocked domains |
| `http_request` | GET requests | POST/PUT/DELETE | localhost |
| `payment_charge` | ≤$100 + full_autonomy | >$100 | - |
| `database_query` | SELECT | INSERT/UPDATE | DELETE/DROP |

See the full policy matrix in the [Policy Documentation](./POLICIES.md).

## Migration: Webhook to Builtin

To migrate a tenant from webhook to builtin mode:

1. Update the tenant configuration:
   ```bash
   PATCH /v1/tenant/config
   {
     "execution_webhook_url": null
   }
   ```

2. Ensure Nche has the necessary tool configurations in `nche.yaml`

3. Update your agent to remove any async result reporting (builtin mode is synchronous)

## Migration: Builtin to Webhook

To migrate from builtin to webhook mode:

1. Implement your webhook server (see [examples/python](../examples/python/) or [examples/nodejs](../examples/nodejs/))

2. Update the tenant configuration:
   ```bash
   PATCH /v1/tenant/config
   {
     "execution_webhook_url": "https://your-app.com/nche/execute",
     "execution_webhook_secret": "your-secret"
   }
   ```

3. Nche will automatically start sending execution webhooks

## Hybrid Mode

You can mix modes across tenants:
- Development tenant: `builtin` mode for quick testing
- Production tenant: `webhook` mode for custom integrations

Each tenant's mode is independent and configured separately.

## Security Considerations

### Builtin Mode
- Nche holds all credentials (API keys, secrets)
- All tool execution happens within Nche's process
- Audit logs include full execution details

### Webhook Mode
- Credentials stay in your infrastructure
- Tool execution is isolated from Nche
- Audit logs record tool parameters and results

## Troubleshooting

### "Tool not implemented" Error

In builtin mode, only registered tools are available. Check:
1. The tool name matches exactly
2. Tool implementation is compiled into Nche
3. Tool configuration is present in `nche.yaml`

### "Unknown execution mode" Error

Valid modes are:
- `builtin` - Nche executes tools directly
- `webhook` - Nche sends webhooks to your app

Check your `nche.yaml` or environment variables.

### Tool Execution Timeout

Increase the executor timeout:

```yaml
executor:
  tool_timeout_ms: 60000  # 60 seconds
```

Or configure per-tool timeouts in the tool configuration.
