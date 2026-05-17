"""Shared test fixtures -- loads the contract fixture and sets up respx mock."""

from __future__ import annotations

import json
from pathlib import Path

import pytest
import respx

# The contract fixture lives two levels up, next to the SDK directory.
_FIXTURE_PATH = Path(__file__).resolve().parent.parent.parent / "contract-fixture.json"


@pytest.fixture(scope="session")
def fixture_data() -> dict:
    """Load the shared contract-fixture.json once per test session."""
    return json.loads(_FIXTURE_PATH.read_text())


@pytest.fixture()
def respx_mock():
    """Provide a respx mock router that auto-activates for the test."""
    with respx.mock(assert_all_called=False) as router:
        yield router
