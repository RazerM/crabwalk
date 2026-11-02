import gc
import os
import sys
import time
import warnings
import weakref
from pathlib import Path

import pytest

from crabwalk import DirEntry, Walk


@pytest.fixture(name="big_tree", scope="module")
def big_tree_fixture(tmp_path_factory: pytest.TempPathFactory) -> Path:
    # Big enough for sequential walks to move to a background thread.
    root = tmp_path_factory.mktemp("big")
    for i in range(30):
        directory = root / f"dir{i:02}"
        directory.mkdir()
        (directory / ".ignore").write_text("*.skip\n")
        for j in range(60):
            (directory / f"file{j:02}.txt").touch()
            (directory / f"file{j:02}.skip").touch()
    return root


def walk_paths(walk: Walk) -> list[str]:
    with walk:
        return [entry.path for entry in walk]


def test_threads_attribute() -> None:
    walk = Walk(".")
    assert walk.threads == 1
    walk.threads = 0
    assert walk.threads == 0
    with walk:
        next(walk)
        with pytest.raises(RuntimeError, match="read-only"):
            walk.threads = 1


@pytest.mark.parametrize("threads", [0, 2])
@pytest.mark.parametrize("standard_filters", [True, False])
def test_parallel(big_tree: Path, threads: int, standard_filters: bool) -> None:
    sequential = Walk(big_tree)
    parallel = Walk(big_tree, threads=threads)
    if not standard_filters:
        sequential.disable_standard_filters()
        parallel.disable_standard_filters()
    expected = walk_paths(sequential)
    assert len(expected) == 1 + 30 * (61 if standard_filters else 122)
    assert sorted(walk_paths(parallel)) == sorted(expected)


def test_sequential_order(big_tree: Path) -> None:
    # Large sequential walks continue in a background thread, which must not
    # change the order.
    paths = walk_paths(Walk(big_tree, sort=True))
    expected = [str(big_tree)]
    for i in range(30):
        directory = big_tree / f"dir{i:02}"
        expected.append(str(directory))
        expected.extend(str(directory / f"file{j:02}.txt") for j in range(60))
    assert paths == expected

    unsorted = walk_paths(Walk(big_tree))
    assert sorted(unsorted) == sorted(expected)


def test_parallel_sort(big_tree: Path) -> None:
    with (
        pytest.raises(ValueError, match="sort is not supported"),
        Walk(big_tree, threads=2, sort=True) as walk,
    ):
        next(walk)


@pytest.mark.parametrize("threads", [1, 2])
def test_close_early(big_tree: Path, threads: int) -> None:
    with Walk(big_tree, threads=threads) as walk:
        walk.disable_standard_filters()
        # Read past the first batches so that any producer threads are busy.
        for _ in range(1500):
            next(walk)
        walk.close()
        with pytest.raises(StopIteration):
            next(walk)


@pytest.mark.parametrize("threads", [1, 2])
def test_filter_entry_threads(big_tree: Path, threads: int) -> None:
    def filter_entry(entry: DirEntry) -> bool:
        return not entry.name.startswith("dir1")

    walk = Walk(big_tree, threads=threads, filter_entry=filter_entry)
    paths = walk_paths(walk)
    assert len(paths) == 1 + 20 * 61
    assert not any(os.path.basename(path).startswith("dir1") for path in paths)


def test_filter_entry_error_parallel(big_tree: Path) -> None:
    def filter_entry(entry: DirEntry) -> bool:
        raise ZeroDivisionError(entry.path)

    with pytest.raises(ZeroDivisionError):
        walk_paths(Walk(big_tree, threads=2, filter_entry=filter_entry))


@pytest.mark.skipif(
    sys.platform == "win32" or os.geteuid() == 0,
    reason="needs POSIX permissions and a non-root user",
)
def test_onerror_parallel(tmp_path: Path) -> None:
    locked = tmp_path / "locked"
    locked.mkdir()
    locked.chmod(0)
    errors: list[Exception] = []
    try:
        paths = walk_paths(Walk(tmp_path, threads=2, onerror=errors.append))
    finally:
        locked.chmod(0o700)
    assert sorted(paths) == [str(tmp_path), str(locked)]
    [error] = errors
    assert isinstance(error, PermissionError)
    assert error.filename == str(locked)


class Callback:
    """Returns its argument, so it works as filter_entry and as a sort key."""

    walk: Walk

    def __call__(self, arg: object) -> object:
        return arg


@pytest.mark.parametrize(
    "kwargs",
    [
        pytest.param({"filter_entry": True}, id="filter_entry"),
        pytest.param({"sort": True}, id="sort"),
        pytest.param({"filter_entry": True, "threads": 2}, id="filter_entry-threads"),
    ],
)
def test_callback_cycle_collected(big_tree: Path, kwargs: dict[str, object]) -> None:
    # A callback that references its walk must be collectable while the walk
    # is in progress, even though the walker holds it too.
    callback = Callback()
    callbacks = {name: callback for name in kwargs if name != "threads"}
    walk = Walk(big_tree, threads=kwargs.get("threads", 1), **callbacks)  # type: ignore[arg-type]
    callback.walk = walk
    ref = weakref.ref(callback)
    for _ in range(100):
        next(walk)
    del walk, callback, callbacks

    with warnings.catch_warnings():
        warnings.simplefilter("ignore", ResourceWarning)
        # A worker thread may briefly hold the callback while calling it.
        deadline = time.monotonic() + 5
        while ref() is not None and time.monotonic() < deadline:
            gc.collect()
            time.sleep(0.01)
    assert ref() is None
