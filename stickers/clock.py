# SPDX-FileCopyrightText: 2026 Daniel
# SPDX-License-Identifier: GPL-3.0-or-later

from datetime import datetime

from stickers.text import TextSticker

class ClockSticker(TextSticker):
    def __init__(
            self,
            format="%H:%M:%S",
            **kwargs
    ):
        super().__init__(
            text="",
            **kwargs
        )
        self.format = format
        self.update_clock()

    def update_clock(self):
        self.text = datetime.now().strftime(self.format)

    def update(self, delta):
        self.update_clock()
        self.mark_dirty()
        return True