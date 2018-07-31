from setuptools import setup
from setuptools_rust import Binding, RustExtension

setup(
    name="pylib",
    version="1.0.1",
    rust_extensions=[RustExtension("pylib.pylib", "Cargo.toml", binding=Binding.PyO3)],
    packages=["pylib"],
    # rust extensions are not zip safe, just like C-extensions.
    zip_safe=False,
)
