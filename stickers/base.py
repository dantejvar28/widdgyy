# SPDX-FileCopyrightText: 2026 Daniel
# SPDX-License-Identifier: GPL-3.0-or-later

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
        self.dirty = True

    def mark_dirty(self):
        self.dirty = True

    def consume_dirty(self):
        if not self.dirty:
            return False
        self.dirty = False
        return True

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
        return False
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
        shadow = (
            self.style.get("box_shadow")
            or self.style.get("box-shadow")
        )
        background = (
            self.style.get("background_color")
            or self.style.get("background-color")
            or self.style.get("background")
        )
        radius = self.parse_radius(
            self.style.get("border_radius")
            or self.style.get("border-radius")
            or 0
        )
        if shadow:
            shadow_data = self.parse_box_shadow(shadow)
            if shadow_data:
                offset_x, offset_y, blur, color = shadow_data
                r, g, b, a = self.parse_color(color)

                ctx.save()
                ctx.new_path()

                # Cairo has no direct box-blur primitive; approximate by layering
                # expanded rounded-rect fills with reduced alpha.
                passes = max(1, min(8, int(blur / 2) + 1))
                for i in range(passes):
                    spread = 0.0 if blur <= 0 else (blur * i / max(1, passes - 1))
                    alpha = a / passes
                    ctx.set_source_rgba(r, g, b, alpha)
                    self.rounded_rect(
                        ctx,
                        x + offset_x - spread,
                        y + offset_y - spread,
                        w + spread * 2,
                        h + spread * 2,
                        radius + spread,
                    )
                    ctx.fill()
                ctx.restore()

        if not background:
            return
        
        r, g, b, a = self.parse_color(background)
        ctx.save()
        ctx.set_source_rgba(r,g,b,a)
        # Ensure each sticker box uses an isolated path and does not inherit
        # leftover geometry from previous draw operations.
        ctx.new_path()
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

    def parse_length(self, value, allow_negative=False):
        if isinstance(value, (int, float)):
            number = float(value)
            return number if allow_negative else max(0.0, number)

        text = str(value).strip().lower()
        if text.endswith("px"):
            text = text[:-2].strip()

        try:
            number = float(text)
            return number if allow_negative else max(0.0, number)
        except ValueError:
            return 0.0
    
    def parse_color(self, value, fallback=None):
        if value is None:
            if fallback is None:
                return (0, 0, 0, 0)
            return self.parse_color(fallback)

        if isinstance(value, (tuple, list)):
            if len(value) == 4:
                r, g, b, a = value
                return (float(r), float(g), float(b), float(a))
            if len(value) == 3:
                r, g, b = value
                return (float(r), float(g), float(b), 1.0)
            if fallback is not None:
                return self.parse_color(fallback)
            return (0, 0, 0, 0)

        value = str(value).strip()
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
        if fallback is not None:
            return self.parse_color(fallback)
        return (0, 0, 0, 0)
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

    def parse_box_shadow(self,value):
        parts = value.split()
        if len(parts) < 3:
            return None

        offset_x = self.parse_length(parts[0], allow_negative=True)
        offset_y = self.parse_length(parts[1], allow_negative=True)
        blur = self.parse_length(parts[2])

        color = " ".join(parts[3:]).strip() or "rgba(0,0,0,0.35)"

        return (
            offset_x,
            offset_y,
            blur,
            color
        )
    
    def get_opacity(self):
        try:
            return float(
                self.style.get(
                    "opacity",
                    1.0
                )
            )
        except:
            return 1.0