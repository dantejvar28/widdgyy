from gi.repository import Gdk
from gi.repository import GdkPixbuf

from stickers.base import Sticker
from utils.units import resolve_unit

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
        self.cached_image = None
        self.cached_width = None
        self.cached_height = None

    def render(self,ctx,screen_width,screen_height,x,y,w,h):

        image = self.original_image
        
        original_width = self.original_image.get_width()
        original_height = self.original_image.get_height()

        target_width = w
        target_height = h

        if target_width and target_height:
            scale = min(
                target_width / original_width,
                target_height / original_height
            )
            width = max(1,int(original_width * scale))
            height = max(1,int(original_height * scale))

            image = image.scale_simple(
                width,
                height,
                GdkPixbuf.InterpType.BILINEAR
            )
        Gdk.cairo_set_source_pixbuf(
            ctx,
            image,
            x,
            y
        )
        ctx.paint()
    
    def get_scaled_image(self, target_width, target_height):

        if (
            self.cached_image is not None
            and self.cached_width == target_width
            and self.cached_height == target_height 
        ):
            return self.cached_image
        
        image = self.original_image.scale_simple(
            target_width,
            target_height,
            GdkPixbuf.InterpType.BILINEAR
        )

        #save to cache
        self.cached_image = image
        self.cached_width = target_width
        self.cached_height = target_height
        return image
    
    def measure(self, ctx, screen_width, screen_height):
        if self.width == "100%" and self.height == "100%":
            return screen_width, screen_height
        
        width = resolve_unit(self.width, screen_width)
        height = resolve_unit(self.height, screen_height)

        original_width = self.original_image.get_width()
        original_height = self.original_image.get_height()

        if width is None and height is None:
            return original_width, original_height

        if width is not None and height is None:
            scale = width / original_width
            return width, int(original_height * scale)

        if height is not None and width is None:
            scale = height / original_height
            return int(original_width * scale), height

        scale = min(
            width / original_width,
            height / original_height
        )

        return (
            max(1, int(original_width * scale)),
            max(1, int(original_height * scale))
        )
