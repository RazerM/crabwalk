import platform

from setuptools import setup
from setuptools_rust import RustExtension
from wheel.bdist_wheel import bdist_wheel

ABI_VERSION = "37"


class bdist_wheel_abi3(bdist_wheel):
    def get_tag(self):
        python, abi, plat = super().get_tag()

        if python.startswith("cp"):
            # on CPython, our wheels are abi3 and compatible back to ABI_VERSION
            return f"cp{ABI_VERSION}", "abi3", plat

        return python, abi, plat


setup(
    rust_extensions=[
        RustExtension(
            "crabwalk._lib",
            "src/rust/Cargo.toml",
            py_limited_api=True,
            # Enable abi3 mode if we're not using PyPy.
            features=(
                []
                if platform.python_implementation() == "PyPy"
                else [f"pyo3/abi3-py{ABI_VERSION}"]
            ),
            rust_version=">=1.48.0",
        )
    ],
    cmdclass={"bdist_wheel": bdist_wheel_abi3},
)
