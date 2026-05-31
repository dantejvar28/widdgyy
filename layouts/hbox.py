from stickers.base import Sticker

class Hbox(Sticker):
    def __init__(
            self,
            children=None,
            spacing=0,
            padding=0,
            justify="start",
            align_items="start",
            **kwargs
            ):
        super().__init__(**kwargs)

        self.children=children or []

        self.spacing=spacing
        self.padding = padding
        self.justify = justify
        self.align_items = align_items 
    def measure(
            self,
            ctx,
            screen_width,
            screen_height
    ):
        total_width = self.padding * 2
        max_height=0
        for child in self.children:
            w,h =child.measure(
                ctx,
                screen_width,
                screen_height
            )
            total_width += w
            max_height = max(max_height, h)
        if self.children:
            total_width += (
                len(self.children)-1
            ) * self.spacing
        return total_width , (
            max_height + self.padding * 2 
        )
    def render(
            self,
            ctx,
            x,
            y,
            w,
            h
    ):
        children_width = 0
        for child in self.children:
            child_w, child_h = child.measure(
                ctx,
                w,
                h
            )
            children_width += child_w
        if self.children:
            children_width += (
                len(self.children)-1
            ) * self.spacing
        if self.justify == "start":
            current_x = x + self.padding
        elif self.justify == "center":
            current_x = (
                x+ (w - children_width) / 2
            )
        elif self.justify == "end":
            current_x = (
                x + w - children_width - self.padding
            )
        else: 
            current_x = x + self.padding
        for child in self.children:
            child_w, child_h = child.measure(
                ctx,
                w,
                h
            )
            if self.align_items == "start":
                child_y = y + self.padding
            elif self.align_items == "center":
                child_y = (
                    (y + (h - child_h)/2)
                )
            elif self.align_items == "end":
                child_y = (
                    y + h - child_h - self.padding
                )
            else:
                child_y = y + self.padding

            child.render(
                ctx,
                current_x,
                child_y,
                child_w,
                child_h
            )
            current_x += (
                child_w + self.spacing
            )

    def update(self, delta):
        for child in self.children:
            if child.should_update(delta):
                child.update(delta)