import os
from collections.abc import Callable, Sequence
from types import TracebackType
from typing import final

from _typeshed import StrPath, SupportsRichComparison

from ._types import Overrides, Types

__all__ = ["DirEntry", "Walk", "default_types", "is_valid_type_name"]

@final
class DirEntry:
    name: str
    path: str
    def inode(self) -> int: ...
    def is_dir(self) -> bool: ...
    def is_file(self) -> bool: ...
    def is_symlink(self) -> bool: ...
    def stat(self) -> os.stat_result: ...
    def __fspath__(self) -> str: ...
    depth: int
    follow_symlinks: bool

def default_types() -> list[tuple[str, list[str]]]: ...
def is_valid_type_name(name: str) -> bool: ...

@final
class Walk:
    def __new__(
        cls,
        *paths: StrPath,
        max_depth: int | None = ...,
        min_depth: int | None = ...,
        follow_symlinks: bool = ...,
        max_filesize: int | None = ...,
        global_ignore_files: Sequence[StrPath] | None = ...,
        custom_ignore_filenames: Sequence[str] | None = ...,
        overrides: Overrides | None = ...,
        types: Types | None = ...,
        hidden: bool = ...,
        parents: bool = ...,
        ignore: bool = ...,
        git_global: bool = ...,
        git_ignore: bool = ...,
        git_exclude: bool = ...,
        require_git: bool = ...,
        ignore_case_insensitive: bool = ...,
        sort: Callable[[str], SupportsRichComparison] | bool | None = ...,
        same_file_system: bool = ...,
        skip_stdout: bool = ...,
        filter_entry: Callable[[DirEntry], bool] | None = ...,
        onerror: Callable[[Exception], None] | None = ...,
        threads: int = ...,
    ) -> Walk: ...
    def disable_standard_filters(self) -> None: ...
    def enable_standard_filters(self) -> None: ...
    @property
    def paths(self) -> list[StrPath]: ...
    max_depth: int | None
    min_depth: int | None
    follow_symlinks: bool
    max_filesize: int | None
    @property
    def global_ignore_files(self) -> list[StrPath]: ...
    @property
    def custom_ignore_filenames(self) -> list[str]: ...
    overrides: Overrides | None
    types: Types | None
    hidden: bool
    parents: bool
    ignore: bool
    git_global: bool
    git_ignore: bool
    git_exclude: bool
    require_git: bool
    ignore_case_insensitive: bool
    sort: Callable[[str], SupportsRichComparison] | bool
    same_file_system: bool
    skip_stdout: bool
    filter_entry: Callable[[DirEntry], bool] | None
    onerror: Callable[[Exception], None] | None
    threads: int
    def __enter__(self) -> Walk: ...
    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc_val: BaseException | None,
        exc_tb: TracebackType | None,
    ) -> None: ...
    def close(self) -> None: ...
    def __iter__(self) -> Walk: ...
    def __next__(self) -> DirEntry: ...
