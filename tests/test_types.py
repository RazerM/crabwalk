import pickle
from collections.abc import (
    ItemsView,
    Iterator,
    KeysView,
    Mapping,
    MutableMapping,
    ValuesView,
)
from copy import copy, deepcopy

import pytest

from crabwalk import InvalidDefinitionError, Negate, Select, Types


def test_mapping() -> None:
    instance = Types()
    assert issubclass(Types, MutableMapping)
    assert issubclass(Types, Mapping)
    assert isinstance(instance, MutableMapping)
    assert isinstance(instance, Mapping)


def test_new() -> None:
    types = Types({"py": ["*.py"]})
    assert types["py"] == ("*.py",)

    class A:
        def keys(self) -> Iterator[str]:
            yield "rust"

        def __getitem__(self, key: str) -> list[str]:
            if key == "rust":
                return ["*.rs"]
            else:
                raise KeyError(key)

    types = Types(A())
    assert types["rust"] == ("*.rs",)

    types = Types([("js", ["*.js"])])
    assert types["js"] == ("*.js",)

    types = Types(ts=["*.ts"])
    assert types["ts"] == ("*.ts",)


def test_comparison() -> None:
    assert Types({"py": ["*.py"]}) == Types({"py": ["*.py"]})
    assert Types({"py": ["*.py"]}) != Types({"py": ["*.py", "*.pyi"]})
    assert Types({"rs": ["*.rs"], "py": ["*.py"]}) != Types(
        {"rs": ["*.rs"], "py": ["*.py", "*.pyi"]}
    )


def test_update() -> None:
    types = Types()
    types.update({"py": ["*.py"]})
    assert types["py"] == ("*.py",)

    class A:
        def keys(self) -> Iterator[str]:
            yield "rust"

        def __getitem__(self, key: str) -> list[str]:
            if key == "rust":
                return ["*.rs"]
            else:
                raise KeyError(key)

    types = Types()
    types.update(A())
    assert types["rust"] == ("*.rs",)

    types = Types()
    types.update([("js", ["*.js"])])
    assert types["js"] == ("*.js",)

    types = Types()
    types.update(ts=["*.ts"])
    assert types["ts"] == ("*.ts",)


def test_add() -> None:
    types = Types()
    assert "py" not in types
    types.add("py", "*.py")
    assert "py" in types
    assert types["py"] == ("*.py",)


def test_add_defaults() -> None:
    types = Types()
    types.add_defaults()
    assert types["rust"] == ("*.rs",)


def test_copy() -> None:
    types = Types()
    types["py"] = ("*.py",)
    t2 = deepcopy(types)
    assert dict(types) == dict(t2)


def test_pickle() -> None:
    types = Types()
    types.select("rust")
    t2 = pickle.loads(pickle.dumps(types))
    assert dict(types) == dict(t2)
    assert list(types.selections()) == list(t2.selections())


def test_get() -> None:
    missing = object()
    types = Types()
    assert types.get("foo", missing) is missing
    assert types.get("foo") is None
    types["py"] = ("*.py",)
    assert types.get("py") == ("*.py",)


def test_del() -> None:
    types = Types()
    types["py"] = ("*.py",)
    assert "py" in types
    del types["py"]
    assert "py" not in types


def test_keys() -> None:
    types = Types()
    types["py"] = ("*.py",)
    types["rust"] = ("*.rs",)
    assert type(types.keys()) is KeysView
    assert types.keys() == {"py", "rust"}


def test_values() -> None:
    types = Types()
    types["py"] = ("*.py",)
    types["rust"] = ("*.rs",)
    assert type(types.values()) is ValuesView
    assert sorted(types.values()) == [("*.py",), ("*.rs",)]


def test_items() -> None:
    types = Types()
    types["py"] = ("*.py",)
    types["rust"] = ("*.rs",)
    assert type(types.items()) is ItemsView
    assert sorted(types.items()) == [("py", ("*.py",)), ("rust", ("*.rs",))]


def test_comparison_not_implemented() -> None:
    assert Types().__lt__(object()) is NotImplemented  # type: ignore[operator]
    assert Types().__le__(object()) is NotImplemented  # type: ignore[operator]
    assert Types().__ge__(object()) is NotImplemented  # type: ignore[operator]
    assert Types().__gt__(object()) is NotImplemented  # type: ignore[operator]


@pytest.mark.parametrize(
    ("a", "b"),
    [
        (Types(), Types({"py": ("*.py",)})),
        (Types({"rust": ("*.rs",)}), Types({"py": ("*.py",)})),
        (Types({"py": ("*.py",)}), Types({"py": ("*.py", "*.pyi")})),
    ],
)
def test_not_equal(a: Types, b: Types) -> None:
    assert a != b


