import os
from dataclasses import dataclass
from pathlib import Path
from typing import Protocol, TypeAlias

StrPath: TypeAlias = "str | os.PathLike[str]"


class FsItem(Protocol):
    def create(self, path: StrPath) -> None: ...


class Directory:
    name: str
    children: list["FsItem"]

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
    # Windows needs to know whether the target is a directory.
    directory: bool = False

    def create(self, path: StrPath) -> None:
        source = Path(path, self.name)
        os.symlink(Path(path, self.target), source, target_is_directory=self.directory)


@dataclass
class File:
    name: str
    contents: str = ""

    def create(self, path: StrPath) -> None:
        path = Path(path, self.name)
        path.write_text(self.contents)
