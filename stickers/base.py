
class Sticker:

    def __init__(
            self,
            x=0,
            y=0,
            width=0,
            height=0,
            visible=True,
            z_index=0,
            css_class=""):
        self.x = x
        self.y = y

        self.width = width
        self.height = height

        self.visible = visible
        self.z_index = z_index
        self.css_class = css_class

    def update(self,delta):
        pass
    def render(self, ctx):
        pass 