import gi 

gi.require_version("Gtk", "4.0")
gi.require_version("Gtk4LayerShell", "1.0")

from gi.repository import Gtk
from gi.repository import Gtk4LayerShell
from gi.repository import Gdk

from core.renderer import Renderer

class LayerSurface:
    def __init__(self,app,scene):
        self.window =Gtk.ApplicationWindow(application=app)

        self.setup_window()
        self.setup_layer_shell()
        self.renderer=Renderer(self.window,scene)
    
    def setup_window(self):
        self.window.set_decorated(False)

        #Transparent background 
        self.window.set_opacity(1.0)

        #Fullscreen
        self.window.fullscreen()
    # CSS test hardcoded for now, should be moved to a separate file
    def setup_css(self):

        css = b"""
        window {
            background: transparent;
        }
        """

        provider = Gtk.CssProvider()
        provider.load_from_data(css)

        Gtk.StyleContext.add_provider_for_display(
            Gdk.Display.get_default(),
            provider,
            Gtk.STYLE_PROVIDER_PRIORITY_APPLICATION
        )

    def setup_layer_shell(self):
        
        Gtk4LayerShell.init_for_window(self.window)

        Gtk4LayerShell.set_layer(
            self.window,
            Gtk4LayerShell.Layer.BACKGROUND
        )

        Gtk4LayerShell.set_keyboard_mode(
            self.window,
            Gtk4LayerShell.KeyboardMode.NONE
        )
    def show(self):
        self.window.present()