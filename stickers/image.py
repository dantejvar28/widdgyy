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

    def render(self,ctx,screen_width,screen_height):

        target_width = resolve_unit(
            self.width,
            screen_width
        )
        target_height = resolve_unit(  
            self.height,
            screen_height
        )

        image = self.original_image
        original_width = self.original_image.get_width()
        original_height = self.original_image.get_height()

        if target_width is None and target_height is None:
            width = original_width
            height = original_height
        elif target_width is not None and target_height is not None:
            scale = min(
                target_width / original_width,
                target_height / original_height
            )
            width = max(1, int(original_width * scale))
            height = max(1, int(original_height * scale))
        elif target_width is not None:
            width = target_width
            height = max(1, int(original_height * (target_width / original_width)))
        else:
            height = target_height
            width = max(1, int(original_width * (target_height / original_height)))

        if width != original_width or height != original_height:
            image = self.get_scaled_image(
                width,
                height
            )

        real_x, real_y = self.get_position(
            screen_width,
            screen_height,
            image.get_width(),
            image.get_height()
        )
        Gdk.cairo_set_source_pixbuf(
            ctx,
            image,
            real_x,
            real_y

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
    
    
