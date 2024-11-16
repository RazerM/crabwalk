import os
import sys
from collections.abc import Callable, Iterator
from pathlib import Path
from unittest.mock import Mock

import pytest

from crabwalk import DirEntry, Overrides, Types, Walk

from .tree import Directory, File, Symlink

if sys.version_info >= (3, 10):
    from typing import TypeAlias
else:
    from typing_extensions import TypeAlias


WalkPaths: TypeAlias = Callable[[Walk], Iterator[str]]
WalkEntries: TypeAlias = Callable[[Walk], Iterator[DirEntry]]


@pytest.fixture(name="walk_entries")
def walk_entries_fixture() -> WalkEntries:
    def walk_entries(walk: Walk) -> Iterator[DirEntry]:
        with walk:
            yield from walk

    return walk_entries


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
def test_depth(tree_path: Path, walk_entries: WalkEntries) -> None:
    walk = Walk("root", max_depth=2, sort=True)
    assert [entry.depth for entry in walk_entries(walk)] == [0, 1, 2]


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
def test_follow_symlinks(tree_path: Path, walk_paths: WalkPaths) -> None:
    walk = Walk("root", follow_symlinks=True, sort=True)
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
    tree_path: Path, walk_paths: WalkPaths, hidden: bool, paths: list[str]
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
    tree_path: Path, walk_paths: WalkPaths, parents: bool, paths: list[str]
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
    tree_path: Path, walk_paths: WalkPaths, ignore: bool, paths: list[str]
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
    tree_path: Path, walk_paths: WalkPaths, git_ignore: bool, paths: list[str]
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
    tree_path: Path, walk_paths: WalkPaths, git_exclude: bool, paths: list[str]
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
    tree_path: Path, walk_paths: WalkPaths, require_git: bool, paths: list[str]
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
    paths: list[str],
) -> None:
    walk = Walk("root", ignore_case_insensitive=ignore_case_insensitive, sort=True)
    assert list(walk_paths(walk)) == paths


@pytest.mark.tree(
    Directory(
        "root",
        File("a"),
        File("b"),
    ),
    chdir=True,
)
def test_sort_function(
    tree_path: Path,
    walk_paths: WalkPaths,
) -> None:
    def sort(path: str) -> str:
        return path

    mock_sort = Mock(wraps=sort)
    walk = Walk("root", sort=mock_sort)
    assert walk.sort == mock_sort
    assert list(walk_paths(walk)) == ["root", "root/a", "root/b"]
    assert {args[0] for _, args, _ in mock_sort.mock_calls} == {"root/a", "root/b"}


@pytest.mark.tree(
    Directory(
        "root",
        File("Ab"),
        File("Ba"),
    ),
    chdir=True,
)
def test_sort_key(tree_path: Path, walk_paths: WalkPaths) -> None:
    def key(path: str) -> str:
        return os.path.basename(path)[1:]

    walk = Walk("root", sort=key)
    assert list(walk_paths(walk)) == ["root", "root/Ba", "root/Ab"]


@pytest.mark.tree(
    Directory(
        "root",
        File("foo"),
        File("bar"),
    ),
    chdir=True,
)
def test_sort_exception(tree_path: Path) -> None:
    class MyError(Exception):
        pass

    def key(path: str) -> str:
        raise MyError

    with pytest.raises(MyError):
        with Walk("root", sort=key) as walk:
            for _ in walk:
                pass


@pytest.mark.tree(
    Directory(
        "root",
        File("foo"),
        File("bar"),
    ),
    chdir=True,
)
def test_filter_entry(tree_path: Path, walk_paths: WalkPaths) -> None:
    def filter_entry(entry: DirEntry) -> bool:
        return entry.is_dir() or entry.name == "foo"

    walk = Walk("root", filter_entry=filter_entry, sort=True)
    assert list(walk_paths(walk)) == ["root", "root/foo"]


@pytest.mark.tree(
    Directory(
        "root",
        File("foo"),
    ),
    chdir=True,
)
def test_filter_entry_exception(tree_path: Path) -> None:
    class MyError(Exception):
        pass

    def filter_entry(entry: DirEntry) -> bool:
        raise MyError

    with pytest.raises(MyError):
        with Walk("root", filter_entry=filter_entry, sort=True) as walk:
            for _ in walk:
                pass


@pytest.mark.tree(
    Directory(
        "root",
        File("foo"),
    ),
    chdir=True,
)
def test_direntry_fspath(tree_path: Path, walk_entries: WalkEntries) -> None:
    root, foo = walk_entries(Walk("root", sort=True))
    assert os.fspath(root) == root.path == "root"
    assert os.fspath(foo) == foo.path == "root/foo"


@pytest.mark.tree(
    Directory(
        "root",
        File("foo"),
    ),
)
def test_direntry_fspath_absolute(tree_path: Path, walk_entries: WalkEntries) -> None:
    root_path = tree_path / "root"
    root, foo = walk_entries(Walk(root_path, sort=True))
    assert os.fspath(root) == root.path == str(root_path)
    assert os.fspath(foo) == foo.path == str(root_path / "foo")


@pytest.mark.tree(
    Directory(
        "root",
        File("foo"),
    ),
)
@pytest.mark.parametrize("stat_before_delete", [True, False])
def test_stat_cached(
    tree_path: Path, walk_entries: WalkEntries, stat_before_delete: bool
) -> None:
    root_path = tree_path / "root"
    root, foo = walk_entries(Walk(root_path, sort=True))
    if stat_before_delete:
        foo.stat()
    Path(foo).unlink()
    if stat_before_delete:
        # not only will this not raise but the same object must be returned
        assert foo.stat() is foo.stat()
    else:
        with pytest.raises(FileNotFoundError):
            foo.stat()


@pytest.mark.tree(
    Directory(
        "root",
        File("file1"),
        Directory("dir1"),
        Symlink("link1", "file1"),
        Symlink("link2", "dir1"),
    ),
    chdir=True,
)
@pytest.mark.parametrize(
    "follow_symlinks",
    [True, False],
    ids=lambda follow_symlinks: f"follow_symlinks={follow_symlinks}",
)
def test_direntry_against_os_direntry(tree_path: Path, follow_symlinks: bool) -> None:
    with os.scandir("root") as it:
        os_entries = sorted(it, key=lambda entry: entry.path)

    assert [entry.path for entry in os_entries] == [
        "root/dir1",
        "root/file1",
        "root/link1",
        "root/link2",
    ]

    with Walk("root", follow_symlinks=follow_symlinks) as walk:
        root = next(walk)
        assert root.path == "root"
        cw_entries = sorted(walk, key=lambda entry: entry.path)

    for os_entry, cw_entry in zip(os_entries, cw_entries):
        assert os.fspath(os_entry) == os.fspath(cw_entry)
        assert os_entry.path == cw_entry.path
        assert os_entry.name == cw_entry.name

        # This difference is documented in the inode method documentation.
        if follow_symlinks:
            assert os_entry.inode() == os.stat(cw_entry, follow_symlinks=False).st_ino
        else:
            assert os_entry.inode() == cw_entry.inode()

        assert os_entry.is_dir(follow_symlinks=follow_symlinks) == cw_entry.is_dir()
        assert os_entry.is_file(follow_symlinks=follow_symlinks) == cw_entry.is_file()
        assert os_entry.is_symlink() == cw_entry.is_symlink()
        assert os_entry.stat(follow_symlinks=follow_symlinks) == cw_entry.stat()
        assert cw_entry.follow_symlinks == follow_symlinks
