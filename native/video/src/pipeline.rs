use gstreamer as gst;
use gst::prelude::*;

pub fn create_pipeline(path: &str) -> gst::Pipeline {
    
    gst::init().unwrap();
    
    let uri = glib::filename_to_uri(
        std::fs::canonicalize(path).unwrap(),
        None,
    )
    .unwrap();

    let pipeline = gst::ElementFactory::make("playbin")
        .property("uri", uri)
        .build()
        .unwrap();

    pipeline
        .downcast::<gst::Pipeline>()
        .unwrap()
}