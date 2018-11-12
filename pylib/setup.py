import os

from setuptools import setup

from setuptools_rust import Binding, RustExtension

pylib_force_debug = os.getenv("PYLIB_FORCE_DEBUG", "").lower()
if pylib_force_debug == "1" or pylib_force_debug == "true":
    force_debug = True
elif pylib_force_debug == "0" or pylib_force_debug == "false":
    force_debug = False
else:
    force_debug = None


setup(
    name="pylib",
    version="1.0.1",
    rust_extensions=[
        RustExtension(
            "pylib.pylib", "Cargo.toml", binding=Binding.PyO3, debug=force_debug
        )
    ],
    packages=["pylib"],
    # rust extensions are not zip safe, just like C-extensions.
    zip_safe=False,
)
