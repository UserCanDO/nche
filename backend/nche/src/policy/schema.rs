//! Tool schemas defining expected parameters and validation rules.
//!
//! Each semantic tool has a schema that:
//! - Documents expected parameters
//! - Provides validation
//! - Extracts typed data for policy evaluation


/// Defines the schema for a semantic tool
#[derive(Debug, Clone)]
pub struct ToolSchema {
    pub name: &'static str,
    pub description: &'static str,
    pub fields: &'static [FieldDef],
}

/// Field definition within a tool schema
#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: &'static str,
    pub field_type: FieldType,
    pub required: bool,
    pub description: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FieldType {
    String,
    Integer,
    Boolean,
    Array,
    Object,
}

impl ToolSchema {
    /// Validate params against the schema
    pub fn validate(&self, params: &serde_json::Value) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        for field in self.fields {
            if field.required {
                if params.get(field.name).is_none() {
                    errors.push(format!("Missing required field: {}", field.name));
                }
            }

            if let Some(value) = params.get(field.name) {
                let type_ok = match field.field_type {
                    FieldType::String => value.is_string(),
                    FieldType::Integer => value.is_i64() || value.is_u64(),
                    FieldType::Boolean => value.is_boolean(),
                    FieldType::Array => value.is_array(),
                    FieldType::Object => value.is_object(),
                };

                if !type_ok && !value.is_null() {
                    errors.push(format!(
                        "Field '{}' expected {:?}, got {:?}",
                        field.name, field.field_type, value
                    ));
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

// === Communication Tools ===

pub static EMAIL_SEND: ToolSchema = ToolSchema {
    name: "email_send",
    description: "Send an email message",
    fields: &[
        FieldDef { name: "to", field_type: FieldType::String, required: true, description: "Recipient email address" },
        FieldDef { name: "cc", field_type: FieldType::String, required: false, description: "CC recipients" },
        FieldDef { name: "bcc", field_type: FieldType::String, required: false, description: "BCC recipients" },
        FieldDef { name: "subject", field_type: FieldType::String, required: true, description: "Email subject line" },
        FieldDef { name: "body", field_type: FieldType::String, required: true, description: "Email body content" },
        FieldDef { name: "reply_to", field_type: FieldType::String, required: false, description: "Reply-to address" },
    ],
};

pub static SLACK_MESSAGE: ToolSchema = ToolSchema {
    name: "slack_message",
    description: "Send a Slack message",
    fields: &[
        FieldDef { name: "channel", field_type: FieldType::String, required: true, description: "Slack channel ID or name" },
        FieldDef { name: "text", field_type: FieldType::String, required: true, description: "Message text" },
        FieldDef { name: "thread_ts", field_type: FieldType::String, required: false, description: "Thread timestamp for replies" },
    ],
};

pub static SMS_SEND: ToolSchema = ToolSchema {
    name: "sms_send",
    description: "Send an SMS message",
    fields: &[
        FieldDef { name: "to", field_type: FieldType::String, required: true, description: "Phone number (E.164 format)" },
        FieldDef { name: "body", field_type: FieldType::String, required: true, description: "SMS message body" },
    ],
};

pub static NOTIFICATION_PUSH: ToolSchema = ToolSchema {
    name: "notification_push",
    description: "Send a push notification",
    fields: &[
        FieldDef { name: "user_id", field_type: FieldType::String, required: true, description: "Target user ID" },
        FieldDef { name: "title", field_type: FieldType::String, required: true, description: "Notification title" },
        FieldDef { name: "body", field_type: FieldType::String, required: true, description: "Notification body" },
        FieldDef { name: "data", field_type: FieldType::Object, required: false, description: "Additional data payload" },
    ],
};

// === HTTP/API Tools ===

pub static HTTP_REQUEST: ToolSchema = ToolSchema {
    name: "http_request",
    description: "Make an HTTP request",
    fields: &[
        FieldDef { name: "method", field_type: FieldType::String, required: true, description: "HTTP method (GET, POST, etc.)" },
        FieldDef { name: "url", field_type: FieldType::String, required: true, description: "Request URL" },
        FieldDef { name: "headers", field_type: FieldType::Object, required: false, description: "Request headers" },
        FieldDef { name: "body", field_type: FieldType::Object, required: false, description: "Request body" },
    ],
};

pub static GRAPHQL_EXECUTE: ToolSchema = ToolSchema {
    name: "graphql_execute",
    description: "Execute a GraphQL query or mutation",
    fields: &[
        FieldDef { name: "endpoint", field_type: FieldType::String, required: true, description: "GraphQL endpoint URL" },
        FieldDef { name: "query", field_type: FieldType::String, required: true, description: "GraphQL query/mutation" },
        FieldDef { name: "variables", field_type: FieldType::Object, required: false, description: "Query variables" },
    ],
};

// === Calendar Tools ===

pub static CALENDAR_EVENT_CREATE: ToolSchema = ToolSchema {
    name: "calendar_event_create",
    description: "Create a calendar event",
    fields: &[
        FieldDef { name: "title", field_type: FieldType::String, required: true, description: "Event title" },
        FieldDef { name: "start", field_type: FieldType::String, required: true, description: "Start time (ISO 8601)" },
        FieldDef { name: "end", field_type: FieldType::String, required: true, description: "End time (ISO 8601)" },
        FieldDef { name: "attendees", field_type: FieldType::Array, required: true, description: "List of attendee emails" },
        FieldDef { name: "description", field_type: FieldType::String, required: false, description: "Event description" },
    ],
};

pub static CALENDAR_EVENT_CANCEL: ToolSchema = ToolSchema {
    name: "calendar_event_cancel",
    description: "Cancel a calendar event",
    fields: &[
        FieldDef { name: "event_id", field_type: FieldType::String, required: true, description: "Event ID to cancel" },
        FieldDef { name: "notify_attendees", field_type: FieldType::Boolean, required: true, description: "Whether to notify attendees" },
    ],
};

// === File Tools ===

pub static FILE_UPLOAD: ToolSchema = ToolSchema {
    name: "file_upload",
    description: "Upload a file to storage",
    fields: &[
        FieldDef { name: "bucket", field_type: FieldType::String, required: true, description: "Storage bucket name" },
        FieldDef { name: "path", field_type: FieldType::String, required: true, description: "File path within bucket" },
        FieldDef { name: "content_type", field_type: FieldType::String, required: true, description: "MIME type" },
        FieldDef { name: "size_bytes", field_type: FieldType::Integer, required: true, description: "File size in bytes" },
    ],
};

pub static FILE_DELETE: ToolSchema = ToolSchema {
    name: "file_delete",
    description: "Delete a file from storage",
    fields: &[
        FieldDef { name: "bucket", field_type: FieldType::String, required: true, description: "Storage bucket name" },
        FieldDef { name: "path", field_type: FieldType::String, required: true, description: "File path to delete" },
    ],
};

// === Database Tools ===

pub static DATABASE_QUERY: ToolSchema = ToolSchema {
    name: "database_query",
    description: "Execute a database query",
    fields: &[
        FieldDef { name: "connection_id", field_type: FieldType::String, required: true, description: "Database connection identifier" },
        FieldDef { name: "query", field_type: FieldType::String, required: true, description: "SQL query to execute" },
        FieldDef { name: "params", field_type: FieldType::Array, required: false, description: "Query parameters" },
    ],
};

// === Ticketing Tools ===

pub static TICKET_CREATE: ToolSchema = ToolSchema {
    name: "ticket_create",
    description: "Create a support ticket",
    fields: &[
        FieldDef { name: "project", field_type: FieldType::String, required: true, description: "Project identifier" },
        FieldDef { name: "type", field_type: FieldType::String, required: true, description: "Ticket type (bug, feature, etc.)" },
        FieldDef { name: "title", field_type: FieldType::String, required: true, description: "Ticket title" },
        FieldDef { name: "description", field_type: FieldType::String, required: true, description: "Ticket description" },
        FieldDef { name: "priority", field_type: FieldType::String, required: false, description: "Priority level" },
        FieldDef { name: "customer_visible", field_type: FieldType::Boolean, required: false, description: "Whether visible to customers" },
    ],
};

pub static TICKET_UPDATE: ToolSchema = ToolSchema {
    name: "ticket_update",
    description: "Update an existing ticket",
    fields: &[
        FieldDef { name: "ticket_id", field_type: FieldType::String, required: true, description: "Ticket ID to update" },
        FieldDef { name: "status", field_type: FieldType::String, required: false, description: "New status" },
        FieldDef { name: "assignee", field_type: FieldType::String, required: false, description: "New assignee" },
        FieldDef { name: "comment", field_type: FieldType::String, required: false, description: "Comment to add" },
    ],
};

pub static TICKET_REPLY: ToolSchema = ToolSchema {
    name: "ticket_reply",
    description: "Reply to a ticket",
    fields: &[
        FieldDef { name: "ticket_id", field_type: FieldType::String, required: true, description: "Ticket ID" },
        FieldDef { name: "body", field_type: FieldType::String, required: true, description: "Reply content" },
        FieldDef { name: "internal", field_type: FieldType::Boolean, required: true, description: "Internal note vs customer-facing" },
    ],
};

// === Financial Tools ===

pub static PAYMENT_CHARGE: ToolSchema = ToolSchema {
    name: "payment_charge",
    description: "Charge a payment",
    fields: &[
        FieldDef { name: "amount_cents", field_type: FieldType::Integer, required: true, description: "Amount in cents" },
        FieldDef { name: "currency", field_type: FieldType::String, required: true, description: "Currency code (USD, EUR, etc.)" },
        FieldDef { name: "customer_id", field_type: FieldType::String, required: true, description: "Customer identifier" },
        FieldDef { name: "description", field_type: FieldType::String, required: false, description: "Charge description" },
    ],
};

pub static INVOICE_SEND: ToolSchema = ToolSchema {
    name: "invoice_send",
    description: "Send an invoice",
    fields: &[
        FieldDef { name: "invoice_id", field_type: FieldType::String, required: true, description: "Invoice ID to send" },
        FieldDef { name: "recipient_email", field_type: FieldType::String, required: true, description: "Recipient email address" },
    ],
};

// === Document Tools ===

pub static DOCUMENT_SIGN_REQUEST: ToolSchema = ToolSchema {
    name: "document_sign_request",
    description: "Request document signatures",
    fields: &[
        FieldDef { name: "document_id", field_type: FieldType::String, required: true, description: "Document ID" },
        FieldDef { name: "signers", field_type: FieldType::Array, required: true, description: "List of signer emails" },
        FieldDef { name: "message", field_type: FieldType::String, required: false, description: "Message to signers" },
    ],
};

pub static FORM_SUBMIT: ToolSchema = ToolSchema {
    name: "form_submit",
    description: "Submit a form",
    fields: &[
        FieldDef { name: "form_id", field_type: FieldType::String, required: true, description: "Form identifier" },
        FieldDef { name: "fields", field_type: FieldType::Object, required: true, description: "Form field values" },
        FieldDef { name: "submit_to", field_type: FieldType::String, required: true, description: "Submission endpoint/system" },
    ],
};

// === Code/DevOps Tools ===

pub static GIT_ISSUE_CREATE: ToolSchema = ToolSchema {
    name: "git_issue_create",
    description: "Create a Git issue",
    fields: &[
        FieldDef { name: "repo", field_type: FieldType::String, required: true, description: "Repository (owner/repo)" },
        FieldDef { name: "title", field_type: FieldType::String, required: true, description: "Issue title" },
        FieldDef { name: "body", field_type: FieldType::String, required: true, description: "Issue body" },
        FieldDef { name: "labels", field_type: FieldType::Array, required: false, description: "Labels to apply" },
    ],
};

pub static GIT_PR_MERGE: ToolSchema = ToolSchema {
    name: "git_pr_merge",
    description: "Merge a pull request",
    fields: &[
        FieldDef { name: "repo", field_type: FieldType::String, required: true, description: "Repository (owner/repo)" },
        FieldDef { name: "pr_number", field_type: FieldType::Integer, required: true, description: "Pull request number" },
        FieldDef { name: "method", field_type: FieldType::String, required: false, description: "Merge method (merge, squash, rebase)" },
    ],
};

/// Get the schema for a tool by name
pub fn get_schema(tool_name: &str) -> Option<&'static ToolSchema> {
    match tool_name {
        "email_send" | "send_email" => Some(&EMAIL_SEND),
        "slack_message" => Some(&SLACK_MESSAGE),
        "sms_send" => Some(&SMS_SEND),
        "notification_push" => Some(&NOTIFICATION_PUSH),
        "http_request" => Some(&HTTP_REQUEST),
        "graphql_execute" => Some(&GRAPHQL_EXECUTE),
        "calendar_event_create" => Some(&CALENDAR_EVENT_CREATE),
        "calendar_event_cancel" => Some(&CALENDAR_EVENT_CANCEL),
        "file_upload" => Some(&FILE_UPLOAD),
        "file_delete" => Some(&FILE_DELETE),
        "database_query" => Some(&DATABASE_QUERY),
        "ticket_create" => Some(&TICKET_CREATE),
        "ticket_update" => Some(&TICKET_UPDATE),
        "ticket_reply" => Some(&TICKET_REPLY),
        "payment_charge" => Some(&PAYMENT_CHARGE),
        "invoice_send" => Some(&INVOICE_SEND),
        "document_sign_request" => Some(&DOCUMENT_SIGN_REQUEST),
        "form_submit" => Some(&FORM_SUBMIT),
        "git_issue_create" => Some(&GIT_ISSUE_CREATE),
        "git_pr_merge" => Some(&GIT_PR_MERGE),
        _ => None,
    }
}

/// Get all registered tool schemas
pub fn all_schemas() -> Vec<&'static ToolSchema> {
    vec![
        &EMAIL_SEND,
        &SLACK_MESSAGE,
        &SMS_SEND,
        &NOTIFICATION_PUSH,
        &HTTP_REQUEST,
        &GRAPHQL_EXECUTE,
        &CALENDAR_EVENT_CREATE,
        &CALENDAR_EVENT_CANCEL,
        &FILE_UPLOAD,
        &FILE_DELETE,
        &DATABASE_QUERY,
        &TICKET_CREATE,
        &TICKET_UPDATE,
        &TICKET_REPLY,
        &PAYMENT_CHARGE,
        &INVOICE_SEND,
        &DOCUMENT_SIGN_REQUEST,
        &FORM_SUBMIT,
        &GIT_ISSUE_CREATE,
        &GIT_PR_MERGE,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_email_schema_validation_valid() {
        let params = serde_json::json!({
            "to": "user@example.com",
            "subject": "Hello",
            "body": "World"
        });
        assert!(EMAIL_SEND.validate(&params).is_ok());
    }

    #[test]
    fn test_email_schema_validation_missing_required() {
        let params = serde_json::json!({
            "to": "user@example.com"
            // missing subject and body
        });
        let result = EMAIL_SEND.validate(&params);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.contains("subject")));
        assert!(errors.iter().any(|e| e.contains("body")));
    }

    #[test]
    fn test_payment_schema_validation() {
        let params = serde_json::json!({
            "amount_cents": 5000,
            "currency": "USD",
            "customer_id": "cus_123"
        });
        assert!(PAYMENT_CHARGE.validate(&params).is_ok());
    }

    #[test]
    fn test_payment_schema_wrong_type() {
        let params = serde_json::json!({
            "amount_cents": "not a number",  // should be integer
            "currency": "USD",
            "customer_id": "cus_123"
        });
        let result = PAYMENT_CHARGE.validate(&params);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_schema_known_tool() {
        assert!(get_schema("email_send").is_some());
        assert!(get_schema("payment_charge").is_some());
        assert!(get_schema("git_pr_merge").is_some());
    }

    #[test]
    fn test_get_schema_unknown_tool() {
        assert!(get_schema("unknown_tool").is_none());
    }

    #[test]
    fn test_all_schemas_count() {
        assert_eq!(all_schemas().len(), 20);
    }

    #[test]
    fn test_file_upload_schema() {
        let params = serde_json::json!({
            "bucket": "my-bucket",
            "path": "/uploads/file.txt",
            "content_type": "text/plain",
            "size_bytes": 1024
        });
        assert!(FILE_UPLOAD.validate(&params).is_ok());
    }

    #[test]
    fn test_ticket_create_schema() {
        let params = serde_json::json!({
            "project": "SUPPORT",
            "type": "bug",
            "title": "Something is broken",
            "description": "It doesn't work"
        });
        assert!(TICKET_CREATE.validate(&params).is_ok());
    }

    #[test]
    fn test_database_query_schema() {
        let params = serde_json::json!({
            "connection_id": "prod-db",
            "query": "SELECT * FROM users WHERE id = $1",
            "params": [123]
        });
        assert!(DATABASE_QUERY.validate(&params).is_ok());
    }
}
