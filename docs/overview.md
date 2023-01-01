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

## Error Handling

Like {func}`os.walk`, errors are ignored by default. You must set an `onerror`
function if you want to report the error or re-raise it to abort the walk.

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
