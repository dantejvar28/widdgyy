import gi 

gi.require_version('Gtk', '4.0')

from gi.repository import Gtk
from core.layer_surface import LayerSurface
from core.scene import Scene
from core.config_loader import ConfigLoader

class App(Gtk.Application):
    def do_activate(self):
        scene = Scene()
        loader = ConfigLoader(
            "/home/daniel/.config/widdgyy/config.jsonc"
        )
        loader.load_scene(scene)
        surface=LayerSurface(self,scene)
        surface.show()

app=App()
app.run()
