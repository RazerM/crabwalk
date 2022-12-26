import pickle
from collections.abc import Mapping, MutableMapping
from copy import deepcopy
from typing import Iterator, List

from crabwalk import Types


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

        def __getitem__(self, key: str) -> List[str]:
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


def test_update() -> None:
    types = Types()
    types.update({"py": ["*.py"]})
    assert types["py"] == ("*.py",)

    class A:
        def keys(self) -> Iterator[str]:
            yield "rust"

        def __getitem__(self, key: str) -> List[str]:
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
    assert t2.__getstate__() == {"selections": [("select", "rust")]}
