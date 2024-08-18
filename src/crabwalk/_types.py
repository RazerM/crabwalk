import os
import sys
from typing import (
    Iterable,
    Iterator,
    List,
    MutableSequence,
    NamedTuple,
    Optional,
    Sequence,
    Tuple,
    Union,
    overload,
)


def _display(s: str) -> str:
    return os.fsencode(s).decode(sys.getfilesystemencoding(), "replace")


class WalkError(Exception):
    def __init__(
        self,
        *args: object,
        line: Optional[int] = None,
        path: Optional[str] = None,
        depth: Optional[int] = None,
    ) -> None:
        super().__init__(*args)
        self.line = line
        self.path = path
        self.depth = depth

    def __reduce__(self) -> Tuple[object, ...]:
        cls = type(self)
        return cls.__new__, (cls, *self.args), self.__dict__

    @property
    def _prefix(self) -> str:
        parts = []
        if self.path is not None:
            parts.append(f"{_display(self.path)}: ")
        if self.line is not None:
            parts.append(f"line {self.line}: ")
        return "".join(parts)


class LoopError(WalkError):
    def __init__(
        self,
        *,
        ancestor: str,
        child: str,
        line: Optional[int] = None,
        path: Optional[str] = None,
        depth: Optional[int] = None,
    ) -> None:
        super().__init__(line=line, path=path, depth=depth)
        self.ancestor = ancestor
        self.child = child

    def __str__(self) -> str:
        return (
            f"{self._prefix}File system loop found: {_display(self.child)} "
            f"points to an ancestor {_display(self.ancestor)}"
        )


class GlobError(WalkError):
    def __init__(
        self,
        message: str,
        *,
        glob: Optional[str] = None,
        line: Optional[int] = None,
        path: Optional[str] = None,
        depth: Optional[int] = None,
    ) -> None:
        super().__init__(message, line=line, path=path, depth=depth)
        self.glob = glob

    def __str__(self) -> str:
        if self.glob is None:
            msg = self.args[0]
        else:
            msg = f"error parsing glob {self.glob!r}: {self.args[0]}"
        return f"{self._prefix}{msg}"


class PartialError(WalkError):
    errors: Sequence[WalkError]

    def __init__(self, errors: Sequence[WalkError]) -> None:
        self.errors = errors

    def __str__(self) -> str:
        return "\n".join(str(error) for error in self.errors)


class InvalidDefinitionError(WalkError):
    def __init__(self) -> None:
        super().__init__()

    def __str__(self) -> str:
        return "invalid definition (format is type:glob, e.g. html:*.html)"


class UnrecognizedFileTypeError(WalkError):
    def __init__(self, *, name: str) -> None:
        super().__init__()
        self.name = name

    def __str__(self) -> str:
        return f"unrecognized file type: {self.name}"


class Override(NamedTuple):
    """A :class:`~collections.namedtuple` used by :class:`Overrides`.

    :param glob: A glob with the same semantics as a single line in a
        ``.gitignore`` file, where the meaning of ``!`` is inverted: namely,
        ``!`` at the beginning of a glob will ignore a file. Without ``!``, all
        matches of the glob provided are treated as whitelist matches.
    :type glob: str
    :param case_insensitive: Whether this glob should be matched case
        insensitively or not.
    :type case_insensitive: bool
    """

    glob: str
    case_insensitive: bool = False


def coerce_override(v: object) -> Override:
    if isinstance(v, str):
        v = Override(v)

    if not isinstance(v, tuple):
        raise TypeError(f"Expected a (str, bool) tuple, got {v!r}")

    return Override._make(v)


OverrideT = Union[str, Tuple[str, bool]]


class Overrides(MutableSequence[OverrideT]):
    """A :class:`~collections.abc.MutableSequence` of :class:`Override` tuples.

    Strings and tuples will be coerced to :class:`Override` instances.

    :param overrides: An iterable of globs, ``(glob, case_insensitive)``
        tuples, or :class:`Override` namedtuples.

        .. doctest::

            >>> o = Overrides(["*.py", ("*.pyi", True), Override("!*.pyc")], path=".")
            >>> assert o[0] == Override("*.py", False)
            >>> assert o[1] == Override("*.pyi", True)
            >>> assert o[2] == Override("!*.pyc", False)
    :param path: Globs are matched relative to this path.

    """

    _path: str
    _overrides: List[Override]

    def __init__(
        self,
        overrides: Iterable[OverrideT] = (),
        *,
        path: "Union[str, os.PathLike[str]]",
    ) -> None:
        path = os.fspath(path)
        if not isinstance(path, str):
            raise TypeError(
                "path must be a str object or an os.PathLike object returning "
                f"str, not {type(path)}"
            )
        self._path = path
        self._overrides = []
        self.extend(overrides)

    @property
    def path(self) -> str:
        """Read-only attribute of specified path."""
        return self._path

    @overload
    def __getitem__(self, index: int) -> Override: ...

    @overload
    def __getitem__(self, index: slice) -> "Overrides": ...

    def __getitem__(self, index: Union[int, slice]) -> "Union[Override, Overrides]":
        if isinstance(index, slice):
            return Overrides(self._overrides[index], path=self._path)
        else:
            return self._overrides[index]

    @overload
    def __setitem__(self, index: int, value: OverrideT) -> None: ...

    @overload
    def __setitem__(self, index: slice, value: Iterable[OverrideT]) -> None: ...

    def __setitem__(
        self, index: Union[int, slice], value: Union[OverrideT, Iterable[OverrideT]]
    ) -> None:
        if isinstance(index, slice):
            self._overrides[index] = (coerce_override(o) for o in value)
        else:
            self._overrides[index] = coerce_override(value)

    @overload
    def __delitem__(self, index: int) -> None: ...

    @overload
    def __delitem__(self, index: slice) -> None: ...

    def __delitem__(self, index: Union[int, slice]) -> None:
        del self._overrides[index]

    def __len__(self) -> int:
        return len(self._overrides)

    def insert(self, index: int, value: OverrideT) -> None:
        self._overrides.insert(index, coerce_override(value))

    def __iter__(self) -> Iterator[Override]:
        return iter(self._overrides)

    def __reversed__(self) -> Iterator[Override]:
        yield from reversed(self._overrides)

    def pop(self, index: int = -1) -> Override:
        return self._overrides.pop(index)

    def __repr__(self) -> str:
        cls = type(self)
        return f"{cls.__name__}({self._overrides!r}, path={self._path!r})"
