#!/usr/bin/env python3
"""
NCHE Agent Example - OpenAI GPT

This example demonstrates how to build an AI agent using OpenAI's GPT
that executes actions through NCHE for human oversight.

Requirements:
    pip install openai requests

Usage:
    export OPENAI_API_KEY=your_api_key
    export NCHE_API_KEY=your_nche_api_key
    python agent_openai.py
"""

import json
import os
from openai import OpenAI
from nche_client import NcheClient

# Configuration
NCHE_URL = os.getenv("NCHE_URL", "http://localhost:3000")
NCHE_API_KEY = os.getenv("NCHE_API_KEY")
OPENAI_API_KEY = os.getenv("OPENAI_API_KEY")

# Define tools that the agent can use (these map to NCHE tools)
TOOLS = [
    {
        "type": "function",
        "function": {
            "name": "send_email",
            "description": "Send an email to a recipient. Use this when you need to communicate via email.",
            "parameters": {
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
    },
    {
        "type": "function",
        "function": {
            "name": "http_request",
            "description": "Make an HTTP request to an API endpoint.",
            "parameters": {
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
    if not OPENAI_API_KEY:
        raise ValueError("OPENAI_API_KEY environment variable is required")

    # Initialize clients
    nche = NcheClient(NCHE_URL, NCHE_API_KEY)
    openai = OpenAI(api_key=OPENAI_API_KEY)

    # Create NCHE session
    print("[NCHE] Creating session...")
    session = nche.create_session(
        actor_id="gpt-agent",
        actor_type="agent",
        autonomy_level="supervised",  # Require human approval
    )
    print(f"[NCHE] Session created: {session.id}")

    try:
        # Initialize conversation
        messages = [
            {
                "role": "system",
                "content": """You are a helpful assistant that can send emails and make HTTP requests.
When you need to perform an action, use the provided tools.
All actions are reviewed by humans before execution for safety.""",
            },
            {"role": "user", "content": user_message},
        ]

        print(f"\n[User] {user_message}\n")

        # Agent loop
        while True:
            # Call GPT
            response = openai.chat.completions.create(
                model="gpt-4o",
                messages=messages,
                tools=TOOLS,
                tool_choice="auto",
            )

            message = response.choices[0].message

            # Check if we're done (no tool calls)
            if not message.tool_calls:
                if message.content:
                    print(f"[GPT] {message.content}")
                break

            # Print any text content
            if message.content:
                print(f"[GPT] {message.content}")

            # Add assistant message to conversation
            messages.append(message)

            # Execute tools through NCHE
            for tool_call in message.tool_calls:
                tool_name = tool_call.function.name
                tool_input = json.loads(tool_call.function.arguments)

                result = execute_tool_via_nche(nche, tool_name, tool_input)

                # Add tool result to conversation
                messages.append(
                    {
                        "role": "tool",
                        "tool_call_id": tool_call.id,
                        "content": result,
                    }
                )

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
