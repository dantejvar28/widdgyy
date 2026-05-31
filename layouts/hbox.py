from stickers.base import Sticker

class Hbox(Sticker):
    def __init__(
            self,
            children=None,
            spacing=0,
            padding=0,
            padding_top=None,
            padding_right=None,
            padding_bottom=None,
            padding_left=None,
            justify="start",
            align_items="start",
            **kwargs
            ):
        super().__init__(**kwargs)

        self.children=children or []

        self.spacing=spacing
        self.padding_top = padding if padding_top is None else padding_top
        self.padding_right = padding if padding_right is None else padding_right
        self.padding_bottom = padding if padding_bottom is None else padding_bottom
        self.padding_left = padding if padding_left is None else padding_left
        self.justify = justify
        self.align_items = align_items 

    def measure(
            self,
            ctx,
            screen_width,
            screen_height
    ):
        total_width = self.padding_left + self.padding_right
        max_height=0
        for child in self.children:
            w,h =child.measure(
                ctx,
                screen_width,
                screen_height
            )
            total_width += (
                child.margin_left + w + child.margin_right
            )
            max_height = max(max_height, h)
        if self.children:
            total_width += (
                len(self.children)-1
            ) * self.spacing
        return total_width, (
            max_height + self.padding_top + self.padding_bottom
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
            children_width += (
                child.margin_left + child_w + child.margin_right
            )
        if self.children:
            children_width += (
                len(self.children)-1
            ) * self.spacing

        content_w = w - self.padding_left - self.padding_right
        content_h = h - self.padding_top - self.padding_bottom

        if self.justify == "start":
            current_x = x + self.padding_left
        elif self.justify == "center":
            current_x = (
                x + self.padding_left + (content_w - children_width) / 2
            )
        elif self.justify == "end":
            current_x = (
                x + w - self.padding_right - children_width
            )
        else: 
            current_x = x + self.padding_left

        for child in self.children:
            child_w, child_h = child.measure(
                ctx,
                w,
                h
            )
            if self.align_items == "start":
                child_y = y + self.padding_top
            elif self.align_items == "center":
                child_y = (
                    y + self.padding_top + (content_h - child_h)/2
                )
            elif self.align_items == "end":
                child_y = (
                    y + h - self.padding_bottom - child_h
                )
            else:
                child_y = y + self.padding_top
            
            current_x += child.margin_left
            child.render(
                ctx,
                current_x,
                child_y,
                child_w,
                child_h
            )
            current_x += (
                child_w + child.margin_right + self.spacing
            )

    def update(self, delta):
        for child in self.children:
            if child.should_update(delta):
                child.update(delta)