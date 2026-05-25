import gi 
import cairo

gi.require_version('Gtk', '4.0')
from gi.repository import Gtk
from gi.repository import GdkPixbuf
from gi.repository import Gdk

from core.update_loop import UpdateLoop

class Renderer: 
    def __init__(self,window,scene):
        self.window = window
        self.scene = scene

        self.canvas = Gtk.DrawingArea()
        self.update_loop = UpdateLoop(self.canvas,self.scene)

        self.canvas.set_draw_func(self.on_draw)

        self.window.set_child(self.canvas)

        self.update_loop.start()
    
    def on_draw(self,area,ctx,width,height):
        self.render_background(ctx)
        self.render_scene(ctx, width, height)
    
    def render_background(self,ctx):
        ctx.set_source_rgba(0,0,0,0)
        ctx.set_operator(cairo.OPERATOR_SOURCE)
        ctx.paint()

        ctx.set_operator(cairo.OPERATOR_OVER)
    
    def render_scene(self,ctx,width,height):
        stickers = sorted(
            self.scene.stickers,
            key=lambda s: s.z_index
        )
        for sticker in self.scene.stickers:
            if sticker.visible:
                sticker.render(ctx, width, height)