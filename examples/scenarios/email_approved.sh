#!/bin/bash
# =============================================================================
# NCHE Scenario: Email Action Approved
# =============================================================================
# This script demonstrates the complete flow of an email action being proposed
# by an agent, paused for approval, and then approved by a human operator.
#
# Prerequisites:
#   - NCHE server running on localhost:3000
#   - API key set in NCHE_API_KEY environment variable
#
# Usage:
#   export NCHE_API_KEY="nche_agt_xxx_yyy"
#   ./email_approved.sh
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
# Step 1: Create a session
# =============================================================================
log_step "Step 1: Creating a session"

SESSION_RESPONSE=$(curl -s -X POST "$BASE_URL/v1/sessions" \
    -H "Authorization: Bearer $API_KEY" \
    -H "Content-Type: application/json" \
    -d '{
        "actor_id": "demo_user",
        "actor_type": "user",
        "autonomy_level": "supervised"
    }')

SESSION_ID=$(echo "$SESSION_RESPONSE" | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)

if [ -z "$SESSION_ID" ]; then
    log_error "Failed to create session: $SESSION_RESPONSE"
fi

log_success "Created session: $SESSION_ID"
log_info "Autonomy level: supervised (requires approval for emails)"

# =============================================================================
# Step 2: Propose an email action
# =============================================================================
log_step "Step 2: Proposing an email action"

ACTION_RESPONSE=$(curl -s -X POST "$BASE_URL/v1/actions" \
    -H "Authorization: Bearer $API_KEY" \
    -H "Content-Type: application/json" \
    -d "{
        \"session_id\": \"$SESSION_ID\",
        \"tool\": \"send_email\",
        \"params\": {
            \"to\": \"customer@example.com\",
            \"subject\": \"Your order has shipped!\",
            \"body\": \"Dear Customer,\\n\\nYour order #12345 has been shipped and will arrive in 3-5 business days.\\n\\nThank you for your purchase!\\n\\nBest regards,\\nThe Team\"
        }
    }")

ACTION_ID=$(echo "$ACTION_RESPONSE" | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
ACTION_STATE=$(echo "$ACTION_RESPONSE" | grep -o '"state":"[^"]*"' | cut -d'"' -f4)

if [ -z "$ACTION_ID" ]; then
    log_error "Failed to create action: $ACTION_RESPONSE"
fi

log_success "Created action: $ACTION_ID"
log_info "Action state: $ACTION_STATE"

# Check if action requires approval
if [ "$ACTION_STATE" != "paused_for_approval" ]; then
    log_info "Note: Action was auto-approved (state: $ACTION_STATE)"
    log_info "This happens when the session has 'full' autonomy level"
else
    log_success "Action is paused for approval as expected"
fi

# =============================================================================
# Step 3: Get the approval record
# =============================================================================
log_step "Step 3: Fetching approval details"

sleep 1  # Give the server a moment to create the approval

APPROVAL_RESPONSE=$(curl -s -X GET "$BASE_URL/v1/approvals?action_id=$ACTION_ID" \
    -H "Authorization: Bearer $API_KEY")

APPROVAL_ID=$(echo "$APPROVAL_RESPONSE" | grep -o '"id":"appr_[^"]*"' | head -1 | cut -d'"' -f4)

if [ -z "$APPROVAL_ID" ] && [ "$ACTION_STATE" = "paused_for_approval" ]; then
    log_error "Failed to find approval for action: $APPROVAL_RESPONSE"
fi

if [ -n "$APPROVAL_ID" ]; then
    log_success "Found approval: $APPROVAL_ID"

    # =============================================================================
    # Step 4: Approve the action
    # =============================================================================
    log_step "Step 4: Approving the action"

    APPROVE_RESPONSE=$(curl -s -X PATCH "$BASE_URL/v1/approvals/$APPROVAL_ID" \
        -H "Authorization: Bearer $API_KEY" \
        -H "Content-Type: application/json" \
        -d '{
            "decision": "approved",
            "decided_by": "human_operator",
            "note": "Verified customer email and order details. Safe to send."
        }')

    APPROVAL_STATUS=$(echo "$APPROVE_RESPONSE" | grep -o '"status":"[^"]*"' | cut -d'"' -f4)

    if [ "$APPROVAL_STATUS" = "approved" ]; then
        log_success "Action approved successfully!"
    else
        log_error "Failed to approve action: $APPROVE_RESPONSE"
    fi
fi

# =============================================================================
# Step 5: Check final action state
# =============================================================================
log_step "Step 5: Checking final action state"

sleep 2  # Give executor time to process

FINAL_ACTION=$(curl -s -X GET "$BASE_URL/v1/actions/$ACTION_ID" \
    -H "Authorization: Bearer $API_KEY")

FINAL_STATE=$(echo "$FINAL_ACTION" | grep -o '"state":"[^"]*"' | cut -d'"' -f4)

log_info "Final action state: $FINAL_STATE"

case "$FINAL_STATE" in
    "executed")
        log_success "Email was sent successfully!"
        ;;
    "ready_to_execute")
        log_info "Action is ready to execute (executor will process it shortly)"
        ;;
    "executing")
        log_info "Action is currently being executed"
        ;;
    *)
        log_info "Unexpected state: $FINAL_STATE"
        ;;
esac

# =============================================================================
# Step 6: End the session
# =============================================================================
log_step "Step 6: Ending the session"

curl -s -X DELETE "$BASE_URL/v1/sessions/$SESSION_ID" \
    -H "Authorization: Bearer $API_KEY" > /dev/null

log_success "Session ended"

# =============================================================================
# Summary
# =============================================================================
echo ""
echo -e "${GREEN}==============================================================================${NC}"
echo -e "${GREEN}Scenario Complete: Email Action Approved${NC}"
echo -e "${GREEN}==============================================================================${NC}"
echo ""
echo "Summary:"
echo "  - Session ID:  $SESSION_ID"
echo "  - Action ID:   $ACTION_ID"
if [ -n "$APPROVAL_ID" ]; then
echo "  - Approval ID: $APPROVAL_ID"
fi
echo "  - Final State: $FINAL_STATE"
echo ""
echo "This scenario demonstrated:"
echo "  1. Creating a supervised session"
echo "  2. Proposing an email action (requires approval)"
echo "  3. Approving the action via the API"
echo "  4. Action execution after approval"
echo ""
