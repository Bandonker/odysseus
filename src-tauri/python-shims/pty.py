"""Windows-only shim for Odysseus desktop local launches.

The app's shell route imports Unix PTY modules at import time. The desktop
wrapper keeps core app code untouched and places this shim on PYTHONPATH only
for Windows local launches so non-PTY routes can start normally.
"""


def openpty():
    raise OSError("PTY shell streaming is unavailable on Windows local desktop launches")
