from gi.repository import Gdk
from gi.repository import GdkPixbuf

from stickers.base import Sticker
from utils.units import resolve_unit
"""
Deprecated: This class is no longer used. The functionality has been moved to MediaSticker in media.py
"""
class ImageSticker(Sticker):
    
    def __init__(
            self,
            path,
            
            width=None,
            height=None,
            fit="contain",
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
        self.fit = fit

    def render(self,ctx,x,y,w,h):

        image = self.get_scaled_image(w, h)

        Gdk.cairo_set_source_pixbuf(
            ctx,
            image,
            x,
            y
        )

        ctx.paint()
    
    def get_scaled_image(self, target_width, target_height):

        target_width = max(1, int(target_width))
        target_height = max(1, int(target_height))
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

        target_width = resolve_unit(
            self.width,
            screen_width
        )

        target_height = resolve_unit(
            self.height,
            screen_height
        )

        original_width = self.original_image.get_width()
        original_height = self.original_image.get_height()

        if target_width is None and target_height is None:
            return original_width, original_height

        if target_width is not None and target_height is None:

            scale = target_width / original_width

            return (
                target_width,
                int(original_height * scale)
            )
        
        if target_height is not None and target_width is None:

            scale = target_height / original_height

            return (
                int(original_width * scale),
                target_height
            )

        if self.fit == "stretch":
            return target_width, target_height

        if self.fit == "contain":

            scale = min(
                target_width / original_width,
                target_height / original_height
            )

        elif self.fit == "cover":

            scale = max(
                target_width / original_width,
                target_height / original_height
            )

        else:
            scale = 1

        return (
            max(1, int(original_width * scale)),
            max(1, int(original_height * scale))
        )
