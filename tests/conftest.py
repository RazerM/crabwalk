from operator import methodcaller
from pathlib import Path

import pytest

from .tree import FsItem, StrPath


class Tree:
    children: list[FsItem]
    chdir: bool

    def __init__(self, *children: FsItem, chdir: bool = False) -> None:
        self.children = list(children)
        self.chdir = chdir

    @classmethod
    def from_marker(cls, marker: pytest.Mark) -> "Tree":
        __tracebackhide__ = methodcaller("errisinstance", TypeError)
        return cls(*marker.args, **marker.kwargs)

    def setup(self, path: StrPath, monkeypatch: pytest.MonkeyPatch) -> None:
        if self.chdir:
            monkeypatch.chdir(path)
        for child in self.children:
            child.create(path)


@pytest.fixture(name="tree_path")
def tree_path_fixture(
    tmp_path_factory: pytest.TempPathFactory,
    request: pytest.FixtureRequest,
    monkeypatch: pytest.MonkeyPatch,
) -> Path:
    tree_path = tmp_path_factory.mktemp("tree")

    marker = request.node.get_closest_marker("tree")
    tree = Tree.from_marker(marker)
    tree.setup(tree_path, monkeypatch)

    return tree_path
