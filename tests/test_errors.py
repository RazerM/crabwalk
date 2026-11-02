import errno
import os
import pickle
import sys
from pathlib import Path
from typing import NoReturn

import pytest

from crabwalk import (
    GlobError,
    InvalidDefinitionError,
    LoopError,
    Overrides,
    PartialError,
    Types,
    UnrecognizedFileTypeError,
    Walk,
)


def raise_error(exc: Exception) -> NoReturn:
    raise exc


def test_loop(tmp_path: Path) -> None:
    ancestor = tmp_path / "root"
    ancestor.mkdir()
    child = ancestor / "loop"
    os.symlink(ancestor, child, target_is_directory=True)

    with pytest.raises(LoopError) as exc_info:
        with Walk(tmp_path, follow_symlinks=True, onerror=raise_error) as walk:
            for entry in walk:
                print(entry)

    assert exc_info.value.depth == 2
    assert exc_info.value.path is None
    assert exc_info.value.line is None
    assert exc_info.value.ancestor == str(ancestor)
    assert exc_info.value.child == str(child)
    assert str(exc_info.value) == (
        f"File system loop found: {child} points to an ancestor {ancestor}"
    )


@pytest.mark.parametrize("filename", [".ignore", ".gitignore", ".testignore"])
def test_ignore_glob_error(tmp_path: Path, filename: str) -> None:
    ignore = tmp_path / filename
    glob = "{"
    ignore.write_text(glob)

    with pytest.raises(GlobError) as exc_info:
        with Walk(tmp_path, onerror=raise_error) as walk:
            walk.custom_ignore_filenames.append(".testignore")
            for entry in walk:
                print(entry)

    assert exc_info.value.glob == glob
    assert exc_info.value.depth is None
    assert exc_info.value.line == 1
    assert exc_info.value.path == str(ignore)
    assert str(exc_info.value) == (
        f"{ignore}: line 1: error parsing glob '{{': unclosed alternate group; "
        "missing '}' (maybe escape '{' with '[{]'?)"
    )


def test_override_glob_error(tmp_path: Path) -> None:
    overrides = Overrides(path=tmp_path)
    glob = "{"
    overrides.append(glob)

    with pytest.raises(GlobError) as exc_info:
        with Walk(tmp_path, overrides=overrides, onerror=raise_error) as walk:
            for entry in walk:
                print(entry)

    assert exc_info.value.glob == glob
    assert exc_info.value.depth is None
    assert exc_info.value.line is None
    assert exc_info.value.path is None
    assert str(exc_info.value) == (
        "error parsing glob '{': unclosed alternate group; missing '}' (maybe "
        "escape '{' with '[{]'?)"
    )


def test_uncrecognized_file_type_error(tmp_path: Path) -> None:
    types = Types()
    file_type = "foo"
    types.select(file_type)

    with pytest.raises(UnrecognizedFileTypeError) as exc_info:
        with Walk(tmp_path, types=types) as walk:
            for entry in walk:
                print(entry)

    assert exc_info.value.depth is None
    assert exc_info.value.line is None
    assert exc_info.value.path is None
    assert exc_info.value.name == file_type
    assert str(exc_info.value) == "unrecognized file type: foo"


def test_invalid_definition_error(tmp_path: Path) -> None:
    types = Types()

    with pytest.raises(InvalidDefinitionError) as exc_info:
        types.add("*bar", "*.foo")

    assert exc_info.value.depth is None
    assert exc_info.value.line is None
    assert exc_info.value.path is None
    assert isinstance(exc_info.value, ValueError)
    assert exc_info.value.name == "*bar"
    assert str(exc_info.value) == (
        "invalid file type name '*bar': names must only contain letters and "
        "numbers, and can't be 'all'"
    )
    copy = pickle.loads(pickle.dumps(exc_info.value))
    assert copy.name == "*bar"


def test_partial_error(tmp_path: Path) -> None:
    ignore = tmp_path / "myignore"
    ignore.write_text("{\na{")

    with pytest.raises(PartialError) as exc_info:
        with Walk(tmp_path, onerror=raise_error) as walk:
            walk.global_ignore_files.append(ignore)
            for entry in walk:
                print(entry)

    first, second = exc_info.value.errors
    assert isinstance(first, GlobError)
    assert isinstance(second, GlobError)
    for exc in [first, second]:
        assert exc.path == str(ignore)
        assert exc.depth is None

    assert str(first) == (
        f"{ignore}: line 1: error parsing glob '{{': unclosed alternate group; "
        "missing '}' (maybe escape '{' with '[{]'?)"
    )
    assert first.line == 1
    assert str(second) == (
        f"{ignore}: line 2: error parsing glob 'a{{': unclosed alternate group; "
        "missing '}' (maybe escape '{' with '[{]'?)"
    )
    assert second.line == 2


def test_missing_path(tmp_path: Path) -> None:
    missing = tmp_path / "missing"
    errors: list[Exception] = []

    with Walk(missing, onerror=errors.append) as walk:
        assert list(walk) == []

    [error] = errors
    assert isinstance(error, FileNotFoundError)
    assert error.errno == errno.ENOENT
    assert error.filename == str(missing)
    assert error.depth == 0  # type: ignore[attr-defined]

    # The same message as Python's, which differs on Windows.
    with pytest.raises(FileNotFoundError) as expected:
        os.stat(str(missing))
    assert error.strerror == expected.value.strerror
    assert str(error) == str(expected.value)

    copy = pickle.loads(pickle.dumps(error))
    assert type(copy) is FileNotFoundError
    assert copy.filename == str(missing)


@pytest.mark.skipif(
    sys.platform == "win32" or os.geteuid() == 0,
    reason="needs POSIX permissions and a non-root user",
)
def test_permission_error(tmp_path: Path) -> None:
    locked = tmp_path / "locked"
    locked.mkdir()
    locked.chmod(0)
    errors: list[Exception] = []

    try:
        with Walk(tmp_path, onerror=errors.append) as walk:
            assert [entry.path for entry in walk] == [str(tmp_path), str(locked)]
    finally:
        locked.chmod(0o700)

    [error] = errors
    assert isinstance(error, PermissionError)
    assert error.errno == errno.EACCES
    assert error.filename == str(locked)
    assert error.depth == 1  # type: ignore[attr-defined]


def test_partial_error_attributes() -> None:
    error = PartialError([GlobError("bad", glob="{")])
    assert error.line is None
    assert error.path is None
    assert error.depth is None

    copy = pickle.loads(pickle.dumps(error))
    assert str(copy) == str(error)
    [glob_error] = copy.errors
    assert isinstance(glob_error, GlobError)
    assert glob_error.glob == "{"
