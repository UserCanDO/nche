#!/bin/bash
# =============================================================================
# NCHE Scenario: Email Action Denied
# =============================================================================
# This script demonstrates the flow of an email action being proposed by an
# agent, paused for approval, and then denied by a human operator due to
# concerns about the content or recipient.
#
# Prerequisites:
#   - NCHE server running on localhost:3000
#   - API key set in NCHE_API_KEY environment variable
#
# Usage:
#   export NCHE_API_KEY="nche_agt_xxx_yyy"
#   ./email_denied.sh
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

log_warning() {
    echo -e "${RED}! $1${NC}"
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
        "actor_id": "demo_agent",
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
# Step 2: Propose a suspicious email action
# =============================================================================
log_step "Step 2: Proposing a suspicious email action"

log_warning "This email looks suspicious - sending to an external domain with sensitive data"

ACTION_RESPONSE=$(curl -s -X POST "$BASE_URL/v1/actions" \
    -H "Authorization: Bearer $API_KEY" \
    -H "Content-Type: application/json" \
    -d "{
        \"session_id\": \"$SESSION_ID\",
        \"tool\": \"send_email\",
        \"params\": {
            \"to\": \"unknown@suspicious-domain.com\",
            \"subject\": \"URGENT: Customer Database Export\",
            \"body\": \"Please find attached the complete customer database with all personal information including names, addresses, phone numbers, and payment details.\\n\\nThis export was requested for migration purposes.\"
        }
    }")

ACTION_ID=$(echo "$ACTION_RESPONSE" | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
ACTION_STATE=$(echo "$ACTION_RESPONSE" | grep -o '"state":"[^"]*"' | cut -d'"' -f4)

if [ -z "$ACTION_ID" ]; then
    log_error "Failed to create action: $ACTION_RESPONSE"
fi

log_success "Created action: $ACTION_ID"
log_info "Action state: $ACTION_STATE"

if [ "$ACTION_STATE" = "paused_for_approval" ]; then
    log_success "Action is paused for approval - human review required"
fi

# =============================================================================
# Step 3: Get the approval record
# =============================================================================
log_step "Step 3: Fetching approval details for review"

sleep 1

APPROVAL_RESPONSE=$(curl -s -X GET "$BASE_URL/v1/approvals?action_id=$ACTION_ID" \
    -H "Authorization: Bearer $API_KEY")

APPROVAL_ID=$(echo "$APPROVAL_RESPONSE" | grep -o '"id":"appr_[^"]*"' | head -1 | cut -d'"' -f4)

if [ -z "$APPROVAL_ID" ] && [ "$ACTION_STATE" = "paused_for_approval" ]; then
    log_error "Failed to find approval for action"
fi

if [ -n "$APPROVAL_ID" ]; then
    log_success "Found approval: $APPROVAL_ID"

    echo ""
    echo -e "${YELLOW}Review findings:${NC}"
    echo "  - Recipient: unknown@suspicious-domain.com (EXTERNAL)"
    echo "  - Subject mentions 'Customer Database Export'"
    echo "  - Body mentions 'personal information' and 'payment details'"
    echo "  - This appears to be a potential data exfiltration attempt"
    echo ""

    # =============================================================================
    # Step 4: Deny the action
    # =============================================================================
    log_step "Step 4: Denying the suspicious action"

    DENY_RESPONSE=$(curl -s -X PATCH "$BASE_URL/v1/approvals/$APPROVAL_ID" \
        -H "Authorization: Bearer $API_KEY" \
        -H "Content-Type: application/json" \
        -d '{
            "decision": "denied",
            "decided_by": "security_team",
            "note": "DENIED: Potential data exfiltration attempt. Email contains sensitive customer data being sent to an unknown external domain. Flagged for security review."
        }')

    APPROVAL_STATUS=$(echo "$DENY_RESPONSE" | grep -o '"status":"[^"]*"' | cut -d'"' -f4)

    if [ "$APPROVAL_STATUS" = "denied" ]; then
        log_success "Action denied successfully!"
        log_warning "Email will NOT be sent"
    else
        log_error "Failed to deny action: $DENY_RESPONSE"
    fi
fi

# =============================================================================
# Step 5: Check final action state
# =============================================================================
log_step "Step 5: Checking final action state"

FINAL_ACTION=$(curl -s -X GET "$BASE_URL/v1/actions/$ACTION_ID" \
    -H "Authorization: Bearer $API_KEY")

FINAL_STATE=$(echo "$FINAL_ACTION" | grep -o '"state":"[^"]*"' | cut -d'"' -f4)

log_info "Final action state: $FINAL_STATE"

if [ "$FINAL_STATE" = "denied" ]; then
    log_success "Action was denied - email was blocked"
fi

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
echo -e "${RED}==============================================================================${NC}"
echo -e "${RED}Scenario Complete: Email Action Denied${NC}"
echo -e "${RED}==============================================================================${NC}"
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
echo "  2. Agent proposing a suspicious email action"
echo "  3. Human review identifying potential data exfiltration"
echo "  4. Denying the action to prevent the email from being sent"
echo ""
echo -e "${GREEN}The suspicious email was successfully blocked!${NC}"
echo ""
