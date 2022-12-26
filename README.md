<p>
    <a href="https://pypi.org/project/crabwalk/">
        <img src="https://img.shields.io/pypi/v/crabwalk.svg" alt="PyPI" />
    </a>
    <a href="https://parver.readthedocs.io/en/stable/">
        <img src="https://img.shields.io/badge/docs-read%20now-blue.svg" alt="Documentation" />
    </a>
    <a href="https://github.com/RazerM/crabwalk/actions?workflow=CI">
        <img src="https://github.com/RazerM/crabwalk/workflows/CI/badge.svg?branch=main" alt="CI Status" />
    </a>
    <a href="https://raw.githubusercontent.com/RazerM/crabwalk/master/LICENSE">
        <img src="https://img.shields.io/github/license/RazerM/crabwalk.svg" alt="MIT License" />
    </a>
</p>

# crabwalk

<!-- blurb-begin -->

crabwalk is a Python package built in Rust on top of the excellent [ignore][] crate:

> The ignore crate provides a fast recursive directory iterator that respects
> various filters such as globs, file types and .gitignore files.

[ignore]: https://github.com/BurntSushi/ripgrep/tree/master/crates/ignore

<!-- blurb-end -->

## Examples

### Defaults

By default, `Walk` will recursively traverse the given path(s) while ignoring
hidden files and those matching globs found in `.gitignore` and `.ignore` files:

```python
from crabwalk import Walk

with Walk(".") as walk:
    for entry in walk:
        print(entry.path)
```

### Disable standard filters

To disable all default filtering (and therefore behave more like `os.walk`),
you can set the corresponding flags individually or use the helper method shown
here:

```python
from crabwalk import Walk

with Walk(".") as walk:
    walk.disable_standard_filters()
    for entry in walk:
        print(entry.path)
```

### Advanced

Disable the hidden files filter and enable parsing of custom ignore files called
`.myignore`:

```python
from crabwalk import Walk

with Walk(".", hidden=False, custom_ignore_filenames=(".myignore",)) as walk:
    for entry in walk:
        print(entry.path)
```

See the documentation for all options.
