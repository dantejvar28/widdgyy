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
        ctx.set_source_rgba(*self.color)
        
        ctx.select_font_face(
            "Sans",
            cairo.FONT_SLANT_NORMAL,
            cairo.FONT_WEIGHT_NORMAL
        )

        ctx.set_font_size(self.font_size)
        ctx.move_to(self.x, self.y)
        ctx.show_text(self.text)
