import os
import sys
import ctypes.util
from core.app import App

def _ensure_layer_shell_preload():
    marker = "WIDDGYY_LAYER_PRELOAD_DONE"
    if os.environ.get(marker) == "1":
        return

    lib_name = ctypes.util.find_library("gtk4-layer-shell")
    if not lib_name:
        return

    current = os.environ.get("LD_PRELOAD", "").strip()
    entries = [p for p in current.split() if p]
    if lib_name in entries:
        os.environ[marker] = "1"
        return

    os.environ["LD_PRELOAD"] = f"{lib_name} {current}".strip()
    os.environ[marker] = "1"
    os.execvpe(sys.executable, [sys.executable, *sys.argv], os.environ)


_ensure_layer_shell_preload()


if __name__ == "__main__":
    app=App()
    app.run()