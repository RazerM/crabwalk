# Overview

## Typical Usage

```{currentmodule} crabwalk

```

It is recommended to use {class}`Walk` as a context manager, otherwise make sure to call {meth}`Walk.close` when you're done iterating.

```python
with Walk(".") as walk:
    for entry in walk:
        print(entry.path)
```

## File Types

{class}`Types` maps file type names to globs. Selecting a file type only yields
files which match it, and negating one skips files which match it. Directories
are still traversed.

```python
types = Types()
types.add_defaults()  # The same types as ripgrep, see `rg --type-list`
types["blorg"] = ["*.blorg"]
types.select("rust")
types.select("blorg")

with Walk(".", types=types) as walk:
    for entry in walk:
        print(entry.path)
```

## Overrides

{class}`Overrides` are globs which take precedence over all ignore files. A
glob starting with `!` ignores matching paths. If there are any other globs,
files which don't match one of them are ignored.

```python
overrides = Overrides(["*.py", "!test_*.py"], path=".")

with Walk(".", overrides=overrides) as walk:
    for entry in walk:
        print(entry.path)
```

## Error Handling

Like {func}`os.walk`, errors are ignored by default. You must set an `onerror`
function if you want to report the error or re-raise it to abort the walk.

File system errors are passed as {exc}`OSError` subclasses (like
{exc}`PermissionError`) with `errno`, `strerror`, and `filename` set, the same
as {func}`os.scandir` would raise. If the error happened while walking, it also
has a `depth` attribute. Other errors are subclasses of {exc}`WalkError`.

```python
def onerror(exc):
    print(exc)


with Walk(".", onerror=onerror) as walk:
    for entry in walk:
        print(entry)
```

## Mutable Attributes

All the arguments that can be passed to {class}`Walk` can also be accessed as
attributes on the instance. Mutations are only possible if iteration hasn't
yet started.

```python
with Walk(".") as walk:
    walk.paths.append("/foo")
    walk.hidden = False
    for entry in walk:
        ...
```

Modifying attributes after iteration starts is an error:

```{testcode}
with Walk(".") as walk:
    for entry in walk:
        walk.hidden = True
```

```{testoutput}
Traceback (most recent call last):
    ...
RuntimeError: This property is read-only once iteration has started
```
