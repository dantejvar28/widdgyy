# SPDX-FileCopyrightText: 2026 Daniel
# SPDX-License-Identifier: GPL-3.0-or-later

import cairo 
import math
import re
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

    def _resolve_font_size(self):
        if "font_size" in self.style:
            return float(self.style["font_size"])
        if "font-size" in self.style:
            return float(self.style["font-size"])
        return float(self.font_size)

    def _resolve_font_family(self):
        return (
            self.style.get("font_family")
            or self.style.get("font-family")
            or "Sans"
        )
    def _resolve_font_weight(self):
        return (
            self.style.get("font_weight") or 
            self.style.get("font-weight") or
            "normal"
        )
    
    def _resolve_font_style(self):
        return (
            self.style.get("font_style") or 
            self.style.get("font-style") or
            "normal"
        )

    def _build_layout(self, ctx):
        layout = PangoCairo.create_layout(ctx)
        layout.set_text(self.text, -1)

        font_desc = Pango.FontDescription()
        font_desc.set_family(self._resolve_font_family())
        font_desc.set_size(int(self._resolve_font_size() * Pango.SCALE))

        weight= self._resolve_font_weight()
        if weight == "bold":
            font_desc.set_weight(Pango.Weight.BOLD)
        elif weight == "light":
            font_desc.set_weight(Pango.Weight.LIGHT)
        elif weight == "ultralight":
            font_desc.set_weight(Pango.Weight.ULTRALIGHT)
        elif weight == "semibold":
            font_desc.set_weight(Pango.Weight.SEMIBOLD)
        elif weight == "heavy":
            font_desc.set_weight(Pango.Weight.HEAVY)
        else: 
            font_desc.set_weight(Pango.Weight.NORMAL)
        
        style = self._resolve_font_style()

        if style == "italic":
            font_desc.set_style(Pango.Style.ITALIC)
        elif style == "oblique":
            font_desc.set_style(Pango.Style.OBLIQUE)
        else:
            font_desc.set_style(Pango.Style.NORMAL)
        
        layout.set_font_description(font_desc)

        align=(
            self.style.get("text_align") or 
            self.style.get("text-align") or
            "left"
        ).lower()

        if align == "center":
            layout.set_alignment(Pango.Alignment.CENTER)
        elif align == "right":
            layout.set_alignment(Pango.Alignment.RIGHT)
        else:
            layout.set_alignment(Pango.Alignment.LEFT)
        
        width = (
            self.style.get("width")
            or self.style.get("max_width") 
            or self.style.get("max-width")
        )
        if width:
            width_value = self.parse_length(width)
            if width_value > 0:
                layout.set_width(
                    int(width_value * Pango.SCALE)
                )

                wrap = (
                    self.style.get("text_wrap")
                    or self.style.get("text-wrap")
                    or "wrap"
                ).lower()

                if wrap in ("nowrap", "no-wrap", "none"):
                    layout.set_wrap(Pango.WrapMode.WORD)
                    overflow = (
                        self.style.get("text_overflow")
                        or self.style.get("text-overflow")
                        or "clip"
                    ).lower()
                    if overflow == "ellipsis":
                        layout.set_ellipsize(Pango.EllipsizeMode.END)
                    else:
                        layout.set_ellipsize(Pango.EllipsizeMode.NONE)
                else:
                    layout.set_wrap(Pango.WrapMode.WORD_CHAR)
                    layout.set_ellipsize(Pango.EllipsizeMode.NONE)

        return layout

    def render(self, ctx, x, y, w, h):
        ctx.save()
        ctx.push_group()
        self.draw_box(ctx, x, y, w, h)

        ctx.save()
        ctx.new_path()
        ctx.rectangle(x, y, w, h)
        ctx.clip()
        
        color = self.color

        if "color" in self.style:
            color = self._parse_style_color(
                self.style["color"],
                color
            )

        layout = self._build_layout(ctx)
        
        shadow = self.style.get("text_shadow") or self.style.get("text-shadow")
        if shadow:
            shadow_data = (
                self.parse_text_shadow(shadow)
            )
            if shadow_data:
                offset_x, offset_y, blur, shadow_color = shadow_data
                r, g, b, a = self.parse_color(shadow_color)
                self.draw_text_shadow(
                    ctx,
                    layout,
                    x,
                    y,
                    offset_x,
                    offset_y,
                    blur,
                    (r, g, b, a)
                )
        
        ctx.set_source_rgba(*color)
        ctx.move_to(x, y)
        PangoCairo.show_layout(ctx, layout)

        ctx.restore()
        
        ctx.pop_group_to_source()
        ctx.paint_with_alpha(
            self.get_opacity()
        )
        ctx.restore()
    
    def measure(self, ctx, screen_width, screen_height):
        layout = self._build_layout(ctx)
        text_w, text_h = layout.get_pixel_size()
        style_width =(
            self.style.get("width")
            or self.style.get("max_width")
            or self.style.get("max-width")
        )
        if style_width:
            text_w = self.parse_length(style_width)

        return text_w, text_h

    def parse_text_shadow(
            self,
            value
    ):
        parts = value.split(None, 2)
        if len(parts) < 3:
            return None
        
        offset_x = self.parse_length(parts[0], allow_negative=True)
        offset_y = self.parse_length(parts[1], allow_negative=True)

        rest = parts[2].strip()
        blur = 0.0
        color = rest

        if rest:
            blur_match = re.match(
                r"^([+-]?(?:\d+\.\d+|\d+|\.\d+)(?:px)?)\s+(.*)$",
                rest
            )
            if blur_match:
                blur = self.parse_length(blur_match.group(1))
                color = blur_match.group(2).strip()
        return (
            offset_x,
            offset_y,
            blur,
            color
        )

    def draw_text_shadow(
            self,
            ctx,
            layout,
            x,
            y,
            offset_x,
            offset_y,
            blur,
            color
    ):
        r, g, b, a = color
        if blur <= 0:
            ctx.save()
            ctx.set_source_rgba(r, g, b, a)
            ctx.move_to(x + offset_x, y + offset_y)
            PangoCairo.show_layout(ctx, layout)
            ctx.restore()
            return

        # Cairo does not expose a real text blur filter here, so approximate it
        # by painting the shadow multiple times around the target position.
        rings = max(2, min(8, int(blur)))
        samples = max(8, min(24, int(blur) * 4))

        ctx.save()
        for ring in range(1, rings + 1):
            radius = (blur * ring) / rings
            ring_alpha = a / (rings * samples)
            for sample in range(samples):
                angle = (2 * math.pi * sample) / samples
                dx = math.cos(angle) * radius
                dy = math.sin(angle) * radius
                ctx.set_source_rgba(r, g, b, ring_alpha)
                ctx.move_to(
                    x + offset_x + dx,
                    y + offset_y + dy
                )
                PangoCairo.show_layout(ctx, layout)
        ctx.restore()
        