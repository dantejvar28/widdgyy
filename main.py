import gi 

gi.require_version('Gtk', '4.0')

from gi.repository import Gtk
from core.layer_surface import LayerSurface

class App(Gtk.Application):
    def do_activate(self):
        surface=LayerSurface(self)
        surface.show()

app=App()
app.run()
