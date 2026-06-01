# Odysseus Windows Portable

## Start

1. Unzip `Odysseus-Windows-Portable.zip`.
2. Open the extracted folder.
3. Double-click `launch-windows.bat`.

The desktop app starts the local Odysseus webserver for you. On first run it can
take several minutes while Python dependencies install. A startup window shows
live progress logs and opens Odysseus automatically when the server is ready.

## Requirements

- Windows 10 or Windows 11
- Python 3.11 or newer on PATH

No Docker, Node.js, Rust, or browser setup is required for the portable zip.

## Troubleshooting

- If the app says Python was not found, install Python 3.11 or newer and enable
  "Add Python to PATH" during installation.
- If the app is already running, `launch-windows.bat` will not open a duplicate
  copy.
- Startup logs are written to `logs\desktop-local.log`.
