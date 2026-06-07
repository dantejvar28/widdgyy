import cairo
import math

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
            margin=0,
            margin_top=None,
            margin_right=None,
            margin_bottom=None,
            margin_left=None,
            visible=True,
            z_index=0,
            update_interval=0,
            css_class=""):
        
        self.x = x
        self.y = y

        self.anchor = anchor

        self.offset_x = offset_x
        self.offset_y = offset_y

        self.width = width
        self.height = height

        self.margin_top = margin if margin_top is None else margin_top
        self.margin_right = margin if margin_right is None else margin_right
        self.margin_bottom = margin if margin_bottom is None else margin_bottom
        self.margin_left =  margin if margin_left is None else margin_left

        self.visible = visible
        self.z_index = z_index
        self.css_class = css_class
        self.style = {}
        self.update_interval = update_interval
        self.elapsed_time = 0

    def should_update(self, delta):
        if self.update_interval <=0:
            return True
        self.elapsed_time += delta
        if self.elapsed_time >= self.update_interval:
            self.elapsed_time -= self.update_interval
            return True
        return False 

    def measure(self,ctx,screen_width,screen_height):
        return 0,0

    # Backward-compatible alias for legacy call sites.
    def mesure(self,ctx,screen_width,screen_height):
        return self.measure(ctx,screen_width,screen_height)

    def update(self,delta):
        pass
    def render(self, ctx,x,y,w,h):
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
        elif self.anchor == "top-center":
            x = (screen_width - object_width) / 2 + self.offset_x
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
    
    def draw_box(
            self,
            ctx,
            x,
            y,
            w,
            h
    ):
        background = (
            self.style.get("background_color")
            or self.style.get("background-color")
            or self.style.get("background")
        )
        if not background:
            return
        
        r, g, b, a = self.parse_color(background)
        ctx.save()
        ctx.set_source_rgba(r,g,b,a)
        # Ensure each sticker box uses an isolated path and does not inherit
        # leftover geometry from previous draw operations.
        ctx.new_path()
        radius = self.parse_radius(
            self.style.get("border_radius")
            or self.style.get("border-radius")
            or 0
        )
        self.rounded_rect(ctx,x,y,w,h,radius)
        ctx.fill()
        border_width = self.parse_radius(
            self.style.get("border_width")
            or self.style.get("border-width")
            or 0
        )
        if border_width > 0:
            border_color = (
                self.style.get("border_color")
                or self.style.get("border-color")
                or "rgba(255,255,255,1)"
            )
            r, g, b, a = self.parse_color(border_color)
            ctx.set_source_rgba(r,g,b,a)
            ctx.set_line_width(border_width)
            self.rounded_rect(ctx,x,y,w,h,radius)
            ctx.stroke()  
        ctx.restore()

    def parse_radius(self, value):
        if isinstance(value, (int, float)):
            return max(0.0, float(value))

        text = str(value).strip().lower()
        if text.endswith("px"):
            text = text[:-2].strip()

        try:
            return max(0.0, float(text))
        except ValueError:
            return 0.0
    
    def parse_color(self,value):
        value = value.strip()
        # Accept CSS-like quoted color values, e.g. "#88F54C".
        if len(value) >= 2 and value[0] == value[-1] and value[0] in ('"', "'"):
            value = value[1:-1].strip()
        if value.startswith("rgba("):
            raw = value[5:-1]

            r,g,b,a= [
                p.strip() for p in raw.split(",")
            ]
            return (
                int(r)/255,
                int(g)/255,
                int(b)/255,
                float(a)
            )
        if value.startswith("rgb("):
            raw = value[4:-1]

            r,g,b= [
                p.strip() for p in raw.split(",")
            ]
            return (
                int(r)/255,
                int(g)/255,
                int(b)/255,
                1.0    
            )
        if value.startswith("#"):
            hex = value[1:]
            if len(hex) == 6:
                r = int(hex[0:2],16)/255
                g = int(hex[2:4],16)/255
                b = int(hex[4:6],16)/255
                return (r,g,b,1.0)
            if len(hex) == 8:
                r = int(hex[0:2],16)/255
                g = int(hex[2:4],16)/255
                b = int(hex[4:6],16)/255
                a = int(hex[6:8],16)/255
                return (r,g,b,a)
        return (0,0,0,0)
    def rounded_rect(self,ctx,x,y,w,h,radius):
        radius = min (radius, w/2, h/2)

        if radius <= 0:
            ctx.rectangle(x, y, w, h)
            return

        # Start a fresh sub-path to avoid accidental connector lines.
        ctx.new_sub_path()
        ctx.move_to(x + radius, y)
        ctx.line_to(x + w - radius, y)
        ctx.arc(x + w - radius, y + radius, radius, -math.pi / 2, 0)
        ctx.line_to(x + w, y + h - radius)
        ctx.arc(x + w - radius, y + h - radius, radius, 0, math.pi / 2)
        ctx.line_to(x + radius, y + h)
        ctx.arc(x + radius, y + h - radius, radius, math.pi / 2, math.pi)
        ctx.line_to(x, y + radius)
        ctx.arc(x + radius, y + radius, radius, math.pi, 3 * math.pi / 2)
        ctx.close_path()