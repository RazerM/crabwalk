import sys
from pathlib import Path
from typing import Callable, Iterator, List

import pytest

from crabwalk import Overrides, Types, Walk

from .tree import Directory, File, Symlink

if sys.version_info >= (3, 10):
    from typing import TypeAlias
else:
    from typing_extensions import TypeAlias


WalkPaths: TypeAlias = Callable[[Walk], Iterator[str]]


@pytest.fixture(name="walk_paths")
def walk_paths_fixture() -> WalkPaths:
    def walk_paths(walk: Walk) -> Iterator[str]:
        with walk:
            for entry in walk:
                yield entry.path

    return walk_paths


@pytest.mark.tree(
    Directory(
        "a",
        File("foo"),
        File("bar"),
    ),
    Directory(
        "b",
        File("spam"),
        File("egg"),
    ),
    chdir=True,
)
def test_multiple_paths(tree_path: Path, walk_paths: WalkPaths) -> None:
    walk = Walk("a", "b", sort=True)
    assert list(walk_paths(walk)) == ["a", "a/bar", "a/foo", "b", "b/egg", "b/spam"]


@pytest.mark.tree(
    Directory(
        "root",
        Directory(
            "1",
            Directory(
                "2",
                Directory("3"),
            ),
        ),
    ),
    chdir=True,
)
def test_max_depth(tree_path: Path, walk_paths: WalkPaths) -> None:
    walk = Walk("root", max_depth=2, sort=True)
    assert list(walk_paths(walk)) == ["root", "root/1", "root/1/2"]


@pytest.mark.tree(
    Directory(
        "root",
        Symlink("linked", "../sibling"),
    ),
    Directory(
        "sibling",
        File("foo"),
    ),
    chdir=True,
)
def test_follow_links(tree_path: Path, walk_paths: WalkPaths) -> None:
    walk = Walk("root", follow_links=True, sort=True)
    assert list(walk_paths(walk)) == ["root", "root/linked", "root/linked/foo"]


@pytest.mark.tree(
    Directory(
        "root",
        File("one", "x"),
        File("two", "xx"),
        File("three", "xxx"),
    ),
    chdir=True,
)
def test_max_filesize(tree_path: Path, walk_paths: WalkPaths) -> None:
    walk = Walk("root", max_filesize=2, sort=True)
    assert list(walk_paths(walk)) == ["root", "root/one", "root/two"]


@pytest.mark.tree(
    Directory(
        "root",
        File("bar1"),
        Directory(
            "bar2",
            File("foo"),
        ),
        File("baz"),
    ),
    chdir=True,
)
def test_global_ignore_files(
    tree_path: Path, walk_paths: WalkPaths, tmp_path: Path
) -> None:
    globalignore = tmp_path / "globalignore"
    globalignore.write_text("bar*\n")
    walk = Walk("root", global_ignore_files=[globalignore], sort=True)
    assert list(walk_paths(walk)) == ["root", "root/baz"]


@pytest.mark.tree(
    Directory(
        "root",
        File(".customignore", "foo*\n"),
        File("foo"),
        File("bar"),
    ),
    chdir=True,
)
def test_custom_ignore_filenames(tree_path: Path, walk_paths: WalkPaths) -> None:
    walk = Walk("root", custom_ignore_filenames=[".customignore"], sort=True)
    assert list(walk_paths(walk)) == ["root", "root/bar"]


@pytest.mark.tree(
    Directory(
        "root",
        File("foo"),
        File("bar"),
    ),
    chdir=True,
)
def test_overrides(tree_path: Path, walk_paths: WalkPaths) -> None:
    walk = Walk("root", overrides=Overrides(["!foo"], path="root"), sort=True)
    assert list(walk_paths(walk)) == ["root", "root/bar"]


@pytest.mark.tree(
    Directory(
        "root",
        File("foo.py"),
        File("foo.rs"),
    ),
    chdir=True,
)
def test_types(tree_path: Path, walk_paths: WalkPaths) -> None:
    types = Types()
    types.add_defaults()
    types.select("rust")
    walk = Walk("root", types=types, sort=True)
    assert list(walk_paths(walk)) == ["root", "root/foo.rs"]


