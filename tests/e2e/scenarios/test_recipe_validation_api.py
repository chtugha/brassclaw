"""E2E test: Recipe-Skill-Tool validation queue REST API surface.

Tests the REST endpoints introduced in Phase 7 for the Recipe-Skill-Tool
learning pipeline. Focuses on:

1. Auth enforcement — every endpoint requires a valid bearer token.
2. Empty-state correctness — endpoints return well-formed 200s with empty
   collections when no recipes/skills have been extracted yet.
3. Count endpoint — returns correct totals by validation status.
4. Unknown ID handling — GET/PUT for unknown IDs return 404 (not 500).
5. Status transition enforcement — invalid transitions return 422, not 500.
6. Validation queue — returns only user-actionable items (auto_passed,
   upgrade_queued, review_requested), not automated-pipeline statuses.

These tests exercise the HTTP surface without requiring a running learning
mission or LLM. They use the same gateway fixture as other Reborn e2e tests
(``reborn_gateway_server``), which starts brassclaw with a mock LLM server.

Full validation pipeline testing (extract → auto-validate → user validate →
Tier 0 hit) requires learning missions to fire, which in turn needs a mock
LLM response shaped like an extraction mission output. That flow is covered
by the Rust integration tests in
``crates/brassclaw_reborn_composition/src/recipe_store.rs``.
"""
import pytest
pytest.skip(
    "Disabled during legacy CI cleanup: v1-era scenario not confirmed against "
    "Reborn. Remove this skip (and the import) to reactivate.",
    allow_module_level=True,
)


import asyncio
import os
import signal
import socket
import tempfile
from pathlib import Path

import httpx
import pytest

import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from helpers import api_get, api_post, AUTH_TOKEN, wait_for_ready


ROOT = Path(__file__).resolve().parent.parent.parent.parent
_RECIPE_DB_TMPDIR = tempfile.TemporaryDirectory(prefix="brassclaw-recipe-api-e2e-")
_RECIPE_HOME_TMPDIR = tempfile.TemporaryDirectory(prefix="brassclaw-recipe-api-e2e-home-")

_BAD_AUTH = "definitely-not-a-valid-token"
_DEFAULT_PROJECT = "bootstrap"


def _forward_coverage_env(env: dict):
    for key in os.environ:
        if key.startswith(("CARGO_LLVM_COV", "LLVM_", "CARGO_ENCODED_RUSTFLAGS",
                           "CARGO_INCREMENTAL")):
            env[key] = os.environ[key]


async def _stop_process(proc, sig=signal.SIGINT, timeout=5):
    try:
        proc.send_signal(sig)
    except ProcessLookupError:
        return
    try:
        await asyncio.wait_for(proc.wait(), timeout=timeout)
    except asyncio.TimeoutError:
        proc.kill()
        await proc.wait()


def _api_put(base_url: str, path: str, *, token: str = AUTH_TOKEN, **kwargs):
    return httpx.AsyncClient().put(
        f"{base_url}{path}",
        headers={"Authorization": f"Bearer {token}"},
        timeout=kwargs.pop("timeout", 10),
        **kwargs,
    )


async def api_put(base_url: str, path: str, *, token: str = AUTH_TOKEN, **kwargs) -> httpx.Response:
    async with httpx.AsyncClient() as client:
        return await client.put(
            f"{base_url}{path}",
            headers={"Authorization": f"Bearer {token}"},
            timeout=kwargs.pop("timeout", 10),
            **kwargs,
        )


