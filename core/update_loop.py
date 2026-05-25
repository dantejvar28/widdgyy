import time 

class UpdateLoop:
    def __init__(self,canvas,scene):
        self.canvas = canvas
        self.scene = scene
        
        self.last_time = time.time()

    def start(self):
        self.canvas.add_tick_callback(
            self.on_tick
        )

    def on_tick(self,widget,frame_clock):
        current_time = time.time()
        delta = current_time - self.last_time
        self.last_time = current_time

        self.scene.update(delta)

        self.canvas.queue_draw()

        return True
     
