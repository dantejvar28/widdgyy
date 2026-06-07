import os
import sys
import ctypes.util


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

import gi 

gi.require_version('Gtk', '4.0')

from gi.repository import Gtk
from core.layer_surface import LayerSurface
from core.scene import Scene
from core.config_loader import ConfigLoader
from core.css_loader import CSSLoader
from core.hot_reload import HotReload
from utils.paths import user_config_dir

class App(Gtk.Application):
    def do_activate(self):
        scene = Scene()
        config_dir = user_config_dir()
        loader = ConfigLoader(config_dir / "config.jsonc")
        loader.load_scene(scene)

        css_loader=CSSLoader(config_dir / "styles.css")
        css_loader.load()
        
        hot_reload = HotReload(
            scene,
            loader,
            css_loader
        )
        hot_reload.start()

        for sticker in scene.stickers:
            hot_reload.apply_styles(sticker)

        surface=LayerSurface(self,scene)
        surface.show()

app=App()
app.run()
