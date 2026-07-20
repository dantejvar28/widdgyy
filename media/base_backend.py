# SPDX-FileCopyrightText: 2026 Daniel
# SPDX-License-Identifier: GPL-3.0-or-later

class MediaBackend:
    def update(self, delta):
        pass

    def measure(self, ctx, sw, sh):
        raise NotImplementedError

    def render(self, ctx, x, y, w, h):
        raise NotImplementedError

    def destroy(self):
        pass