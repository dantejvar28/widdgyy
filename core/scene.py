class Scene:
    def __init__(self):
        self.stickers=[]

    def add(self, sicker):
        self.stickers.append(sicker)
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
        for sticker in self.stickers:
            if not sticker.visible:
                continue
            if sticker.should_update(delta):
                sticker.update(delta)

    
