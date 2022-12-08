API Reference
=============

.. currentmodule:: crabwalk

.. autoclass:: Walk

    Recursive directory iterator which yields :class:`DirEntry` objects.

    If ``Walk`` is not closed (either by using a ``with`` statement or calling
    :meth:`close` explicitly) then a :class:`ResourceWarning` will be emittted
    in its destructor.

    :param paths: Paths to iterate recursively.
    :type paths: typing.Union[str, os.PathLike[str]]
    :param max_depth: The maximum depth to recurse.
    :type max_depth: typing.Optional[int]
    :param follow_links: Whether to follow symbolic links or not.
    :type follow_links: bool
    :param max_filesize: Whether to ignore files above the specified limit.
    :type max_filesize: typing.Optional[int]
    :param global_ignore_files: Paths to global ignore files. These have lower
        precedence than all other sources of ignore rules.
    :type global_ignore_files: typing.Sequence[typing.Union[str, os.PathLike[str]]]
    :param custom_ignore_filenames: Custom ignore file names. These have higher
        precedence than all other ignore files.
    :type custom_ignore_filenames: typing.Sequence[str]
    :param overrides: Add an override matcher.
    :type overrides: typing.Optional[Overrides]
    :param types: Add a file type matcher.
    :type types: typing.Optional[Types]
    :param hidden: Enables ignoring hidden files.
    :type hidden:  bool
    :param parents: Enables reading ignore files from parent directories. When
        enabled, ``.gitignore`` files in parent directories of each file path
        given are respected. Otherwise, they are ignored.
    :type parents:  bool
    :param ignore: Enables reading ``.ignore`` files.

        ``.ignore`` files have the same semantics as gitignore files and are
        supported by search tools such as ripgrep and The Silver Searcher.
    :type ignore:  bool
    :param git_global: Enables reading a global gitignore file, whose path is
        specified in git's ``core.excludesFile`` config option.

        Git's config file location is ``$HOME/.gitconfig``. If ``$HOME/.gitconfig``
        does not exist or does not specify ``core.excludesFile``, then
        ``$XDG_CONFIG_HOME/git/ignore`` is read. If ``$XDG_CONFIG_HOME`` is not
        set or is empty, then ``$HOME/.config/git/ignore`` is used instead.
    :type git_global:  bool
    :param git_ignore: Enables reading ``.gitignore`` files.
    :type git_ignore:  bool
    :param git_exclude: Enables reading ``.git/info/exclude`` files.

        ``.git/info/exclude`` files have match semantics as described in the
        gitignore man page.
    :type git_exclude:  bool
    :param require_git: Whether a git repository is required to apply git-related
        ignore rules (global rules, ``.gitignore`` and local exclude rules).
    :type require_git:  bool
    :param ignore_case_insensitive: Process ignore files case insensitively.
    :type ignore_case_insensitive: bool
    :param sort: May be true to sort entries by file path, or a callable to
        extract a comparison key based on the file path (like the ``key``
        argument to :func:`sorted`).
    :type sort: typing.Union[typing.Callable[[str], SupportsRichComparison], bool]
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
        and skips descending into directories that do not satify the given
        predicate.
    :type filter_entry: typing.Optional[typing.Callable[[DirEntry], bool]]
    :param onerror: By default, errors are ignored. You may specify a function
        to either log the error or re-raise it.
    :type onerror: typing.Optional[typing.Callable[[Exception], None]]

    .. method:: disable_standard_filters()

        Disable the :attr:`hidden`, :attr:`parents`, :attr:`ignore`,
        :attr:`git_ignore`, :attr:`git_global`, and :attr:`git_exclude` filters.

    .. method:: enable_standard_filters()

        Enable the :attr:`hidden`, :attr:`parents`, :attr:`ignore`,
        :attr:`git_ignore`, :attr:`git_global`, and :attr:`git_exclude` filters.

    .. method:: close()

        Close the iterator and free acquired resources

        It is recommended to use a ``with`` statement instead.

.. autoclass:: DirEntry

    Object yielded by :class:`Walk` to expose the file path and other file
    attributes of a directory entry.

    The interface is *similar*—but not identical—to :class:`os.DirEntry`.

    ``DirEntry`` implements the :class:`os.PathLike` interface.

    .. attribute:: path
        :type: str

        The entry's full path name. The path is only absolute if the
        :class:`Walk` path argument was absolute.

    .. method:: inode() -> int

        Return the inode number of the entry.
    .. method:: is_dir() -> bool

        Returns whether this entry is a directory or if :class:`Walk` was
        configured with ``follow_links=True`` and this is a symbolic link
        pointing to a directory.
    .. method:: is_file() -> bool

        Returns whether this entry is a file or if :class:`Walk` was
        configured with ``follow_links=True`` and this is a symbolic link
        pointing to a file.
    .. method:: is_symlink() -> bool

        Returns whether this entry is a symbolic link.

    .. attribute:: file_name
        :type: str

        Return the file name of this entry.

        If this entry has no file name (e.g., ``/``), then the full path is
        returned.

    .. attribute:: depth
        :type: int

        The depth at which this entry was created relative to the root.

.. autoclass:: Types

.. autoclass:: Override

.. autoclass:: Overrides
