mod backend;
mod video_widget;
mod state;

use pyo3::prelude::*;
use pyo3::types::PyModule;
use backend::VideoBackend;

#[pymodule]
fn widdgyy_video(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<VideoBackend>()?;
    Ok(())
}