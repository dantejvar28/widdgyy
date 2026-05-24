from gi.repository import Gdk
from gi.repository import GdkPixbuf

from stickers.base import Sticker

class ImageSticker(Sticker):
    
    def __init__(self,path,**kwargs):
        super().__init__(**kwargs)

        self.path=path
        self.image=GdkPixbuf.Pixbuf.new_from_file(path)

    def render(self,ctx):
        Gdk.cairo_set_source_pixbuf(
            ctx,
            self.image,
            self.x,
            self.y

        )
        ctx.paint()
