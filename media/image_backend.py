import cairo 
from media.base_backend import MediaBackend
from gi.repository import GdkPixbuf
from gi.repository import Gdk

class ImageBackend(MediaBackend):

    def __init__(self, path):
        self.path = path

        self.pixbuf = GdkPixbuf.Pixbuf.new_from_file(self.path)

        self.original_width = self.pixbuf.get_width()
        self.original_height = self.pixbuf.get_height()
        

    def measure(
        self,
        ctx,
        screen_width,
        screen_height
    ):
        return (
            self.original_width,
            self.original_height
        )

    def render(
        self,
        ctx,
        x,
        y,
        w,
        h
    ):
        ctx.save()

        ctx.translate(x, y)

        ctx.scale(
            w / self.original_width,
            h / self.original_height
        )
        Gdk.cairo_set_source_pixbuf(ctx, self.pixbuf, 0, 0)

        ctx.paint()

        ctx.restore()