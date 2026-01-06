#!/usr/bin/env python3
"""
NCHE Agent Example - Anthropic Claude

This example demonstrates how to build an AI agent using Anthropic's Claude
that executes actions through NCHE for human oversight.

Requirements:
    pip install anthropic requests

Usage:
    export ANTHROPIC_API_KEY=your_api_key
    export NCHE_API_KEY=your_nche_api_key
    python agent_anthropic.py
"""

import json
import os
from anthropic import Anthropic
from nche_client import NcheClient

# Configuration
NCHE_URL = os.getenv("NCHE_URL", "http://localhost:3000")
NCHE_API_KEY = os.getenv("NCHE_API_KEY")
ANTHROPIC_API_KEY = os.getenv("ANTHROPIC_API_KEY")

# Define tools that the agent can use (these map to NCHE tools)
TOOLS = [
    {
        "name": "send_email",
        "description": "Send an email to a recipient. Use this when you need to communicate via email.",
        "input_schema": {
            "type": "object",
            "properties": {
                "to": {
                    "type": "string",
                    "description": "Email address of the recipient",
                },
                "subject": {
                    "type": "string",
                    "description": "Subject line of the email",
                },
                "body": {
                    "type": "string",
                    "description": "Body content of the email",
                },
            },
            "required": ["to", "subject", "body"],
        },
    },
    {
        "name": "http_request",
        "description": "Make an HTTP request to an API endpoint.",
        "input_schema": {
            "type": "object",
            "properties": {
                "method": {
                    "type": "string",
                    "enum": ["GET", "POST", "PUT", "DELETE"],
                    "description": "HTTP method",
                },
                "url": {
                    "type": "string",
                    "description": "URL to request",
                },
                "headers": {
                    "type": "object",
                    "description": "HTTP headers",
                },
                "body": {
                    "type": "string",
                    "description": "Request body (for POST/PUT)",
                },
            },
            "required": ["method", "url"],
        },
    },
]


def execute_tool_via_nche(nche: NcheClient, tool_name: str, tool_input: dict) -> str:
    """Execute a tool through NCHE and return the result."""
    print(f"\n[NCHE] Proposing action: {tool_name}")
    print(f"[NCHE] Parameters: {json.dumps(tool_input, indent=2)}")

    action = nche.execute_tool(tool_name, tool_input, wait=True, timeout=120)

    print(f"[NCHE] Action state: {action.state}")

    if action.state == "executed":
        return json.dumps(action.result or {"status": "success"})
    elif action.state == "denied":
        return json.dumps({"error": "Action was denied by human reviewer"})
    elif action.state == "failed":
        return json.dumps({"error": action.error or "Action execution failed"})
    else:
        return json.dumps({"error": f"Unexpected state: {action.state}"})


def run_agent(user_message: str):
    """Run the agent with a user message."""
    if not NCHE_API_KEY:
        raise ValueError("NCHE_API_KEY environment variable is required")
    if not ANTHROPIC_API_KEY:
        raise ValueError("ANTHROPIC_API_KEY environment variable is required")

    # Initialize clients
    nche = NcheClient(NCHE_URL, NCHE_API_KEY)
    anthropic = Anthropic(api_key=ANTHROPIC_API_KEY)

    # Create NCHE session
    print("[NCHE] Creating session...")
    session = nche.create_session(
        actor_id="claude-agent",
        actor_type="agent",
        autonomy_level="supervised",  # Require human approval
    )
    print(f"[NCHE] Session created: {session.id}")

    try:
        # Initialize conversation
        messages = [{"role": "user", "content": user_message}]

        print(f"\n[User] {user_message}\n")

        # Agent loop
        while True:
            # Call Claude
            response = anthropic.messages.create(
                model="claude-sonnet-4-20250514",
                max_tokens=4096,
                system="""You are a helpful assistant that can send emails and make HTTP requests.
When you need to perform an action, use the provided tools.
All actions are reviewed by humans before execution for safety.""",
                tools=TOOLS,
                messages=messages,
            )

            # Check if we're done
            if response.stop_reason == "end_turn":
                # Extract final text response
                for block in response.content:
                    if hasattr(block, "text"):
                        print(f"[Claude] {block.text}")
                break

            # Process tool uses
            tool_uses = []
            text_content = []

            for block in response.content:
                if block.type == "tool_use":
                    tool_uses.append(block)
                elif hasattr(block, "text"):
                    text_content.append(block.text)

            # Print any text
            for text in text_content:
                print(f"[Claude] {text}")

            # Execute tools through NCHE
            if tool_uses:
                # Add assistant message with tool uses
                messages.append({"role": "assistant", "content": response.content})

                tool_results = []
                for tool_use in tool_uses:
                    result = execute_tool_via_nche(
                        nche, tool_use.name, tool_use.input
                    )
                    tool_results.append(
                        {
                            "type": "tool_result",
                            "tool_use_id": tool_use.id,
                            "content": result,
                        }
                    )

                # Add tool results
                messages.append({"role": "user", "content": tool_results})
            else:
                # No more tool calls, we're done
                break

    finally:
        # Clean up session
        print("\n[NCHE] Ending session...")
        nche.end_session()
        print("[NCHE] Session ended")


if __name__ == "__main__":
    # Example: Ask the agent to send an email
    run_agent(
        "Please send an email to team@example.com with subject 'Weekly Update' "
        "and body 'Hi team, here is this week's progress update. All tasks are on track.'"
    )
