import platform

from setuptools import setup
from setuptools_rust import RustExtension

setup(
    rust_extensions=[
        RustExtension(
            "crabwalk._lib",
            "src/rust/Cargo.toml",
            py_limited_api=True,
            # Enable abi3 mode if we're not using PyPy.
            features=(
                [] if platform.python_implementation() == "PyPy" else ["pyo3/abi3-py37"]
            ),
            rust_version=">=1.48.0",
        )
    ]
)
