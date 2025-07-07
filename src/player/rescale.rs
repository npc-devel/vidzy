use std::os::raw::{c_ulong, c_void};

#[derive(derive_more::Deref, derive_more::DerefMut)]
pub struct Rescaler(ffmpeg_next::software::scaling::Context);
unsafe impl std::marker::Send for Rescaler {}

pub fn rgba_rescaler_for_frame(qs:f64, frame: &ffmpeg_next::util::frame::Video) -> Rescaler {
    Rescaler(
        ffmpeg_next::software::scaling::Context::get(
            frame.format(),
            frame.width(),
            frame.height(),
            Pixel::RGB24,
            (1280.0*qs)as u32,
            (720.0*qs) as u32,
            ffmpeg_next::software::scaling::Flags::FAST_BILINEAR,
        ).unwrap(),
    )
}

pub fn video_frame_to_pixel_buffer(
    frame: &ffmpeg_next::util::frame::Video,
) -> slint::SharedPixelBuffer<slint::Rgb8Pixel> {
    let mut pixel_buffer =
        slint::SharedPixelBuffer::<slint::Rgb8Pixel>::new(frame.width(), frame.height());
    let s = pixel_buffer.make_mut_slice().as_mut_ptr() as *mut c_void;
    let d = frame.data(0).as_ptr() as *mut c_void;
    let l = (frame.width()*frame.height()) as usize * core::mem::size_of::<slint::Rgb8Pixel>() ;
    unsafe {
        memcpy(s, d, l as c_ulong);
    }

    /*
    let mut pixel_buffer =
        slint::SharedPixelBuffer::<slint::Rgb8Pixel>::new(frame.width(), frame.height());

    let ffmpeg_line_iter = frame.data(0).chunks_exact(frame.stride(0));
    let slint_pixel_line_iter = pixel_buffer
        .make_mut_bytes()
        .chunks_mut(frame.width() as usize * core::mem::size_of::<slint::Rgb8Pixel>());

    for (source_line, dest_line) in ffmpeg_line_iter.zip(slint_pixel_line_iter) {
        dest_line.copy_from_slice(&source_line[..dest_line.len()])
    }*/

    pixel_buffer
}

pub fn consts_to_pixel_buffer(
    output:i32,
) -> slint::SharedPixelBuffer<slint::Rgb8Pixel> {
    let mut pixel_buffer =
        slint::SharedPixelBuffer::<slint::Rgb8Pixel>::new(1,1);

    //let ffmpeg_line_iter = frame.data(0).chunks_exact(frame.stride(0));
    let slint_pixel_line_iter = pixel_buffer
        .make_mut_bytes()
        .chunks_mut( core::mem::size_of::<slint::Rgb8Pixel>());

    for l in slint_pixel_line_iter {
        l.copy_from_slice(&[output as u8,output as u8,output as u8])
    }

    pixel_buffer
}