@pytest.mark.parametrize(
    ("a", "b"),
    [
        (Types(), Types()),
        (Types({"py": ("*.py",)}), Types({"py": ("*.py",)})),
    ],
)
def test_equal(a: Types, b: Types) -> None:
    assert a == b


def test_equality_selections() -> None:
    a = Types()
    a.add_defaults()
    b = Types()
    b.add_defaults()
    assert a == b
    a.select("py")
    assert a != b
    b.select("rust")
    assert a != b


def test_clear() -> None:
    types = Types()
    types.add_defaults()
    assert types
    types.clear()
    assert not types


def test_pop() -> None:
    types = Types({"py": ("*.py",)})
    assert types.pop("py") == ("*.py",)
    with pytest.raises(KeyError, match=r"^'py'$"):
        types.pop("py")
    missing = object()
    assert types.pop("py", missing) is missing


def test_popitem() -> None:
    types = Types({"py": ("*.py",)})
    assert types.popitem() == ("py", ("*.py",))
    with pytest.raises(KeyError):
        types.popitem()


def test_setdefault() -> None:
    types = Types()
    # empty setdefault actually does nothing
    assert types.setdefault("py") == ()
    assert types.setdefault("py", ("*.py",)) == ()

    assert types.setdefault("rust", ("*.rs",)) == ("*.rs",)
    assert types["rust"] == ("*.rs",)


def test_setitem_replaces() -> None:
    types = Types()
    types["py"] = ["*.py"]
    types["py"] = ["*.pyi"]
    assert types["py"] == ("*.pyi",)


def test_setitem_iterable() -> None:
    types = Types()
    types["py"] = (glob for glob in ["*.py", "*.pyi"])
    assert types["py"] == ("*.py", "*.pyi")


def test_setitem_str() -> None:
    types = Types()
    with pytest.raises(TypeError, match="not a str"):
        types["py"] = "*.py"
    assert "py" not in types


def test_setitem_non_str_glob() -> None:
    types = Types()
    with pytest.raises(TypeError, match="globs must be str, not int"):
        types["py"] = [1]  # type: ignore[list-item]
    assert "py" not in types


@pytest.mark.parametrize("name", ["all", "py-3", "*", "a b"])
def test_setitem_invalid_name(name: str) -> None:
    types = Types()
    with pytest.raises(InvalidDefinitionError):
        types[name] = ["*.py"]
    assert name not in types


def test_equal_to_mapping() -> None:
    types = Types({"py": ["*.py"]})
    assert types == {"py": ("*.py",)}
    assert types != {"py": ["*.py"]}
    # Like OrderedDict, extra state is only compared between instances.
    types.select("py")
    assert types == {"py": ("*.py",)}


def test_not_equal_to_other_types() -> None:
    assert Types() != []
    assert Types() != object()


def test_shallow_copy() -> None:
    types = Types({"py": ["*.py"]})
    types.select("py")
    for t2 in [copy(types), types.copy()]:
        assert t2 == types
        t2["rust"] = ["*.rs"]
        t2.negate("py")
        assert "rust" not in types
        assert list(types.selections()) == [Select("py")]


def test_repr() -> None:
    types = Types({"py": ["*.py"]})
    types.negate("py")
    assert repr(types) == ("<Types {'py': ('*.py',)} selections=[Negate(name='py')]>")


def test_select_negate() -> None:
    types = Types({"py": ["*.py"], "rust": ["*.rs"]})
    types.select("py")
    types.negate("rust")
    assert list(types.selections()) == [Select("py"), Negate("rust")]


def test_select_all() -> None:
    types = Types({"py": ["*.py"], "rust": ["*.rs"]})
    types.select("all")
    types["js"] = ["*.js"]
    types.negate("all")
    assert list(types.selections()) == [
        Select("py"),
        Select("rust"),
        Negate("py"),
        Negate("rust"),
        Negate("js"),
    ]


def test_selections_snapshot() -> None:
    types = Types({"py": ["*.py"]})
    selections = types.selections()
    types.select("py")
    assert list(selections) == []


def test_selection_classes() -> None:
    assert Select("py") == Select("py")
    assert Select("py") != Negate("py")  # type: ignore[comparison-overlap]
    assert hash(Select("py")) == hash(Select("py"))
    assert repr(Select("py")) == "Select(name='py')"
    match Negate("py"):
        case Negate(name):
            assert name == "py"
        case _:
            pytest.fail("Negate didn't match")


def test_add_defaults_extends() -> None:
    types = Types({"rust": ["*.rust"]})
    types.add_defaults()
    assert types["rust"] == ("*.rust", "*.rs")
