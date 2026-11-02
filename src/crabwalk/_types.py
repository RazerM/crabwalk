from __future__ import annotations

import os
import sys
from collections.abc import (
    Iterable,
    Iterator,
    MutableMapping,
    MutableSequence,
    Sequence,
)
from dataclasses import dataclass
from typing import TYPE_CHECKING, NamedTuple, Self, TypeAlias, overload

from . import _lib

if TYPE_CHECKING:
    from _typeshed import SupportsKeysAndGetItem


def _display(s: str) -> str:
    return os.fsencode(s).decode(sys.getfilesystemencoding(), "replace")


class WalkError(Exception):
    def __init__(
        self,
        *args: object,
        line: int | None = None,
        path: str | None = None,
        depth: int | None = None,
    ) -> None:
        super().__init__(*args)
        self.line = line
        self.path = path
        self.depth = depth

    def __reduce__(self) -> tuple[object, ...]:
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
        line: int | None = None,
        path: str | None = None,
        depth: int | None = None,
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
        glob: str | None = None,
        line: int | None = None,
        path: str | None = None,
        depth: int | None = None,
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
    errors: Sequence[Exception]

    def __init__(self, errors: Sequence[Exception]) -> None:
        super().__init__(errors)
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
        msg = f"Expected a (str, bool) tuple, got {v!r}"
        raise TypeError(msg)

    return Override._make(v)


OverrideT: TypeAlias = str | tuple[str, bool]


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
    _overrides: list[Override]

    def __init__(
        self,
        overrides: Iterable[OverrideT] = (),
        *,
        path: str | os.PathLike[str],
    ) -> None:
        path = os.fspath(path)
        if not isinstance(path, str):
            # Only reachable for untyped callers passing bytes paths.
            msg = (  # type: ignore[unreachable]
                "path must be a str object or an os.PathLike object returning "
                f"str, not {type(path)}"
            )
            raise TypeError(msg)
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
    def __getitem__(self, index: slice) -> Overrides: ...

    def __getitem__(self, index: int | slice) -> Override | Overrides:
        if isinstance(index, slice):
            return Overrides(self._overrides[index], path=self._path)
        else:
            return self._overrides[index]

    @overload
    def __setitem__(self, index: int, value: OverrideT) -> None: ...

    @overload
    def __setitem__(self, index: slice, value: Iterable[OverrideT]) -> None: ...

    def __setitem__(
        self, index: int | slice, value: OverrideT | Iterable[OverrideT]
    ) -> None:
        if isinstance(index, slice):
            self._overrides[index] = (coerce_override(o) for o in value)
        else:
            self._overrides[index] = coerce_override(value)

    @overload
    def __delitem__(self, index: int) -> None: ...

    @overload
    def __delitem__(self, index: slice) -> None: ...

    def __delitem__(self, index: int | slice) -> None:
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

    def __eq__(self, other: object) -> bool:
        if isinstance(other, Overrides):
            return (self._path, self._overrides) == (other._path, other._overrides)
        return NotImplemented

    def __copy__(self) -> Self:
        return type(self)(self._overrides, path=self._path)

    copy = __copy__


@dataclass(frozen=True, slots=True)
class Select:
    """A selection of a file type by :meth:`Types.select`."""

    name: str


@dataclass(frozen=True, slots=True)
class Negate:
    """A negation of a file type by :meth:`Types.negate`."""

    name: str


Selection: TypeAlias = Select | Negate
_TypesInit: TypeAlias = (
    "SupportsKeysAndGetItem[str, Iterable[str]] | Iterable[tuple[str, Iterable[str]]]"
)


