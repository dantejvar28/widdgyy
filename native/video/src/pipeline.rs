use gstreamer as gst;
use gst::prelude::*;

pub fn create_pipeline(path: &str) -> gst::Pipeline {
    
    gst::init().unwrap();
    
    let uri = glib::filename_to_uri(
        std::fs::canonicalize(path).unwrap(),
        None,
    )
    .unwrap();

    gst::parse::launch(
        &format!(
            "playbin uri={}",
            uri
        )
    )
    .unwrap()
    .downcast::<gst::Pipeline>()
    .unwrap()
}