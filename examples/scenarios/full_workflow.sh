#!/bin/bash
# =============================================================================
# NCHE Scenario: Full Workflow
# =============================================================================
# This script demonstrates a complete workflow with multiple actions:
# 1. HTTP GET request (auto-approved in supervised mode)
# 2. HTTP POST request (requires approval, approved)
# 3. Email to internal domain (auto-approved)
# 4. Email to external domain (requires approval, approved)
#
# Prerequisites:
#   - NCHE server running on localhost:3000
#   - API key set in NCHE_API_KEY environment variable
#   - Tenant configured with internal_domains (optional, for auto-approval)
#
# Usage:
#   export NCHE_API_KEY="nche_agt_xxx_yyy"
#   ./full_workflow.sh
# =============================================================================

set -e

# Configuration
BASE_URL="${NCHE_BASE_URL:-http://localhost:3000}"
API_KEY="${NCHE_API_KEY:-}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Helper functions
log_step() {
    echo -e "\n${BLUE}=== $1 ===${NC}"
}

log_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

log_info() {
    echo -e "${YELLOW}→ $1${NC}"
}

log_action() {
    echo -e "${CYAN}⚡ $1${NC}"
}

log_error() {
    echo -e "${RED}✗ $1${NC}"
    exit 1
}

# Check prerequisites
if [ -z "$API_KEY" ]; then
    log_error "NCHE_API_KEY environment variable is not set"
fi

# Check if server is running
if ! curl -s "$BASE_URL/health" > /dev/null 2>&1; then
    log_error "NCHE server is not running at $BASE_URL"
fi

log_success "Connected to NCHE server at $BASE_URL"

# =============================================================================
# Step 1: Create a supervised session
# =============================================================================
log_step "Step 1: Creating a supervised session"

SESSION_RESPONSE=$(curl -s -X POST "$BASE_URL/v1/sessions" \
    -H "Authorization: Bearer $API_KEY" \
    -H "Content-Type: application/json" \
    -d '{
        "actor_id": "workflow_agent",
        "actor_type": "agent",
        "autonomy_level": "supervised"
    }')

SESSION_ID=$(echo "$SESSION_RESPONSE" | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)

if [ -z "$SESSION_ID" ]; then
    log_error "Failed to create session: $SESSION_RESPONSE"
fi

log_success "Created session: $SESSION_ID"
log_info "Autonomy level: supervised"

# =============================================================================
# Step 2: HTTP GET request (auto-approved in supervised mode)
# =============================================================================
log_step "Step 2: HTTP GET Request (Auto-Approved)"

log_action "Proposing HTTP GET request to fetch data..."

ACTION1_RESPONSE=$(curl -s -X POST "$BASE_URL/v1/actions" \
    -H "Authorization: Bearer $API_KEY" \
    -H "Content-Type: application/json" \
    -d "{
        \"session_id\": \"$SESSION_ID\",
        \"tool\": \"http_request\",
        \"params\": {
            \"method\": \"GET\",
            \"url\": \"https://api.example.com/data\"
        }
    }")

ACTION1_ID=$(echo "$ACTION1_RESPONSE" | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
ACTION1_STATE=$(echo "$ACTION1_RESPONSE" | grep -o '"state":"[^"]*"' | cut -d'"' -f4)

if [ -z "$ACTION1_ID" ]; then
    log_error "Failed to create action: $ACTION1_RESPONSE"
fi

log_success "Created action: $ACTION1_ID"
log_info "Action state: $ACTION1_STATE"

if [ "$ACTION1_STATE" = "ready_to_execute" ] || [ "$ACTION1_STATE" = "executing" ] || [ "$ACTION1_STATE" = "executed" ]; then
    log_success "GET request was auto-approved (safe read-only operation)"
fi

# =============================================================================
# Step 3: HTTP POST request (requires approval)
# =============================================================================
log_step "Step 3: HTTP POST Request (Requires Approval)"

log_action "Proposing HTTP POST request to submit data..."

ACTION2_RESPONSE=$(curl -s -X POST "$BASE_URL/v1/actions" \
    -H "Authorization: Bearer $API_KEY" \
    -H "Content-Type: application/json" \
    -d "{
        \"session_id\": \"$SESSION_ID\",
        \"tool\": \"http_request\",
        \"params\": {
            \"method\": \"POST\",
            \"url\": \"https://api.example.com/submit\",
            \"body\": {\"data\": \"important information\"}
        }
    }")

