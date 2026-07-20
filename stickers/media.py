# SPDX-FileCopyrightText: 2026 Daniel
# SPDX-License-Identifier: GPL-3.0-or-later

from stickers.base import Sticker
from pathlib import Path

from media.image_backend import ImageBackend
from media.video_backend import VideoBackend
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
        self.backend=VideoBackend(
            self.path,
            autoplay=self.autoplay,
            loop=self.loop
        )

    def update(self, delta):
        if self.backend:
            self.backend.update(delta)

    def uses_native_widget(self):
        return bool(self.backend and hasattr(self.backend, "attach_overlay"))

    def native_widget(self):
        if self.uses_native_widget() and hasattr(self.backend, "widget"):
            return self.backend.widget()
        return None

    def native_background(self):
        return self.z_index < 0

    def attach_native_widget(self, overlay):
        if self.uses_native_widget():
            self.backend.attach_overlay(overlay)

    def detach_native_widget(self):
        if self.uses_native_widget() and hasattr(self.backend, "detach_overlay"):
            self.backend.detach_overlay()

    def hide_native_widget(self):
        if self.uses_native_widget() and hasattr(self.backend, "hide"):
            self.backend.hide()

    def show_native_widget(self):
        if self.uses_native_widget() and hasattr(self.backend, "show"):
            self.backend.show()

    def render(self, ctx, x, y, w, h):
        if not self.backend:
            return
        draw_w, draw_h = self.measure(
            ctx,
            w,
            h
        )
        draw_x = x + (w - draw_w) / 2
        draw_y = y + (h - draw_h) / 2

        self.backend.render(ctx, draw_x, draw_y, draw_w, draw_h)

    def measure(self, ctx, screen_width, screen_height):

        if not self.backend:
            return 0, 0

        original_w, original_h = self.backend.measure(
            ctx,
            screen_width,
            screen_height
        )

        # Some backends (e.g. video) can report 0x0 before first frame decode.
        # Avoid division-by-zero and return a safe placeholder size.
        if original_w <= 0 or original_h <= 0:
            target_width = resolve_unit(self.width, screen_width)
            target_height = resolve_unit(self.height, screen_height)

            if target_width is not None and target_height is not None:
                return target_width, target_height
            if target_width is not None:
                return target_width, target_width
            if target_height is not None:
                return target_height, target_height
            return 0, 0

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
        return self.measure(ctx, screen_width, screen_height)
