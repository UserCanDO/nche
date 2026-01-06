"""
NCHE Python Client

A simple client for interacting with the NCHE Agent Control Plane API.
"""

import time
from dataclasses import dataclass
from typing import Any, Optional
import requests


@dataclass
class Session:
    id: str
    tenant_id: str
    agent_id: str
    autonomy_level: str


@dataclass
class Action:
    id: str
    state: str
    tool: str
    params: dict
    result: Optional[dict] = None
    error: Optional[str] = None


class NcheClient:
    """Client for the NCHE Agent Control Plane API."""

    def __init__(self, base_url: str, api_key: str):
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key
        self.session: Optional[Session] = None

    def _headers(self) -> dict:
        return {
            "Authorization": f"Bearer {self.api_key}",
            "Content-Type": "application/json",
        }

    def create_session(
        self,
        actor_id: str,
        actor_type: str = "user",
        autonomy_level: str = "supervised",
    ) -> Session:
        """Create a new session."""
        resp = requests.post(
            f"{self.base_url}/v1/sessions",
            headers=self._headers(),
            json={
                "actor_id": actor_id,
                "actor_type": actor_type,
                "autonomy_level": autonomy_level,
            },
        )
        resp.raise_for_status()
        data = resp.json()
        self.session = Session(
            id=data["id"],
            tenant_id=data["tenant_id"],
            agent_id=data["agent_id"],
            autonomy_level=data["autonomy_level"],
        )
        return self.session

    def end_session(self) -> None:
        """End the current session."""
        if not self.session:
            raise ValueError("No active session")
        resp = requests.delete(
            f"{self.base_url}/v1/sessions/{self.session.id}",
            headers=self._headers(),
        )
        resp.raise_for_status()
        self.session = None

    def propose_action(self, tool: str, params: dict) -> Action:
        """Propose an action for execution."""
        if not self.session:
            raise ValueError("No active session")
        resp = requests.post(
            f"{self.base_url}/v1/actions",
            headers=self._headers(),
            json={
                "session_id": self.session.id,
                "tool": tool,
                "params": params,
            },
        )
        resp.raise_for_status()
        data = resp.json()
        return Action(
            id=data["id"],
            state=data["state"],
            tool=data["tool"],
            params=data["params"],
            result=data.get("result"),
            error=data.get("error"),
        )

    def get_action(self, action_id: str) -> Action:
        """Get the current state of an action."""
        resp = requests.get(
            f"{self.base_url}/v1/actions/{action_id}",
            headers=self._headers(),
        )
        resp.raise_for_status()
        data = resp.json()
        return Action(
            id=data["id"],
            state=data["state"],
            tool=data["tool"],
            params=data["params"],
            result=data.get("result"),
            error=data.get("error"),
        )

    def wait_for_action(
        self,
        action_id: str,
        timeout: float = 300,
        poll_interval: float = 1.0,
    ) -> Action:
        """Wait for an action to reach a terminal state."""
        terminal_states = {"executed", "failed", "denied"}
        start = time.time()

        while time.time() - start < timeout:
            action = self.get_action(action_id)
            if action.state in terminal_states:
                return action
            time.sleep(poll_interval)

        raise TimeoutError(f"Action {action_id} did not complete within {timeout}s")

    def execute_tool(
        self,
        tool: str,
        params: dict,
        wait: bool = True,
        timeout: float = 300,
    ) -> Action:
        """Propose an action and optionally wait for completion."""
        action = self.propose_action(tool, params)
        if wait:
            action = self.wait_for_action(action.id, timeout=timeout)
        return action
