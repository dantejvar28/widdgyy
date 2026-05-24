from gi.repository import Gtk

from core.config_loader import ConfigLoader
from core.scene import Scene 
from core.layer_surface import LayerSurface 

from stickers.text import TextSticker
from stickers.image import ImageSticker

class App(Gtk.Application):

    def do_activate(self):
        scene = Scene()
        loader = ConfigLoader(
            "/home/daniel/.config/widdgyy/config.jsonc"
        )

        loader.load_scene(scene)
        surface=LayerSurface(self,scene)
        surface.show()