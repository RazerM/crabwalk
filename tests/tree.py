import os
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import List, Union

if sys.version_info >= (3, 10):
    from typing import TypeAlias
else:
    from typing_extensions import TypeAlias

if sys.version_info >= (3, 8):
    from typing import Protocol
else:
    from typing_extensions import Protocol

StrPath: TypeAlias = "Union[str, os.PathLike[str]]"


class FsItem(Protocol):
    def create(self, path: StrPath) -> None: ...


class Directory:
    name: str
    children: "List[FsItem]"

    def __init__(self, name: str, *children: "FsItem") -> None:
        self.name = name
        self.children = list(children)

    def create(self, path: StrPath) -> None:
        path = Path(path, self.name)
        path.mkdir()
        for child in self.children:
            child.create(path)


@dataclass
class Symlink:
    name: str
    target: str

    def create(self, path: StrPath) -> None:
        source = Path(path, self.name)
        os.symlink(Path(path, self.target), source)


@dataclass
class File:
    name: str
    contents: str = ""

    def create(self, path: StrPath) -> None:
        path = Path(path, self.name)
        path.write_text(self.contents)
