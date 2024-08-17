from setuptools import setup
from wheel.bdist_wheel import bdist_wheel

ABI_VERSION = "37"


class bdist_wheel_abi3(bdist_wheel):
    def get_tag(self):
        python, abi, plat = super().get_tag()

        if python.startswith("cp"):
            # on CPython, our wheels are abi3 and compatible back to ABI_VERSION
            return f"cp{ABI_VERSION}", "abi3", plat

        return python, abi, plat


setup(cmdclass={"bdist_wheel": bdist_wheel_abi3})
