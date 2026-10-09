from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[1]  # tools/research


@pytest.fixture
def repo() -> Path:
    return REPO