@pytest.fixture(scope="module")
async def recipe_api_server(brassclaw_binary, mock_llm_server):
    """Start an isolated brassclaw gateway for recipe API tests."""
    home_dir = _RECIPE_HOME_TMPDIR.name
    os.makedirs(os.path.join(home_dir, ".brassclaw"), exist_ok=True)

    socks = []
    for _ in range(2):
        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s.bind(("127.0.0.1", 0))
        socks.append(s)
    gateway_port = socks[0].getsockname()[1]
    http_port = socks[1].getsockname()[1]
    for s in socks:
        s.close()

    env = {
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
        "HOME": home_dir,
        "BRASSCLAW_BASE_DIR": os.path.join(home_dir, ".brassclaw"),
        "RUST_LOG": "brassclaw=info",
        "RUST_BACKTRACE": "1",
        "ENGINE_V2": "true",
        "AGENT_AUTO_APPROVE_TOOLS": "true",
        "GATEWAY_ENABLED": "true",
        "GATEWAY_HOST": "127.0.0.1",
        "GATEWAY_PORT": str(gateway_port),
        "GATEWAY_AUTH_TOKEN": AUTH_TOKEN,
        "GATEWAY_USER_ID": "e2e-recipe-api-user",
        "HTTP_HOST": "127.0.0.1",
        "HTTP_PORT": str(http_port),
        "CLI_ENABLED": "false",
        "LLM_BACKEND": "openai_compatible",
        "LLM_BASE_URL": mock_llm_server,
        "LLM_API_KEY": "mock-api-key",
        "LLM_MODEL": "mock-model",
        "DATABASE_BACKEND": "libsql",
        "LIBSQL_PATH": os.path.join(_RECIPE_DB_TMPDIR.name, "recipe-api-e2e.db"),
        "SANDBOX_ENABLED": "false",
        "SKILLS_ENABLED": "false",
        "ROUTINES_ENABLED": "false",
        "HEARTBEAT_ENABLED": "false",
        "EMBEDDING_ENABLED": "false",
        "WASM_ENABLED": "false",
        "ONBOARD_COMPLETED": "true",
    }
    _forward_coverage_env(env)

    proc = await asyncio.create_subprocess_exec(
        brassclaw_binary,
        "--no-onboard",
        stdin=asyncio.subprocess.DEVNULL,
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
        env=env,
    )

    base_url = f"http://127.0.0.1:{gateway_port}"
    try:
        await wait_for_ready(f"{base_url}/api/health", timeout=60)
    except TimeoutError:
        if proc.returncode is None:
            await _stop_process(proc, timeout=2)
        pytest.fail("Recipe API e2e server failed to start within 60 seconds")

    try:
        yield base_url
    finally:
        if proc is not None and proc.returncode is None:
            await _stop_process(proc, sig=signal.SIGINT, timeout=10)
            if proc.returncode is None:
                await _stop_process(proc, sig=signal.SIGTERM, timeout=5)


class TestRecipeListEndpoints:
    """GET /api/webchat/v2/recipes and GET /api/webchat/v2/tool-skills."""

    async def test_list_recipes_returns_empty_on_fresh_instance(self, recipe_api_server):
        resp = await api_get(recipe_api_server, "/api/webchat/v2/recipes")
        assert resp.status_code == 200
        body = resp.json()
        assert isinstance(body, list), "list_recipes must return a JSON array"
        assert len(body) == 0, "fresh instance has no extracted recipes"

    async def test_list_tool_skills_returns_empty_on_fresh_instance(self, recipe_api_server):
        resp = await api_get(recipe_api_server, "/api/webchat/v2/tool-skills")
        assert resp.status_code == 200
        body = resp.json()
        assert isinstance(body, list), "list_tool_skills must return a JSON array"
        assert len(body) == 0, "fresh instance has no extracted tool-skills"

    async def test_list_recipes_requires_auth(self, recipe_api_server):
        resp = await api_get(recipe_api_server, "/api/webchat/v2/recipes", token=_BAD_AUTH)
        assert resp.status_code in (401, 403), (
            f"list_recipes must reject unauthenticated requests (got {resp.status_code})"
        )

    async def test_list_tool_skills_requires_auth(self, recipe_api_server):
        resp = await api_get(recipe_api_server, "/api/webchat/v2/tool-skills", token=_BAD_AUTH)
        assert resp.status_code in (401, 403), (
            f"list_tool_skills must reject unauthenticated requests (got {resp.status_code})"
        )


class TestValidationQueueEndpoints:
    """GET /api/webchat/v2/validation-queue and /count."""

    async def test_validation_queue_returns_empty_on_fresh_instance(self, recipe_api_server):
        resp = await api_get(
            recipe_api_server,
            f"/api/webchat/v2/validation-queue?project_id={_DEFAULT_PROJECT}",
        )
        assert resp.status_code == 200
        body = resp.json()
        assert isinstance(body, list), "validation-queue must return a JSON array"
        assert len(body) == 0, "fresh instance has no validation queue items"

    async def test_validation_queue_count_auto_passed_returns_zero(self, recipe_api_server):
        resp = await api_get(
            recipe_api_server,
            f"/api/webchat/v2/validation-queue/count"
            f"?project_id={_DEFAULT_PROJECT}&status=auto_passed",
        )
        assert resp.status_code == 200
        body = resp.json()
        assert "count" in body, "count response must have a 'count' field"
        assert body["count"] == 0, "fresh instance has zero auto_passed items"

    async def test_validation_queue_count_validated_returns_zero(self, recipe_api_server):
        resp = await api_get(
            recipe_api_server,
            f"/api/webchat/v2/validation-queue/count"
            f"?project_id={_DEFAULT_PROJECT}&status=validated",
        )
        assert resp.status_code == 200
        body = resp.json()
        assert body["count"] == 0

    async def test_validation_queue_requires_auth(self, recipe_api_server):
        resp = await api_get(
            recipe_api_server,
            "/api/webchat/v2/validation-queue",
            token=_BAD_AUTH,
        )
        assert resp.status_code in (401, 403)

    async def test_validation_queue_count_requires_auth(self, recipe_api_server):
        resp = await api_get(
            recipe_api_server,
            "/api/webchat/v2/validation-queue/count?project_id=bootstrap&status=auto_passed",
            token=_BAD_AUTH,
        )
        assert resp.status_code in (401, 403)


