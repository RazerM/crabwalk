# Thanks to pyca/cryptography for the way to get rust coverage out
# SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause
# SPDX-FileCopyrightText: pyca/cryptography contributors
# SPDX-FileCopyrightText: Frazer McLean
import glob
import re
import sys
from itertools import chain
from pathlib import Path
from uuid import uuid4

import nox

nox.options.reuse_existing_virtualenvs = True
nox.options.default_venv_backend = "uv"


@nox.session(python="3.13")
def typing(session: nox.Session) -> None:
    session.run_install(
        "uv",
        "sync",
        "--group=typing",
        env={"UV_PROJECT_ENVIRONMENT": session.virtualenv.location},
    )
    session.run("mypy", "src/crabwalk", "tests")


@nox.session(python="3.13")
def docs(session: nox.Session) -> None:
    session.run_install(
        "uv",
        "sync",
        "--no-editable",
        "--no-dev",
        "--group=docstest",
        "--reinstall-package=crabwalk",
        env={"UV_PROJECT_ENVIRONMENT": session.virtualenv.location},
    )

    temp_dir = session.create_tmp()
    session.run(
        "sphinx-build",
        "-W",
        "-b",
        "html",
        "-d",
        f"{temp_dir}/doctrees",
        "docs",
        "docs/_build/html",
    )
    session.run(
        "sphinx-build",
        "-W",
        "-b",
        "doctest",
        "-d",
        f"{temp_dir}/doctrees",
        "docs",
        "docs/_build/html",
    )
    session.run("doc8", "docs/")


@nox.session(python=["3.9", "3.10", "3.11", "3.12", "3.13"])
def tests(session: nox.Session) -> None:
    prof_location = Path(".rust-cov", str(uuid4())).absolute()
    rustflags = session.env.get("RUSTFLAGS", "")
    rustflags = f"-Cinstrument-coverage {rustflags}"
    session.env.update(
        {
            "RUSTFLAGS": rustflags,
            "LLVM_PROFILE_FILE": str(prof_location / "cov-%p.profraw"),
        }
    )
    session.run_install(
        "uv",
        "sync",
        "--no-editable",
        "--reinstall-package=crabwalk",
        env={"UV_PROJECT_ENVIRONMENT": session.virtualenv.location},
    )

    if session.posargs:
        tests = session.posargs
    else:
        tests = ["tests/"]

    session.run(
        "coverage",
        "run",
        "-m",
        "pytest",
        *tests,
    )

    libs = glob.glob(
        f"{session.virtualenv.location}/lib/**/crabwalk/_lib.*",
        recursive=True,
    )
    [rust_so] = (lib for lib in libs if not lib.endswith(".pyi"))
    process_rust_coverage(session, [rust_so], prof_location)


LCOV_SOURCEFILE_RE = re.compile(
    r"^SF:.*[\\/]src[\\/]rust[\\/](.*)$", flags=re.MULTILINE
)
BIN_EXT = ".exe" if sys.platform == "win32" else ""


def process_rust_coverage(
    session: nox.Session,
    rust_binaries: list[str],
    prof_raw_location: Path,
) -> None:
    # Hitting weird issues merging Windows and Linux Rust coverage, so just
    # say the hell with it.
    if sys.platform == "win32":
        return

    target_libdir = session.run(
        "rustc", "--print", "target-libdir", external=True, silent=True
    )
    if target_libdir is not None:
        target_bindir = Path(target_libdir).parent / "bin"

        profraws = [
            str(prof_raw_location / p) for p in prof_raw_location.glob("*.profraw")
        ]
        session.run(
            str(target_bindir / ("llvm-profdata" + BIN_EXT)),
            "merge",
            "-sparse",
            *profraws,
            "-o",
            "rust-cov.profdata",
            external=True,
        )

        lcov_data = session.run(
            str(target_bindir / ("llvm-cov" + BIN_EXT)),
            "export",
            rust_binaries[0],
            *chain.from_iterable(["-object", b] for b in rust_binaries[1:]),
            "-instr-profile=rust-cov.profdata",
            "--ignore-filename-regex=[/\\].cargo[/\\]",
            "--ignore-filename-regex=[/\\]rustc[/\\]",
            "--ignore-filename-regex=[/\\].rustup[/\\]toolchains[/\\]",
            "--ignore-filename-regex=[/\\]target[/\\]",
            "--format=lcov",
            silent=True,
            external=True,
        )
        assert isinstance(lcov_data, str)
        lcov_data = LCOV_SOURCEFILE_RE.sub(
            lambda m: "SF:src/rust/" + m.group(1).replace("\\", "/"),
            lcov_data.replace("\r\n", "\n"),
        )
        with open(f"{uuid4()}.lcov", "w") as f:
            f.write(lcov_data)
