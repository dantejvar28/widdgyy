from stickers.base import Sticker
from pathlib import Path

from media.image_backend import ImageBackend
from utils.units import resolve_unit

class MediaSticker(Sticker):
    def __init__(
            self,
            path,
            width=None,
            height=None,
            fit="contain",
            autoplay=True,
            loop=True,
            **kwargs
    ):
        super().__init__(
            width=width,
            height=height,
            **kwargs
            )

        self.path=path
        self.fit = fit
        self.autoplay=autoplay
        self.loop=loop

        self.backend=None
        self.load_media()
    
    def load_media(self):
        extension=Path(self.path).suffix.lower()

        if extension in [".png",".jpg", ".jpeg", ".webp"]:
            self.load_image()
        elif extension in [".gif"]:
            self.load_gif()
        elif extension in [".mp4",".mkv",".webm",".avi"]:
            self.load_video()
        else:
            raise ValueError(f"Unsupported media type: {extension}")
    
    def load_image(self):
        self.backend=ImageBackend(
            self.path
        )

    def load_gif(self):
        pass
    def load_video(self):
        pass

    def update(self, delta):
        if self.backend:
            self.backend.update(delta)

    def render(self, ctx, x, y, w, h):
        if not self.backend:
            return
        self.backend.render(ctx, x, y, w, h)

    def measure(self, ctx, screen_width, screen_height):

        if not self.backend:
            return 0, 0

        original_w, original_h = self.backend.measure(
            ctx,
            screen_width,
            screen_height
        )

        target_width = resolve_unit(self.width, screen_width)
        target_height = resolve_unit(self.height, screen_height)

        if target_width is None and target_height is None:
            return original_w, original_h

        if target_width is not None and target_height is None:
            scale = target_width / original_w
            return target_width, max(1, int(original_h * scale))

        if target_height is not None and target_width is None:
            scale = target_height / original_h
            return max(1, int(original_w * scale)), target_height

        if self.fit == "stretch":
            return target_width, target_height

        if self.fit == "cover":
            scale = max(target_width / original_w, target_height / original_h)
        else:
            scale = min(target_width / original_w, target_height / original_h)

        return (
            max(1, int(original_w * scale)),
            max(1, int(original_h * scale))
        )

    def get_size(
        self,
        ctx,
        screen_width,
        screen_height
    ):
        if not self.backend:
            return 0, 0

        original_w, original_h = self.backend.measure(
            ctx,
            screen_width,
            screen_height
        )

        width = resolve_unit(self.width, screen_width)
        height = resolve_unit(self.height, screen_height)

        width = original_w if width is None else width
        height = original_h if height is None else height

        return width, height