ACTION2_ID=$(echo "$ACTION2_RESPONSE" | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
ACTION2_STATE=$(echo "$ACTION2_RESPONSE" | grep -o '"state":"[^"]*"' | cut -d'"' -f4)

if [ -z "$ACTION2_ID" ]; then
    log_error "Failed to create action: $ACTION2_RESPONSE"
fi

log_success "Created action: $ACTION2_ID"
log_info "Action state: $ACTION2_STATE"

if [ "$ACTION2_STATE" = "paused_for_approval" ]; then
    log_info "POST request requires approval (mutating operation)"

    # Get approval and approve it
    sleep 1
    APPROVAL2_RESPONSE=$(curl -s -X GET "$BASE_URL/v1/approvals?action_id=$ACTION2_ID" \
        -H "Authorization: Bearer $API_KEY")

    APPROVAL2_ID=$(echo "$APPROVAL2_RESPONSE" | grep -o '"id":"appr_[^"]*"' | head -1 | cut -d'"' -f4)

    if [ -n "$APPROVAL2_ID" ]; then
        log_info "Approving HTTP POST request..."

        curl -s -X PATCH "$BASE_URL/v1/approvals/$APPROVAL2_ID" \
            -H "Authorization: Bearer $API_KEY" \
            -H "Content-Type: application/json" \
            -d '{
                "decision": "approved",
                "decided_by": "human_operator",
                "note": "Verified API endpoint and data payload. Safe to proceed."
            }' > /dev/null

        log_success "HTTP POST request approved"
    fi
fi

# =============================================================================
# Step 4: Email to internal domain (if configured, auto-approved)
# =============================================================================
log_step "Step 4: Email to Internal Domain"

log_action "Proposing email to internal address..."

ACTION3_RESPONSE=$(curl -s -X POST "$BASE_URL/v1/actions" \
    -H "Authorization: Bearer $API_KEY" \
    -H "Content-Type: application/json" \
    -d "{
        \"session_id\": \"$SESSION_ID\",
        \"tool\": \"send_email\",
        \"params\": {
            \"to\": \"team@internal.company.com\",
            \"subject\": \"Workflow Update: Task Completed\",
            \"body\": \"The automated workflow has completed the data submission task successfully.\"
        }
    }")

ACTION3_ID=$(echo "$ACTION3_RESPONSE" | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
ACTION3_STATE=$(echo "$ACTION3_RESPONSE" | grep -o '"state":"[^"]*"' | cut -d'"' -f4)

if [ -z "$ACTION3_ID" ]; then
    log_error "Failed to create action: $ACTION3_RESPONSE"
fi

log_success "Created action: $ACTION3_ID"
log_info "Action state: $ACTION3_STATE"

if [ "$ACTION3_STATE" = "ready_to_execute" ] || [ "$ACTION3_STATE" = "executing" ] || [ "$ACTION3_STATE" = "executed" ]; then
    log_success "Internal email was auto-approved"
elif [ "$ACTION3_STATE" = "paused_for_approval" ]; then
    log_info "Internal email requires approval (internal_domains not configured)"

    # Approve it
    sleep 1
    APPROVAL3_RESPONSE=$(curl -s -X GET "$BASE_URL/v1/approvals?action_id=$ACTION3_ID" \
        -H "Authorization: Bearer $API_KEY")

    APPROVAL3_ID=$(echo "$APPROVAL3_RESPONSE" | grep -o '"id":"appr_[^"]*"' | head -1 | cut -d'"' -f4)

    if [ -n "$APPROVAL3_ID" ]; then
        curl -s -X PATCH "$BASE_URL/v1/approvals/$APPROVAL3_ID" \
            -H "Authorization: Bearer $API_KEY" \
            -H "Content-Type: application/json" \
            -d '{
                "decision": "approved",
                "decided_by": "human_operator",
                "note": "Verified internal email. Safe to send."
            }' > /dev/null

        log_success "Internal email approved"
    fi
fi

# =============================================================================
# Step 5: Email to external domain (requires approval)
# =============================================================================
log_step "Step 5: Email to External Domain (Requires Approval)"

log_action "Proposing email to external address..."

ACTION4_RESPONSE=$(curl -s -X POST "$BASE_URL/v1/actions" \
    -H "Authorization: Bearer $API_KEY" \
    -H "Content-Type: application/json" \
    -d "{
        \"session_id\": \"$SESSION_ID\",
        \"tool\": \"send_email\",
        \"params\": {
            \"to\": \"partner@external-company.com\",
            \"subject\": \"Partnership Update\",
            \"body\": \"Dear Partner,\\n\\nWe wanted to update you on the progress of our joint project. Everything is on track.\\n\\nBest regards,\\nThe Team\"
        }
    }")