@pytest.mark.tree(
    Directory(
        "root",
        File(".foo"),
        File("bar"),
    ),
    chdir=True,
)
@pytest.mark.parametrize(
    ("hidden", "paths"),
    [
        (True, ["root", "root/bar"]),
        (False, ["root", "root/.foo", "root/bar"]),
    ],
)
def test_hidden(
    tree_path: Path, walk_paths: WalkPaths, hidden: bool, paths: List[str]
) -> None:
    walk = Walk("root", hidden=hidden, sort=True)
    assert list(walk_paths(walk)) == paths


@pytest.mark.tree(
    Directory(
        "root",
        File("foo"),
        File("bar"),
    ),
    File(".ignore", "foo\n"),
    chdir=True,
)
@pytest.mark.parametrize(
    ("parents", "paths"),
    [
        (True, ["root", "root/bar"]),
        (False, ["root", "root/bar", "root/foo"]),
    ],
)
def test_parents(
    tree_path: Path, walk_paths: WalkPaths, parents: bool, paths: List[str]
) -> None:
    walk = Walk("root", parents=parents, sort=True)
    assert list(walk_paths(walk)) == paths


@pytest.mark.tree(
    Directory(
        "root",
        File(".ignore", "foo\n"),
        File("foo"),
    ),
    chdir=True,
)
@pytest.mark.parametrize(
    ("ignore", "paths"),
    [
        (True, ["root"]),
        (False, ["root", "root/foo"]),
    ],
)
def test_ignore(
    tree_path: Path, walk_paths: WalkPaths, ignore: bool, paths: List[str]
) -> None:
    walk = Walk("root", ignore=ignore, sort=True)
    assert list(walk_paths(walk)) == paths


@pytest.mark.tree(
    Directory(
        "root",
        File(".gitignore", "foo\n"),
        File("foo"),
    ),
    chdir=True,
)
@pytest.mark.parametrize(
    ("git_ignore", "paths"),
    [
        (True, ["root"]),
        (False, ["root", "root/foo"]),
    ],
)
def test_git_ignore(
    tree_path: Path, walk_paths: WalkPaths, git_ignore: bool, paths: List[str]
) -> None:
    walk = Walk("root", git_ignore=git_ignore, require_git=False, sort=True)
    assert list(walk_paths(walk)) == paths


@pytest.mark.tree(
    Directory(
        "root",
        Directory(
            ".git",
            Directory(
                "info",
                File("exclude", "foo"),
            ),
        ),
        File("foo"),
    ),
    chdir=True,
)
@pytest.mark.parametrize(
    ("git_exclude", "paths"),
    [
        (True, ["root"]),
        (False, ["root", "root/foo"]),
    ],
)
def test_git_exclude(
    tree_path: Path, walk_paths: WalkPaths, git_exclude: bool, paths: List[str]
) -> None:
    walk = Walk("root", git_exclude=git_exclude, require_git=False, sort=True)
    assert list(walk_paths(walk)) == paths


@pytest.mark.tree(
    Directory(
        "root",
        File(".gitignore", "foo\n"),
        File("foo"),
    ),
    chdir=True,
)
@pytest.mark.parametrize(
    ("require_git", "paths"),
    [
        (True, ["root", "root/foo"]),
        (False, ["root"]),
    ],
)
def test_require_git(
    tree_path: Path, walk_paths: WalkPaths, require_git: bool, paths: List[str]
) -> None:
    walk = Walk("root", require_git=require_git, sort=True)
    assert list(walk_paths(walk)) == paths


@pytest.mark.tree(
    Directory(
        "root",
        File(".ignore", "foo\n"),
        File("Foo"),
    ),
    chdir=True,
)
@pytest.mark.parametrize(
    ("ignore_case_insensitive", "paths"),
    [
        (True, ["root"]),
        (False, ["root", "root/Foo"]),
    ],
)
def test_ignore_case_insensitive(
    tree_path: Path,
    walk_paths: WalkPaths,
    ignore_case_insensitive: bool,
    paths: List[str],
) -> None:
    walk = Walk("root", ignore_case_insensitive=ignore_case_insensitive, sort=True)
    assert list(walk_paths(walk)) == paths
