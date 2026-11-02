API Reference
=============

.. currentmodule:: crabwalk

.. autoclass:: Walk

    Recursive directory iterator which yields :class:`DirEntry` objects.

    Like :func:`os.scandir`, ``Walk`` closes itself once iteration is
    finished. If it is not exhausted or closed (either by using a ``with``
    statement or calling :meth:`close` explicitly) then a
    :class:`ResourceWarning` will be emitted in its destructor.

    :param paths: Paths to iterate recursively.
    :type paths: str | os.PathLike[str]
    :param max_depth: The maximum depth to recurse.
    :type max_depth: int | None
    :param min_depth: The minimum depth of entries to yield. Shallower
        directories are still traversed.
    :type min_depth: int | None
    :param follow_symlinks: Whether to follow symbolic links or not.
    :type follow_symlinks: bool
    :param max_filesize: Ignore files larger than this many bytes.
    :type max_filesize: int | None
    :param global_ignore_files: Paths to global ignore files. These have lower
        precedence than all other sources of ignore rules.
    :type global_ignore_files: collections.abc.Sequence[str | os.PathLike[str]]
    :param custom_ignore_filenames: Custom ignore file names. These have higher
        precedence than all other ignore files.
    :type custom_ignore_filenames: collections.abc.Sequence[str]
    :param overrides: Add an override matcher.
    :type overrides: Overrides | None
    :param types: Add a file type matcher.
    :type types: Types | None
    :param hidden: Enables ignoring hidden files.
    :type hidden: bool
    :param parents: Enables reading ignore files from parent directories. When
        enabled, ``.gitignore`` files in parent directories of each file path
        given are respected. Otherwise, they are ignored.
    :type parents: bool
    :param ignore: Enables reading ``.ignore`` files.

        ``.ignore`` files have the same semantics as gitignore files and are
        supported by search tools such as ripgrep and The Silver Searcher.
    :type ignore: bool
    :param git_global: Enables reading a global gitignore file, whose path is
        specified in git's ``core.excludesFile`` config option.

        Git's config file location is ``$HOME/.gitconfig``. If ``$HOME/.gitconfig``
        does not exist or does not specify ``core.excludesFile``, then
        ``$XDG_CONFIG_HOME/git/ignore`` is read. If ``$XDG_CONFIG_HOME`` is not
        set or is empty, then ``$HOME/.config/git/ignore`` is used instead.
    :type git_global: bool
    :param git_ignore: Enables reading ``.gitignore`` files.
    :type git_ignore: bool
    :param git_exclude: Enables reading ``.git/info/exclude`` files.

        ``.git/info/exclude`` files have match semantics as described in the
        gitignore man page.
    :type git_exclude: bool
    :param require_git: Whether a git repository is required to apply git-related
        ignore rules (global rules, ``.gitignore`` and local exclude rules).
    :type require_git: bool
    :param ignore_case_insensitive: Process ignore files case insensitively.
    :type ignore_case_insensitive: bool
    :param sort: May be true to sort the entries in each directory by file
        path, or a callable to extract a comparison key based on the file path
        (like the ``key`` argument to :func:`sorted`).
    :type sort: bool | collections.abc.Callable[[str], typing.Any]
    :param same_file_system: Do not cross file system boundaries.
    :type same_file_system: bool
    :param skip_stdout: Do not yield directory entries that are believed to
        correspond to stdout.

        This is useful when a command is invoked via shell redirection to a file
        that is also being read. For example, ``grep -r foo ./ > results`` might
        end up trying to search ``results`` even though it is also writing to it,
        which could cause an unbounded feedback loop. Setting this option
        prevents this from happening by skipping over the ``results`` file.
    :type skip_stdout: bool
    :param filter_entry: Yields only entries which satisfy the given predicate
        and skips descending into directories that do not satisfy the given
        predicate.
    :type filter_entry: collections.abc.Callable[[DirEntry], bool] | None
    :param onerror: By default, errors are ignored. You may specify a function
        to either log the error or re-raise it.
    :type onerror: collections.abc.Callable[[Exception], None] | None

    .. method:: disable_standard_filters() -> None

        Disable the :attr:`hidden`, :attr:`parents`, :attr:`ignore`,
        :attr:`git_ignore`, :attr:`git_global`, and :attr:`git_exclude` filters.

    .. method:: enable_standard_filters() -> None

        Enable the :attr:`hidden`, :attr:`parents`, :attr:`ignore`,
        :attr:`git_ignore`, :attr:`git_global`, and :attr:`git_exclude` filters.

    .. method:: close() -> None

        Close the iterator and free acquired resources. Further iteration
        raises :exc:`StopIteration`.

        It is recommended to use a ``with`` statement instead.