class TestRecipeDetailEndpoints:
    """GET /api/webchat/v2/recipes/{project_id}/{recipe_id}."""

    async def test_get_recipe_returns_404_for_unknown_id(self, recipe_api_server):
        resp = await api_get(
            recipe_api_server,
            f"/api/webchat/v2/recipes/{_DEFAULT_PROJECT}/does-not-exist",
        )
        assert resp.status_code == 404, (
            f"unknown recipe ID must return 404, got {resp.status_code}"
        )

    async def test_get_tool_skill_returns_404_for_unknown_id(self, recipe_api_server):
        resp = await api_get(
            recipe_api_server,
            f"/api/webchat/v2/tool-skills/{_DEFAULT_PROJECT}/does-not-exist",
        )
        assert resp.status_code == 404, (
            f"unknown skill ID must return 404, got {resp.status_code}"
        )

    async def test_get_recipe_requires_auth(self, recipe_api_server):
        resp = await api_get(
            recipe_api_server,
            f"/api/webchat/v2/recipes/{_DEFAULT_PROJECT}/some-id",
            token=_BAD_AUTH,
        )
        assert resp.status_code in (401, 403)


class TestValidationMutationEndpoints:
    """PUT /api/webchat/v2/recipes/{project_id}/{recipe_id}/validate etc."""

    async def test_validate_recipe_returns_404_for_unknown_id(self, recipe_api_server):
        resp = await api_put(
            recipe_api_server,
            f"/api/webchat/v2/recipes/{_DEFAULT_PROJECT}/no-such-recipe/validate",
            json={},
        )
        assert resp.status_code == 404, (
            f"validate unknown recipe must return 404, got {resp.status_code}"
        )

    async def test_reject_recipe_returns_404_for_unknown_id(self, recipe_api_server):
        resp = await api_put(
            recipe_api_server,
            f"/api/webchat/v2/recipes/{_DEFAULT_PROJECT}/no-such-recipe/reject",
            json={},
        )
        assert resp.status_code == 404

    async def test_review_request_returns_404_for_unknown_id(self, recipe_api_server):
        resp = await api_put(
            recipe_api_server,
            f"/api/webchat/v2/recipes/{_DEFAULT_PROJECT}/no-such-recipe/review-request",
            json={"feedback": "please improve this"},
        )
        assert resp.status_code == 404

    async def test_validate_tool_skill_returns_404_for_unknown_id(self, recipe_api_server):
        resp = await api_put(
            recipe_api_server,
            f"/api/webchat/v2/tool-skills/{_DEFAULT_PROJECT}/no-such-skill/validate",
            json={},
        )
        assert resp.status_code == 404

    async def test_reject_tool_skill_returns_404_for_unknown_id(self, recipe_api_server):
        resp = await api_put(
            recipe_api_server,
            f"/api/webchat/v2/tool-skills/{_DEFAULT_PROJECT}/no-such-skill/reject",
            json={},
        )
        assert resp.status_code == 404

    async def test_validate_recipe_requires_auth(self, recipe_api_server):
        resp = await api_put(
            recipe_api_server,
            f"/api/webchat/v2/recipes/{_DEFAULT_PROJECT}/any-id/validate",
            json={},
            token=_BAD_AUTH,
        )
        assert resp.status_code in (401, 403)


class TestRecipeOutcomeEndpoint:
    """POST /api/webchat/v2/recipes/{project_id}/{recipe_id}/outcomes."""

    async def test_record_outcome_returns_404_for_unknown_recipe(self, recipe_api_server):
        resp = await api_post(
            recipe_api_server,
            f"/api/webchat/v2/recipes/{_DEFAULT_PROJECT}/no-such-recipe/outcomes",
            json={"success": True},
        )
        assert resp.status_code in (404, 503), (
            f"recording outcome for unknown recipe must return 404 or 503 (service unavailable), "
            f"got {resp.status_code}"
        )

    async def test_record_outcome_requires_auth(self, recipe_api_server):
        resp = await api_post(
            recipe_api_server,
            f"/api/webchat/v2/recipes/{_DEFAULT_PROJECT}/any-id/outcomes",
            json={"success": True},
            token=_BAD_AUTH,
        )
        assert resp.status_code in (401, 403)
