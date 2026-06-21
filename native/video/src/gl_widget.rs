use gtk4::prelude::*;
use gtk4::GLArea;

pub struct VideoGLWidget {
    gl_area: GLArea,
}

impl VideoGLWidget {
    pub fn new()-> Self {
        let gl_area: GLArea = GLArea::new();

        gl_area.set_hexpand(true);
        gl_area.set_vexpand(true);

        Self {
            gl_area,
        }
    }

    pub fn widget(&self) -> &GLArea {
        &self.gl_area
    }
}