.. autoclass:: DirEntry

    Object yielded by :class:`Walk` to expose the file path and other file
    attributes of a directory entry.

    The interface is *similar*—but not identical—to :class:`os.DirEntry`.

    ``DirEntry`` implements the :class:`os.PathLike` interface.

    .. attribute:: name
        :type: str

        Return the base filename of this entry.

        .. admonition:: ``os.DirEntry`` difference
            :class: stdlib-difference

            If this entry has no file name (e.g., ``/``), then the full path is
            returned.

    .. attribute:: path
        :type: str

        The entry's full path name. The path is only absolute if the
        :class:`Walk` path argument was absolute.

    .. method:: inode() -> int

        Return the inode number of the entry.

        If ``follow_symlinks=True`` and this entry is a symbolic link, the inode
        number of the target is returned.

        .. admonition:: ``os.DirEntry`` difference
            :class: stdlib-difference

            In contrast, :class:`os.DirEntry` always returns the inode number of
            the symbolic link itself.

            Use ``os.stat(entry, follow_symlinks=False).st_ino`` if that's what
            you want.

        On the first, uncached call, a system call is required on Windows but
        not on Unix.

    .. method:: is_dir() -> bool

        Returns whether this entry is a directory or if :class:`Walk` was
        configured with ``follow_symlinks=True`` and this is a symbolic link
        pointing to a directory.

        Never makes any system calls.

        .. admonition:: ``os.DirEntry`` difference
            :class: stdlib-difference

            There is no ``follow_symlinks`` argument, as it is configured
            via :class:`Walk`.

    .. method:: is_file() -> bool

        Returns whether this entry is a file or if :class:`Walk` was
        configured with ``follow_symlinks=True`` and this is a symbolic link
        pointing to a file.

        Never makes any system calls.

        .. admonition:: ``os.DirEntry`` difference
            :class: stdlib-difference

            There is no ``follow_symlinks`` argument, as it is configured
            via :class:`Walk`.

    .. method:: is_symlink() -> bool

        Returns whether this entry is a symbolic link.

    .. method:: stat() -> os.stat_result

        Returns a :class:`~os.stat_result` object for this entry. Follows
        symbolic links if :class:`Walk` was configured with
        ``follow_symlinks=True``.

        Always makes a system call.

        .. admonition:: ``os.DirEntry`` difference
            :class: stdlib-difference

            There is no ``follow_symlinks`` argument, as it is configured
            via :class:`Walk`.

    .. attribute:: depth
        :type: int

        The depth at which this entry was created relative to the root.

    .. attribute:: follow_symlinks
        :type: bool

        Whether this entry is configured to follow symlinks or not (inherited
        from the :class:`Walk` instance which yielded it).

.. autoclass:: Types
    :members: add, add_defaults, select, negate, selections

.. autoclass:: Select

.. autoclass:: Negate

.. autoclass:: Override

.. autoclass:: Overrides

Exceptions
----------

.. exception:: WalkError

    Base class for all exceptions raised by the ``crabwalk`` package.

    .. attribute:: line

        A line number if this error is associated with one.

    .. attribute:: path

        A file path if this error is associated with one.

    .. attribute:: depth

        A directory depth if this error is associated with one while recursively
        walking a directory.

.. exception:: LoopError

    An error that occurs when a file loop is detected when traversing symbolic
    links.

    .. attribute:: ancestor

        The ancestor file path in the loop.

    .. attribute:: child

        The child file path in the loop.

.. exception:: GlobError

    An error that occurs when trying to parse a glob.

    .. attribute:: glob

        The glob which caused the error, if available.

.. exception:: PartialError

    A collection of “soft” errors. These occur when adding an ignore file
    partially succeeded.

    .. attribute:: errors

        The errors, which may include :exc:`OSError` and :exc:`WalkError`
        instances.

.. exception:: InvalidDefinitionError

    A file type name is invalid. File type names must only contain letters and
    numbers, and can't be ``all``.

    This is also a :exc:`ValueError`.

    .. attribute:: name

        The invalid file type name.

.. exception:: UnrecognizedFileTypeError

    A type selection for a file type that is not defined.

    .. attribute:: name

        The name of the file type.
