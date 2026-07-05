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
        self._native_stickers = set()
        self._background_native_widget = None

        self.overlay = Gtk.Overlay()
        self.canvas = Gtk.DrawingArea()
        self.canvas.set_hexpand(True)
        self.canvas.set_vexpand(True)
        self.overlay.set_child(self.canvas)
        self.update_loop = UpdateLoop(self.canvas,self.scene)

        self.canvas.set_draw_func(self.on_draw)

        self.window.set_child(self.overlay)

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
        active_native_stickers = set()
        background_native_widget = None

        for sticker in stickers:
            if hasattr(sticker, "uses_native_widget") and sticker.uses_native_widget():
                native_widget = sticker.native_widget() if hasattr(sticker, "native_widget") else None
                if native_widget is not None and getattr(sticker, "native_background", lambda: False)():
                    if background_native_widget is None:
                        background_native_widget = native_widget
                else:
                    sticker.attach_native_widget(self.overlay)
                    active_native_stickers.add(sticker)

            if not sticker.visible:
                if hasattr(sticker, "hide_native_widget"):
                    sticker.hide_native_widget()
                continue
            
            w, h = sticker.measure(ctx, width, height)
            
            x, y = sticker.get_position(width, height, w, h)
            
            sticker.render(ctx, x,y, w, h)

        if background_native_widget is not None and background_native_widget is not self._background_native_widget:
            self.overlay.set_child(background_native_widget)
            self.overlay.add_overlay(self.canvas)
            self._background_native_widget = background_native_widget
        elif background_native_widget is None and self._background_native_widget is not None:
            self.overlay.set_child(self.canvas)
            self._background_native_widget = None

        if background_native_widget is not None:
            for sticker in stickers:
                if hasattr(sticker, "native_widget") and sticker.native_widget() is background_native_widget:
                    if hasattr(sticker, "show_native_widget"):
                        sticker.show_native_widget()
                    break

        stale_native_stickers = self._native_stickers - active_native_stickers
        for sticker in stale_native_stickers:
            if hasattr(sticker, "detach_native_widget"):
                sticker.detach_native_widget()

        self._native_stickers = active_native_stickers

    def add_native_widget(self,widget):
        self.overlay.add_overlay(widget)
        
    def remove_native_widget(self, widget):
        self.overlay.remove(widget)
