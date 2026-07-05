class Scene:
    def __init__(self):
        self.stickers=[]

    def add(self, sicker):
        self.stickers.append(sicker)
        if hasattr(sicker, "mark_dirty"):
            sicker.mark_dirty()
        self.sort_z()
    def remove(self,sticker):
        self.stickers.remove(sticker)

    def clear(self):
        self.stickers.clear()

    def sort_z(self):
        self.stickers.sort(
            key=lambda s: s.z_index
        )
    def update(self,delta):
        dirty = False
        for sticker in self.stickers:
            if not sticker.visible:
                continue
            if sticker.should_update(delta):
                result = sticker.update(delta)
                dirty = bool(result) or dirty

            if getattr(sticker, "consume_dirty", None) and sticker.consume_dirty():
                dirty = True

        return dirty

    
