from gi.repository import Gdk
from gi.repository import GdkPixbuf

from stickers.base import Sticker

class ImageSticker(Sticker):
    
    def __init__(
            self,
            path,
            
            width=None,
            height=None,
            **kwargs
    ):
        super().__init__(**kwargs)

        self.path=path
        self.original_image=GdkPixbuf.Pixbuf.new_from_file(path)
        self.width=width
        self.height=height
        self.image=self.create_scaled_image()

    def render(self,ctx,screen_width,screen_height):
        real_x, real_y = self.get_position(screen_width, screen_height)
        Gdk.cairo_set_source_pixbuf(
            ctx,
            self.image,
            real_x,
            real_y

        )
        ctx.paint()
    
    def create_scaled_image(self):
        if self.width and self.height:

            return self.original_image.scale_simple(
                self.width,
                self.height,
                GdkPixbuf.InterpType.BILINEAR
            )
        return self.original_image
