import os
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
        with Walk(tmp_path, follow_links=True, onerror=raise_error) as walk:
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
    assert str(exc_info.value) == (
        "invalid definition (format is type:glob, e.g. html:*.html)"
    )


def test_partial_error(tmp_path: Path) -> None:
    ignore = tmp_path / "myignore"
    ignore.write_text("{\na{")

    with pytest.raises(PartialError) as exc_info:
        with Walk(tmp_path, onerror=raise_error) as walk:
            walk.global_ignore_files.append(ignore)
            for entry in walk:
                print(entry)

    for exc in exc_info.value.errors:
        assert isinstance(exc, GlobError)
        assert exc.path == str(ignore)
        assert exc.depth is None

    assert str(exc_info.value.errors[0]) == (
        f"{ignore}: line 1: error parsing glob '{{': unclosed alternate group; "
        "missing '}' (maybe escape '{' with '[{]'?)"
    )
    assert exc_info.value.errors[0].line == 1
    assert str(exc_info.value.errors[1]) == (
        f"{ignore}: line 2: error parsing glob 'a{{': unclosed alternate group; "
        "missing '}' (maybe escape '{' with '[{]'?)"
    )
    assert exc_info.value.errors[1].line == 2