ACTION4_ID=$(echo "$ACTION4_RESPONSE" | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
ACTION4_STATE=$(echo "$ACTION4_RESPONSE" | grep -o '"state":"[^"]*"' | cut -d'"' -f4)

if [ -z "$ACTION4_ID" ]; then
    log_error "Failed to create action: $ACTION4_RESPONSE"
fi

log_success "Created action: $ACTION4_ID"
log_info "Action state: $ACTION4_STATE"

if [ "$ACTION4_STATE" = "paused_for_approval" ]; then
    log_info "External email requires approval"

    # Get approval and approve it
    sleep 1
    APPROVAL4_RESPONSE=$(curl -s -X GET "$BASE_URL/v1/approvals?action_id=$ACTION4_ID" \
        -H "Authorization: Bearer $API_KEY")

    APPROVAL4_ID=$(echo "$APPROVAL4_RESPONSE" | grep -o '"id":"appr_[^"]*"' | head -1 | cut -d'"' -f4)

    if [ -n "$APPROVAL4_ID" ]; then
        log_info "Approving external email..."

        curl -s -X PATCH "$BASE_URL/v1/approvals/$APPROVAL4_ID" \
            -H "Authorization: Bearer $API_KEY" \
            -H "Content-Type: application/json" \
            -d '{
                "decision": "approved",
                "decided_by": "human_operator",
                "note": "Verified partner email and content. Approved for sending."
            }' > /dev/null

        log_success "External email approved"
    fi
fi

# =============================================================================
# Step 6: Wait for execution and check final states
# =============================================================================
log_step "Step 6: Checking Final Action States"

sleep 2  # Give executor time to process

echo ""
echo "Action Results:"
echo "---------------"

# Check each action
for ACTION_ID in "$ACTION1_ID" "$ACTION2_ID" "$ACTION3_ID" "$ACTION4_ID"; do
    FINAL_ACTION=$(curl -s -X GET "$BASE_URL/v1/actions/$ACTION_ID" \
        -H "Authorization: Bearer $API_KEY")

    FINAL_STATE=$(echo "$FINAL_ACTION" | grep -o '"state":"[^"]*"' | cut -d'"' -f4)
    TOOL=$(echo "$FINAL_ACTION" | grep -o '"tool":"[^"]*"' | cut -d'"' -f4)

    case "$FINAL_STATE" in
        "executed")
            echo -e "  ${GREEN}✓${NC} $ACTION_ID ($TOOL): executed"
            ;;
        "ready_to_execute"|"executing")
            echo -e "  ${YELLOW}○${NC} $ACTION_ID ($TOOL): $FINAL_STATE"
            ;;
        "failed")
            echo -e "  ${RED}✗${NC} $ACTION_ID ($TOOL): failed"
            ;;
        *)
            echo -e "  ${YELLOW}?${NC} $ACTION_ID ($TOOL): $FINAL_STATE"
            ;;
    esac
done

# =============================================================================
# Step 7: End the session
# =============================================================================
log_step "Step 7: Ending the session"

curl -s -X DELETE "$BASE_URL/v1/sessions/$SESSION_ID" \
    -H "Authorization: Bearer $API_KEY" > /dev/null

log_success "Session ended"

# =============================================================================
# Summary
# =============================================================================
echo ""
echo -e "${GREEN}==============================================================================${NC}"
echo -e "${GREEN}Scenario Complete: Full Workflow${NC}"
echo -e "${GREEN}==============================================================================${NC}"
echo ""
echo "Summary:"
echo "  - Session ID: $SESSION_ID"
echo ""
echo "Actions executed:"
echo "  1. HTTP GET  (Action: $ACTION1_ID) - Auto-approved (read-only)"
echo "  2. HTTP POST (Action: $ACTION2_ID) - Required approval"
echo "  3. Internal Email (Action: $ACTION3_ID) - Auto-approved if internal_domains configured"
echo "  4. External Email (Action: $ACTION4_ID) - Required approval"
echo ""
echo "This scenario demonstrated:"
echo "  1. Creating a supervised session"
echo "  2. HTTP GET auto-approval (safe read-only operation)"
echo "  3. HTTP POST requiring approval (mutating operation)"
echo "  4. Internal email handling (auto-approve if configured)"
echo "  5. External email requiring approval"
echo "  6. Action execution after approval"
echo ""
echo "Policy behavior in supervised mode:"
echo "  - http_request GET:  Auto-approved"
echo "  - http_request POST: Requires approval"
echo "  - send_email internal: Auto-approved (if internal_domains configured)"
echo "  - send_email external: Requires approval"
echo ""
