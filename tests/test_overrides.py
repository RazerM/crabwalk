import pickle
from copy import copy
from pathlib import Path

import pytest

from crabwalk import Override, Overrides


def test_coercion() -> None:
    overrides = Overrides(["*.py", ("*.pyi", True), Override("!*.pyc")], path=".")
    assert list(overrides) == [
        Override("*.py", False),
        Override("*.pyi", True),
        Override("!*.pyc", False),
    ]
    overrides[0] = "*.rs"
    overrides.insert(0, ("*.toml", True))
    assert overrides[:2] == Overrides(
        [Override("*.toml", True), Override("*.rs", False)], path="."
    )


def test_invalid_item() -> None:
    with pytest.raises(TypeError, match=r"Expected a \(str, bool\) tuple, got 1"):
        Overrides([1], path=".")  # type: ignore[list-item]


def test_path() -> None:
    assert Overrides(path=Path("foo")).path == "foo"
    with pytest.raises(TypeError, match="path must be a str"):
        Overrides(path=b"foo")  # type: ignore[arg-type]


def test_equality() -> None:
    assert Overrides(["*.py"], path=".") == Overrides(["*.py"], path=".")
    assert Overrides(["*.py"], path=".") != Overrides(["*.py"], path="src")
    assert Overrides(["*.py"], path=".") != Overrides(["*.pyi"], path=".")
    assert Overrides(["*.py"], path=".") != [Override("*.py")]


def test_copy() -> None:
    overrides = Overrides(["*.py"], path=".")
    for other in [copy(overrides), overrides.copy()]:
        assert other == overrides
        other.append("*.rs")
        assert len(overrides) == 1


def test_pickle() -> None:
    overrides = Overrides(["*.py"], path=".")
    assert pickle.loads(pickle.dumps(overrides)) == overrides


def test_repr() -> None:
    assert repr(Overrides(["*.py"], path=".")) == (
        "Overrides([Override(glob='*.py', case_insensitive=False)], path='.')"
    )
