import cairo 
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
        
        ctx.set_source_rgba(*color)
        ctx.move_to(x, y)
        PangoCairo.show_layout(ctx, layout)
    
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