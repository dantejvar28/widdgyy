
class Sticker:

    def __init__(
            self,
            x=0,
            y=0,
            anchor="top-left",
            offset_x=0,
            offset_y=0,
            width=0,
            height=0,
            visible=True,
            z_index=0,
            css_class=""):
        
        self.x = x
        self.y = y

        self.anchor = anchor

        self.offset_x = offset_x
        self.offset_y = offset_y

        self.width = width
        self.height = height

        self.visible = visible
        self.z_index = z_index
        self.css_class = css_class
        self.style = {}

    def update(self,delta):
        pass
    def render(self, ctx):
        pass 

    def get_position(
            self,
            screen_width,
            screen_height,
            object_width=0,
            object_height=0
    ):
        x = self.x
        y = self.y

        if self.anchor =="top-left":
            x = self.offset_x
            y = self.offset_y
        elif self.anchor == "top-right":
            x = screen_width - object_width + self.offset_x
            y = self.offset_y
        elif self.anchor == "bottom-left":
            x = self.offset_x
            y = screen_height - object_height + self.offset_y 
        elif self.anchor == "bottom-right":
            x = screen_width - object_width + self.offset_x
            y = screen_height - object_height + self.offset_y
        elif self.anchor == "center":
            x = (screen_width - object_width) / 2 + self.offset_x
            y = (screen_height - object_height) / 2 + self.offset_y
        return x, y