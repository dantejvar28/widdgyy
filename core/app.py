import gi 

gi.require_version('Gtk', '4.0')
gi.require_version('Gst', '1.0')

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