class Types(MutableMapping[str, tuple[str, ...]]):
    """A collection of file type definitions with selections and negations.

    ``Types`` is a :class:`~collections.abc.MutableMapping` of file type names
    to globs. Values can be set to any iterable of globs, but are always
    returned as a tuple.

    Typical usage:

    .. code-block:: python

        types = Types()
        types.add_defaults()
        types.select("...")
        types.negate("...")

        with Walk(..., types=types) as walk:
            ...

    .. note::

        A :class:`Types` instance without any selections or negations won't
        affect the output of :class:`Walk`.

    Like :class:`~collections.OrderedDict` and its order, selections are only
    compared with other :class:`Types` instances. Comparisons with other
    mappings only compare the definitions.

    :param other: Initial definitions, in any form accepted by
        :meth:`~collections.abc.MutableMapping.update`.
    :type other: collections.abc.Mapping[str, collections.abc.Iterable[str]] |
        collections.abc.Iterable[tuple[str, collections.abc.Iterable[str]]]
    """

    _types: dict[str, tuple[str, ...]]
    _selections: list[Selection]

    def __init__(self, other: _TypesInit = (), /, **kwargs: Iterable[str]) -> None:
        self._types = {}
        self._selections = []
        self.update(other, **kwargs)

    def __getitem__(self, name: str) -> tuple[str, ...]:
        return self._types[name]

    def __setitem__(self, name: str, globs: Iterable[str]) -> None:
        if isinstance(globs, str):
            msg = "globs must be an iterable of str, not a str"
            raise TypeError(msg)
        globs = tuple(globs)
        for glob in globs:
            if not isinstance(glob, str):
                # Only reachable for untyped callers.
                msg = f"globs must be str, not {type(glob).__name__}"  # type: ignore[unreachable]
                raise TypeError(msg)
        _lib.validate_type_name(name)
        self._types[name] = globs

    def __delitem__(self, name: str) -> None:
        del self._types[name]

    def __iter__(self) -> Iterator[str]:
        return iter(self._types)

    def __len__(self) -> int:
        return len(self._types)

    def __eq__(self, other: object) -> bool:
        # Like OrderedDict, selections are only compared with other Types.
        if isinstance(other, Types):
            return (self._types, self._selections) == (
                other._types,
                other._selections,
            )
        return super().__eq__(other)

    def __repr__(self) -> str:
        return (
            f"<{type(self).__name__} {self._types!r} selections={self._selections!r}>"
        )

    def __copy__(self) -> Self:
        new = type(self)(self._types)
        new._selections = self._selections.copy()
        return new

    copy = __copy__

    def update(self, other: _TypesInit = (), /, **kwargs: Iterable[str]) -> None:
        # Overridden to accept any iterable of globs as values.
        super().update(other, **kwargs)  # type: ignore[arg-type]

    def setdefault(self, name: str, default: Iterable[str] = (), /) -> tuple[str, ...]:
        if name not in self._types:
            self[name] = default
        return self._types[name]

    def popitem(self) -> tuple[str, tuple[str, ...]]:
        return self._types.popitem()

    def clear(self) -> None:
        self._types.clear()

    def add(self, name: str, glob: str) -> None:
        """Add ``glob`` to the file type ``name``, creating it if necessary.

        .. doctest::

            >>> types = Types()
            >>> types.add("py", "*.py")
            >>> types.add("py", "*.pyi")
            >>> types["py"]
            ('*.py', '*.pyi')
        """
        self[name] = (*self._types.get(name, ()), glob)

    def add_defaults(self) -> None:
        """Add the default file types from the ``ignore`` crate.

        These are the same as ripgrep's, see ``rg --type-list``. Globs are
        added to existing file types with the same name.

        .. doctest::

            >>> types = Types()
            >>> types.add_defaults()
            >>> types["py"]
            ('*.py', '*.pyi')
        """
        for name, globs in _lib.default_types():
            self[name] = (*self._types.get(name, ()), *globs)

    def select(self, name: str) -> None:
        """Select the file type given by ``name``.

        If ``name`` is ``all``, then all file types currently defined are
        selected.
        """
        self._selections.extend(Select(name) for name in self._expand(name))

    def negate(self, name: str) -> None:
        """Ignore the file type given by ``name``.

        If ``name`` is ``all``, then all file types currently defined are
        negated.
        """
        self._selections.extend(Negate(name) for name in self._expand(name))

    def selections(self) -> Iterator[Selection]:
        """Return an iterator over the selections and negations, in order."""
        return iter(tuple(self._selections))

    def _expand(self, name: str) -> list[str]:
        return list(self._types) if name == "all" else [name]
