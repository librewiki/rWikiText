import os

from setuptools import Distribution, setup

try:
    from setuptools.command.bdist_wheel import bdist_wheel as _bdist_wheel
except ImportError:
    from wheel.bdist_wheel import bdist_wheel as _bdist_wheel


class BinaryDistribution(Distribution):
    def has_ext_modules(self):
        return True


class PlatformWheel(_bdist_wheel):
    def finalize_options(self):
        plat_name = os.environ.get("RWIKITEXT_PLAT_NAME")
        if plat_name:
            self.plat_name = plat_name
            self.plat_name_supplied = True
        super().finalize_options()
        self.root_is_pure = False

    def get_tag(self):
        _, _, plat_tag = super().get_tag()
        return ("py3", "none", plat_tag)


setup(
    distclass=BinaryDistribution,
    cmdclass={"bdist_wheel": PlatformWheel},
)
