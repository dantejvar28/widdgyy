from stickers.base import Sticker

class VBox(Sticker):
    def __init__(
            self,
            children=None,
            spacing=0,
            padding=0,
            justify="start",
            **kwargs
    ):
        super().__init__(**kwargs)
        self.children = children or []

        self.spacing = spacing
        self.padding = padding
        
        self.justify = justify 

    def measure(
            self,
            ctx,
            screen_width,
            screen_height
    ):
        total_height = self.padding * 2
        max_width = 0
        for child in self.children:
            child_w, child_h = child.measure(
                ctx,
                screen_width,
                screen_height
            )

            total_height += child_h

            max_width = max(
                max_width,
                child_w
            )
        
        if self.children: 
            total_height += (
                len(self.children)-1
            ) * self.spacing 
        
        return (
            max_width + self.padding * 2,
            total_height
        )
    def render(
            self,
            ctx,
            x,
            y,
            w,
            h
    ):
        children_height = 0
        for child in self.children:
            child_w, child_h = child.measure(
                ctx,
                w,
                h
            )

            children_height += child_h
        if self.children:
            children_height += (
                len(self.children)-1
            )*self.spacing
        
        if self.justify == "start":
            current_y = y + self.padding
        elif self.justify == "center":
            current_y = (
                y + (h - children_height)/2
            )
        elif self.justify == "end":
            current_y = (
                y + h - children_height - self.padding
            )
        else: 
            current_y = y + self.padding

        for child in self.children: 
            child_w, child_h = child.measure(
                ctx,
                w,
                h
            )

            child.render(
                ctx,
                x+self.padding,
                current_y, 
                child_w,
                child_h
            )
            current_y += (
                child_h + self.spacing
            )  