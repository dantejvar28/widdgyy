import cairo 
import math
import re
from gi.repository import Pango
from gi.repository import PangoCairo
from stickers.base import Sticker

class TextSticker(Sticker):
    def __init__(
        self,
        text,
        font_size=32,
        color=(1,1,1,1),
        **kwargs
    ):
        super().__init__(**kwargs)
        self.text = text
        self.font_size = font_size
        self.color = color

    def _parse_style_color(self, raw_color, fallback):
        try:
            parts = [float(p.strip()) for p in raw_color.split(",")]
            if len(parts) == 3:
                parts.append(1.0)
            if len(parts) != 4:
                return fallback

            # Accept RGB either in 0-1 or 0-255. Keep alpha in 0-1, or map >1 as 0-255.
            r, g, b, a = parts
            if any(value > 1.0 for value in (r, g, b)):
                r, g, b = r / 255.0, g / 255.0, b / 255.0
            if a > 1.0:
                a = a / 255.0

            parts = [r, g, b, a]

            return tuple(max(0.0, min(1.0, value)) for value in parts)
        except Exception:
            return fallback

    def render(self, ctx, x, y, w, h):
        ctx.save()
        ctx.push_group()
        self.draw_box(ctx, x, y, w, h)
        
        font_size = self.font_size
        color = self.color

        if "font_size" in self.style:
            font_size = float(
                self.style["font_size"]
            )
        elif "font-size" in self.style:
            font_size = float(
                self.style["font-size"]
            )
        if "color" in self.style:
            color = self._parse_style_color(
                self.style["color"],
                color
            )

        layout = PangoCairo.create_layout(ctx)
        layout.set_text(self.text, -1)

        font_desc = Pango.FontDescription()
        font_desc.set_family("Sans")
        font_desc.set_size(int(font_size * Pango.SCALE))

        layout.set_font_description(font_desc)
        
        shadow = self.style.get("text_shadow") or self.style.get("text-shadow")
        if shadow:
            shadow_data = (
                self.parse_text_shadow(shadow)
            )
            if shadow_data:
                offset_x, offset_y, blur, shadow_color = shadow_data
                r, g, b, a = self.parse_color(shadow_color)
                self.draw_text_shadow(
                    ctx,
                    layout,
                    x,
                    y,
                    offset_x,
                    offset_y,
                    blur,
                    (r, g, b, a)
                )
        
        ctx.set_source_rgba(*color)
        ctx.move_to(x, y)
        PangoCairo.show_layout(ctx, layout)
        
        ctx.pop_group_to_source()
        ctx.paint_with_alpha(
            self.get_opacity()
        )
        ctx.restore()
    
    def measure(self, ctx, screen_width, screen_height):
        
        font_size = self.font_size
        if "font_size" in self.style:
            font_size = float(
                self.style["font_size"]
            )
        elif "font-size" in self.style:
            font_size = float(
                self.style["font-size"]
            )
        layout = PangoCairo.create_layout(ctx)
        layout.set_text(self.text, -1)
        font_desc = Pango.FontDescription()
        font_desc.set_family("Sans")
        font_desc.set_size(int(font_size * Pango.SCALE))
        layout.set_font_description(font_desc)
        width, height = layout.get_pixel_size()
        return width, height

    def parse_text_shadow(
            self,
            value
    ):
        parts = value.split(None, 2)
        if len(parts) < 3:
            return None
        
        offset_x = self.parse_length(parts[0], allow_negative=True)
        offset_y = self.parse_length(parts[1], allow_negative=True)

        rest = parts[2].strip()
        blur = 0.0
        color = rest

        if rest:
            blur_match = re.match(
                r"^([+-]?(?:\d+\.\d+|\d+|\.\d+)(?:px)?)\s+(.*)$",
                rest
            )
            if blur_match:
                blur = self.parse_length(blur_match.group(1))
                color = blur_match.group(2).strip()
        return (
            offset_x,
            offset_y,
            blur,
            color
        )

    def draw_text_shadow(
            self,
            ctx,
            layout,
            x,
            y,
            offset_x,
            offset_y,
            blur,
            color
    ):
        r, g, b, a = color
        if blur <= 0:
            ctx.save()
            ctx.set_source_rgba(r, g, b, a)
            ctx.move_to(x + offset_x, y + offset_y)
            PangoCairo.show_layout(ctx, layout)
            ctx.restore()
            return

        # Cairo does not expose a real text blur filter here, so approximate it
        # by painting the shadow multiple times around the target position.
        rings = max(2, min(8, int(blur)))
        samples = max(8, min(24, int(blur) * 4))

        ctx.save()
        for ring in range(1, rings + 1):
            radius = (blur * ring) / rings
            ring_alpha = a / (rings * samples)
            for sample in range(samples):
                angle = (2 * math.pi * sample) / samples
                dx = math.cos(angle) * radius
                dy = math.sin(angle) * radius
                ctx.set_source_rgba(r, g, b, ring_alpha)
                ctx.move_to(
                    x + offset_x + dx,
                    y + offset_y + dy
                )
                PangoCairo.show_layout(ctx, layout)
        ctx.restore()
        