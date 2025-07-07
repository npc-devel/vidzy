#[link(name = "c")]
unsafe extern "C" {
    fn memcpy(s: *mut c_void,d: *mut c_void,l: c_ulong);
}

#[macro_export]
macro_rules! vframe_cb {
    ($qs:expr,$aw:expr) => {
        {
            let mut to_rgba_rescaler: Option<player::Rescaler> = None;
            //let app = ($aw);
            let app_weak = ($aw);
            
            move |output,new_frame| {
                let rebuild_rescaler =
                    to_rgba_rescaler.as_ref().map_or(true, |existing_rescaler| {
                        existing_rescaler.input().format != new_frame.format()
                    });

                if rebuild_rescaler {
                    to_rgba_rescaler = Some(player::rgba_rescaler_for_frame($qs,new_frame));
                }

                let rescaler = to_rgba_rescaler.as_mut().unwrap();

                let mut rgb_frame = ffmpeg_next::util::frame::Video::empty();
                rescaler.run(&new_frame, &mut rgb_frame).unwrap();

                let pixel_buffer = player::video_frame_to_pixel_buffer(&rgb_frame);
                let const_buffer = consts_to_pixel_buffer(output);
                
                //   let op = SharedVector::from([0]);;
              //  println!("Dumping: {output}");
                
                app_weak
                    .upgrade_in_event_loop(|app| {
                        let i = slint::Image::from_rgb8(pixel_buffer);
                        let idx  = slint::Image::from_rgb8(const_buffer);
                        match idx.to_rgb8().as_slice()[0].as_bytes()[0] {
                            1 => app.set_video_frame1(i),
                            2 => app.set_video_frame2(i),
                            3 => app.set_video_frame3(i),
                            4 => app.set_video_frame4(i),
                            5 => app.set_video_frame5(i),
                            _ => app.set_video_frame0(i)
                        }
                    })
                    .unwrap();
            }
        }
    };
}


