from pathlib import Path

import pytest

from crabwalk import Walk


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


def test_close_early(big_tree: Path) -> None:
    with Walk(big_tree) as walk:
        walk.disable_standard_filters()
        # Read past the first batches so that the producer thread is busy.
        for _ in range(1500):
            next(walk)
        walk.close()
        with pytest.raises(StopIteration):
            next(walk)
