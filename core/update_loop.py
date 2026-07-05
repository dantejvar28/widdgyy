import time 

class UpdateLoop:
    def __init__(self,canvas,scene):
        self.canvas = canvas
        self.scene = scene
        
        self.last_time = time.time()
        self.last_width = None
        self.last_height = None

    def start(self):
        self.canvas.queue_draw()
        self.canvas.add_tick_callback(
            self.on_tick
        )

    def on_tick(self,widget,frame_clock):
        current_time = time.time()
        delta = current_time - self.last_time
        self.last_time = current_time

        dirty = self.scene.update(delta)

        current_width = self.canvas.get_width()
        current_height = self.canvas.get_height()

        if self.last_width != current_width or self.last_height != current_height:
            self.last_width = current_width
            self.last_height = current_height
            dirty = True

        if dirty:
            self.canvas.queue_draw()

        return True
     
