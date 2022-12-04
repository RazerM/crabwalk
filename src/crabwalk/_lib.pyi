import os
from collections.abc import (
    Callable,
    Iterable,
    Iterator,
    MutableMapping,
    Sequence,
)
from types import TracebackType
from typing import Any, Protocol, TypeVar, overload

from typing_extensions import TypeAlias, final

from ._types import Overrides

StrPath: TypeAlias = str | os.PathLike[str]

KT = TypeVar("KT")
VT_co = TypeVar("VT_co", covariant=True)
T = TypeVar("T")
T_contra = TypeVar("T_contra", contravariant=True)

class SupportsDunderLT(Protocol[T_contra]):
    def __lt__(self, __other: T_contra) -> bool: ...

class SupportsDunderGT(Protocol[T_contra]):
    def __gt__(self, __other: T_contra) -> bool: ...

SupportsRichComparison: TypeAlias = SupportsDunderLT[Any] | SupportsDunderGT[Any]

class SupportsKeysAndGetItem(Protocol[KT, VT_co]):
    def keys(self) -> Iterable[KT]: ...
    def __getitem__(self, __key: KT) -> VT_co: ...

@final
class DirEntry:
    path: str
    def inode(self) -> int: ...
    def is_dir(self) -> bool: ...
    def is_file(self) -> bool: ...
    def is_symlink(self) -> bool: ...
    file_name: str
    depth: int

VIn: TypeAlias = Sequence[str]
VOut: TypeAlias = tuple[str, ...]

@final
class Types(MutableMapping[str, VOut]):
    def __getitem__(self, __k: str) -> VOut: ...
    def __setitem__(self, __k: str, __v: VIn) -> None: ...
    def __delitem__(self, __k: str) -> None: ...
    def __iter__(self) -> Iterator[str]: ...
    def __len__(self) -> int: ...
    def setdefault(self, __key: str, __default: VIn) -> VOut: ...
    @overload
    def update(self, __m: SupportsKeysAndGetItem[str, VIn], **kwargs: VIn) -> None: ...
    @overload
    def update(self, __m: Iterable[tuple[str, VIn]], **kwargs: VIn) -> None: ...
    @overload
    def update(self, **kwargs: VIn) -> None: ...
    def add(self, name: str, glob: str) -> None: ...
    def add_defaults(self) -> None: ...
    def select(self, name: str) -> None: ...
    def negate(self, name: str) -> None: ...

@final
class Walk:
    def __new__(
        cls,
        *paths: StrPath,
        max_depth: int | None = ...,
        follow_links: bool = ...,
        max_filesize: int | None = ...,
        global_ignore_files: Sequence[StrPath] | None = ...,
        custom_ignore_filenames: Sequence[StrPath] | None = ...,
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
        sort_key: Callable[[str], SupportsRichComparison] | None = ...,
        same_file_system: bool = ...,
        skip_stdout: bool = ...,
        filter_entry: Callable[[DirEntry], bool] | None = ...,
        onerror: Callable[[Exception], None] | None = ...,
    ) -> Walk: ...

    standard_filters: bool
    @property
    def paths(self) -> list[StrPath]: ...
    max_depth: int | None
    follow_links: bool
    max_filesize: int | None
    @property
    def global_ignore_files(self) -> list[StrPath]: ...
    @property
    def custom_ignore_filenames(self) -> list[StrPath]: ...
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
    sort_key: Callable[[DirEntry], bool] | None
    same_file_system: bool
    skip_stdout: bool
    filter_entry: Callable[[DirEntry], bool] | None
    onerror: Callable[[Exception], None] | None
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
