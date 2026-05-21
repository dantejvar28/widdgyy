import gi 
import cairo

gi.require_version('Gtk', '4.0')
from gi.repository import Gtk
from gi.repository import GdkPixbuf
from gi.repository import Gdk

class Renderer: 
    def __init__(self,window):
        self.window = window

        self.canvas = Gtk.DrawingArea()

        self.canvas.set_draw_func(self.on_draw)

        self.window.set_child(self.canvas)

        self.image = GdkPixbuf.Pixbuf.new_from_file(
            "assets/tree.png"
        ) 
    
    def on_draw(self,area,ctx,width,height):
        self.render_background(ctx)
        self.render_demo(ctx)
    
    def render_background(self,ctx):
        ctx.set_source_rgba(0,0,0,0)
        ctx.set_operator(cairo.OPERATOR_SOURCE)
        ctx.paint()

        ctx.set_operator(cairo.OPERATOR_OVER)
    
    def render_demo(self,ctx):
        ctx.set_source_rgba(1,1,1,1)
        ctx.select_font_face(
            "Sans",
            cairo.FONT_SLANT_NORMAL,
            cairo.FONT_WEIGHT_BOLD
        )
        ctx.set_font_size(42)

        ctx.move_to(100,100)
        ctx.show_text("Widdgyy!")

        Gdk.cairo_set_source_pixbuf(
            ctx,
            self.image,
            300,
            300
        )
        ctx.paint()
