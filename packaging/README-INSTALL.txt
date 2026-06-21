Drox TUI — installation
=======================

Launch (after install):  drox-tui --workspace /path/to/project

Product version: see VERSION file after install.
Engine baseline: Drox IDE agent engine 1.5.0 (local-first, no Drox cloud telemetry).

Requirements
------------
- Windows 10+ or Linux x86_64
- Terminal: Windows Terminal (recommended) or modern Linux terminal
- Ollama (or compatible LLM) on your machine — default http://127.0.0.1:11434

Windows
-------
1. Run the installer: drox-tui-*-windows-x64-setup.exe
2. Follow the wizard (recommended: add to PATH).
3. Open a new terminal and run:  drox-tui --workspace C:\path\to\project

Linux
-----
1. Extract:  tar xzf drox-tui-*-linux-x64.tar.gz
2. cd into the extracted folder.
3. Run:  ./install.sh
4. Run:  drox-tui --workspace ~/your-project

Uninstall
---------
Windows: Settings → Apps → Drox TUI (or remove %LOCALAPPDATA%\Programs\DroxTUI)
Linux:   rm ~/.local/bin/drox-tui  (or /usr/local/bin/drox-tui if --system was used)

Documentation: https://github.com/DroxKiwi/Drox---TUI---OR
