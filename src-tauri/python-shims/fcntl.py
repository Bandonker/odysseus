"""Windows-only fcntl shim for Odysseus desktop local launches."""

F_GETFL = 3
F_SETFL = 4


def fcntl(*_args, **_kwargs):
    raise OSError("fcntl is unavailable on Windows local desktop launches")
