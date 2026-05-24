import cairo 

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
    def render(self, ctx):
        font_size = self.font_size
        color = self.color

        if "font_size" in self.style:
            font_size = float(
                self.style["font_size"]
            )
        if "color" in self.style:
            color = tuple(
                map(
                    float,
                    self.style["color"].split(",")
                )
            )
        ctx.set_source_rgba(*color)
        
        ctx.select_font_face(
            "Sans",
            cairo.FONT_SLANT_NORMAL,
            cairo.FONT_WEIGHT_NORMAL
        )

        ctx.set_font_size(font_size)
        ctx.move_to(self.x, self.y)
        ctx.show_text(self.text)
