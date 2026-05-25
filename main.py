import gi 

gi.require_version('Gtk', '4.0')

from gi.repository import Gtk
from core.layer_surface import LayerSurface
from core.scene import Scene
from core.config_loader import ConfigLoader
from core.css_loader import CSSLoader
from core.hot_reload import HotReload

class App(Gtk.Application):
    def do_activate(self):
        scene = Scene()
        loader = ConfigLoader(
            "/home/daniel/.config/widdgyy/config.jsonc"
        )
        loader.load_scene(scene)

        css_loader=CSSLoader(
            "/home/daniel/.config/widdgyy/styles.css"
        )
        css_loader.load()
        
        hot_reload = HotReload(
            scene,
            loader,
            css_loader
        )
        hot_reload.start()

        for sticker in scene.stickers:
            sticker.style=css_loader.get_style(
                sticker.css_class
                )

        surface=LayerSurface(self,scene)
        surface.show()

app=App()
app.run()
