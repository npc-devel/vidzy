use std::arch::is_aarch64_feature_detected;
use std::cmp::{Ordering, PartialEq, PartialOrd};
use std::mem::transmute;
use std::str::FromStr;
use libmpv::render::*;
use libmpv::*;

use glow::HasContext;
use libmpv::events::{Event, PropertyData};
use slint::platform::Key::Control;
use smol::channel::Receiver;
use smol::stream::StreamExt;

fn get_proc_address(ctx: &*mut c_void, proc: &str) -> *mut c_void {
    unsafe { (*((*ctx) as *const &dyn Fn(&str) -> *mut c_void))(proc) as *mut c_void }
}

#[derive(Clone,Copy)]
pub struct MpvStatics {
    pub(crate) lib: usize,
    pub(crate) show_idx: usize,
    pub(crate) file_idx: usize
}

impl MpvStatics {
    pub fn fetch<'a>(tag: usize) -> &'a mut MpvStatics {
        unsafe {
            &mut PLAYER_STATICS[tag]
        }
    }
}

pub struct MpvPlayer {
    pub(crate) control_sender: smol::channel::Sender<PlayerCommand>,
    pub(crate) event_receiver: smol::channel::Receiver<PlayerCommand>,
    runner_thread: Option<std::thread::JoinHandle<()>>,
    render_ctx: RenderContext,
    channel: usize
}

pub struct MpvShared {
    pub(crate) fbo: glow::Framebuffer,
    pub(crate) texture: glow::Texture,
    proc_addr: *mut c_void,
    pub(crate) gl: glow::Context,
    depth_texture: glow::Texture,
    pub(crate) program: glow::Program,
    prev_width: f32,
    prev_height: f32,
    vao: glow::VertexArray,
    vbo: glow::NativeBuffer
}

impl MpvShared {
    pub fn new(
        gl: Context,
        proc_addr: *mut c_void,
        (width, height): (f32, f32)
    ) -> Self {
        unsafe {
            let program = gl.create_program().expect("Cannot create program");

            let shader_sources = [
                (glow::VERTEX_SHADER, include_str!("../shaders/vertex.glsl")),
                (glow::FRAGMENT_SHADER, include_str!("../shaders/fragment.glsl")),
            ];

            let mut shaders = Vec::with_capacity(shader_sources.len());

            for (shader_type, shader_source) in shader_sources.iter() {
                let shader = gl
                    .create_shader(*shader_type)
                    .expect("Cannot create shader");
                gl.shader_source(shader, shader_source);
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    panic!("{}", gl.get_shader_info_log(shader));
                }
                gl.attach_shader(program, shader);
                shaders.push(shader);
            }

            gl.link_program(program);


            if !gl.get_program_link_status(program) {
                panic!("{}", gl.get_program_info_log(program));
            }

            for shader in shaders {
                gl.detach_shader(program, shader);
                gl.delete_shader(shader);
            }


                let mut quad_verts: [f32; 24] = [
                    // positions   // texCoords
                    -1.0, 1.0, 0.0, 1.0,
                    -1.0, -1.0, 0.0, 0.0,
                    1.0, -1.0, 1.0, 0.0,
                    -1.0, 1.0, 0.0, 1.0,
                    1.0, -1.0, 1.0, 0.0,
                    1.0, 1.0, 1.0, 1.0
                ];

                let mut sf = 1.0;
                let mut o = 0.0;



                for i in 0..24 {
                    if i % 4 < 3 {
                        quad_verts[i] *= sf;
                        quad_verts[i] += o;
                    }
                }

                let vbo = gl.create_buffer().expect("Cannot create buffer");
                gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));

                gl.buffer_data_u8_slice(
                    glow::ARRAY_BUFFER,
                    quad_verts.align_to().1,
                    glow::STATIC_DRAW,
                );

                let vao = gl
                    .create_vertex_array()
                    .expect("Cannot create vertex array");
                gl.bind_vertex_array(Some(vao));
                gl.enable_vertex_attrib_array(0);
                gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 16, 0);
                gl.enable_vertex_attrib_array(1);
                gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, 16, 8);

                gl.bind_buffer(glow::ARRAY_BUFFER, None);
                gl.bind_vertex_array(None);



            let fbo = gl.create_framebuffer().unwrap();
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));

            let texture = gl.create_texture().unwrap();
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.tex_image_2d(

                glow::TEXTURE_2D,
                0,
                glow::RGB as i32,
                width as _,
                height as _,
                0,
                glow::RGB,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::LINEAR as i32,
            );

            gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(texture),
                0,
            );

            let depth_texture = gl.create_texture().unwrap();

            gl.bind_texture(glow::TEXTURE_2D, Some(depth_texture));
            gl.tex_storage_2d(glow::TEXTURE_2D, 1, glow::DEPTH24_STENCIL8, 5000, 5000);
            gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::DEPTH_STENCIL_ATTACHMENT,
                glow::TEXTURE_2D,
                Some(depth_texture),
                0,
            );

            assert_eq!(
                gl.check_framebuffer_status(glow::FRAMEBUFFER),
                glow::FRAMEBUFFER_COMPLETE
            );

            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            gl.bind_texture(glow::TEXTURE_2D, None);
            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);

            Self {
                fbo,
                texture,
                gl,
                proc_addr,
                depth_texture,
                program,
                vao,
                vbo,
                prev_width: 0.0,
                prev_height: 0.0,
            }
        }
    }

    pub fn render(&mut self, pl: &MpvPlayer, (width, height): (f32, f32)) {
        if pl.channel >4 { return }
        unsafe {
            let gl = &self.gl;

            gl.use_program(Some(self.program));
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(self.fbo));
            if width != self.prev_width || height != self.prev_height {
                gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
                gl.tex_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    glow::RGB as i32,
                    width as _,
                    height as _,
                    0,
                    glow::RGB,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(None),
                );
                gl.bind_texture(glow::TEXTURE_2D, None);

                self.prev_width = width;
                self.prev_height = height;
            }


//            gl.clear_color(1.0, 0.1, 0.1, 1.0);



      //      gl.viewport(0 as i32, 0 as i32, cwidth as i32, height as i32);
            pl.render_ctx
                .render::<*mut c_void>(transmute(self.fbo), width as _, height as _, true)
                .expect("Failed to render");

            let mut vx = 0;
            let mut vy = 0;
            let mut vw = width as i32/2;
            let mut vh = height as i32/2;

            match pl.channel {
                0 => {
                    vw = width as i32;
                    vh = height as i32;
                }
                4 => {
                    vx += vw;
                }
                1 => {
                    vy += vh;
                }
                2 => {
                    vx += vw;
                    vy += vh;
                }
                _ => {}
            }
            gl.viewport(vx, vy, vw, vh);
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            //gl.clear_color(1.0, 0.0, 0.0, 1.0);
         //   gl.clear(glow::COLOR_BUFFER_BIT);

            gl.use_program(Some(self.program));
           // gl.viewport(0 as i32, 0 as i32, width as i32, height as i32);
            gl.bind_vertex_array(Some(self.vao));
            gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
            gl.draw_arrays(glow::TRIANGLES, 0, 6);

            gl.bind_vertex_array(None);
            gl.bind_texture(glow::TEXTURE_2D, None);
            gl.use_program(None);
        }
    }

    pub  fn build_ctx(&self, mpv: &mut Mpv) ->RenderContext {
        RenderContext::new(
            unsafe { mpv.ctx.as_mut() },
            vec![
                RenderParam::ApiType(RenderParamApiType::OpenGl),
                RenderParam::InitParams(OpenGLInitParams  {
                    get_proc_address,
                    ctx: self.proc_addr
                }),
            ],
        )
            .expect("Failed creating render context")
    }
}

impl MpvPlayer {
    const THREAD_AIR_SPACE:f64 = 0.05;
    const MPV_POLL_LIMIT:f64 = 0.025;
    const COMMAND_POLL_LIMIT:f64 = 0.025;

    fn poll_command(ctr: &Receiver<PlayerCommand>)->PlayerCommand {
        let mut ret = PlayerCommand::None;
        smol::block_on(async {
            futures::select! {
                _ = futures::FutureExt::fuse(smol::Timer::after(Duration::from_secs_f64(Self::COMMAND_POLL_LIMIT))) => {}
                received_command = ctr.recv().fuse() => {
                    if received_command.is_ok() {
                        ret = received_command.unwrap();
                    }
                }
            }
        });
        //println!("Polled: {ret}");
        ret
    }

    pub fn run(rc: &MpvShared, feed: Feeder, channel: usize, opts: &Options) -> Self {
        let (control_sender, control_receiver) = smol::channel::unbounded();
        let (event_sender, event_receiver) = smol::channel::unbounded();
        let options = opts.clone();

        if options.fixed_lib < usize::MAX {
            println!("Registering fixed lib: {}->{}",channel,options.fixed_lib);
            feed.register(channel, options.fixed_lib);
        }

        let mut mpv = Mpv::new().unwrap();
        mpv.set_property("hwdec","auto-unsafe").unwrap();
        mpv.set_property("vo", "libmpv").unwrap();
        mpv.set_property("autofit","100%").unwrap();
        mpv.set_property("keep-open", "yes").unwrap();


        if !options.with_audio { mpv.set_property("ao", "null").unwrap(); }
        else { mpv.set_property("vlang", "eng").unwrap(); }

        let render_ctx = rc.build_ctx(&mut mpv);
        let runner_thread = thread::Builder::new().name("runner thread".into()).spawn(move ||{
            let mut state = PlayerState::Sleeping;
            let mut clone_cmd = PlayerCommand::None;
            let mut terminal_pos: f64 = 0.0;
            loop {
                let mut reload = true;
                while state==PlayerState::Sleeping && clone_cmd < PlayerCommand::WakeTo0 {
                    clone_cmd = Self::poll_command(&control_receiver);
                }

                if clone_cmd >= PlayerCommand::WakeTo0 {
                    if options.fixed_lib == usize::MAX {
                        let itemi = clone_cmd as usize - PlayerCommand::WakeTo0 as usize;
                        feed.register(channel, itemi);
                    }
                    clone_cmd = PlayerCommand::Reset;
                }

                let mut path = "".to_string();
                match clone_cmd {
                    PlayerCommand::Sleep => {
                        println!("Sleeping: {channel}");
                        mpv.set_property("pause", "yes").unwrap_or_default();
                        state = PlayerState::Sleeping;
                        continue;
                    }
                    PlayerCommand::More => {
                        path = feed.clone_more(channel);
                    }
                    PlayerCommand::Diff => {
                        path = feed.clone_diff(channel);
                    }
                    PlayerCommand::Clone_4 => {
                        path = feed.clone_tagged(4, channel);
                    }
                    PlayerCommand::Reset => {
                        path = feed.next_tagged(channel);
                    }
                    PlayerCommand::Skip_F => {
                        let pos: f64 = mpv.get_property("time-pos").unwrap();
                        mpv.set_property("time-pos", format!("{}",pos+30.0)).unwrap();
                        reload = false;
                    }
                    PlayerCommand::Skip_R => {
                        let pos: f64 = mpv.get_property("time-pos").unwrap();
                        mpv.set_property("time-pos", format!("{}",pos-30.0)).unwrap();
                        reload = false;
                    }
                    _ => {}
                }
                clone_cmd = PlayerCommand::None;
                if reload {
                    println!("Next: {channel}->{path}");

                    let max_width = options.max_width;
                    let max_height = options.max_height;
                    let start = options.start_secs;
                    let speed = options.play_speed;
                    let end = options.end_secs;

                    mpv.set_property("start", start.to_string()).unwrap();
                    mpv.set_property("speed", speed.to_string()).unwrap();
                    if end != 0 {
                        mpv.set_property("end", end.to_string()).unwrap();
                    }
                    mpv.set_property("pause","yes").unwrap();
                    mpv.command("loadfile", &[&format!("\"{path}\"")])
                        .expect("Error loading file");

                    state = PlayerState::Normal;
                }

                let mut prepped = !reload;
                let mut started = !reload;//options.with_video;
                let ctr = control_receiver.clone();
                loop {
                    if reload {
                        println!("Config loop: {started}");
                        if started {
                            if mpv.get_property::<String>("duration").is_ok() {
                                prepped = true;

                                if options.end_secs < 0 {
                                    let d = mpv.get_property::<String>("duration").unwrap();

                                    terminal_pos = f64::from_str(&d).unwrap();
                                    terminal_pos += options.end_secs as f64;
                                } else if options.end_secs > 0 {
                                    terminal_pos = options.end_secs as f64;
                                } else {
                                    let d = mpv.get_property::<String>("duration").unwrap();
                                    terminal_pos = f64::from_str(&d).unwrap();
                                }
                                terminal_pos -= 3.0;
                                println!("Teriminal pos: {channel} {terminal_pos}");
                            }
                        }
                    }

                /*        let mut vwidth: i64 = mpv.get_property("dwidth").unwrap_or_default();
                        let mut vheight: i64 = mpv.get_property("dheight").unwrap_or_default();
                   //     println!("Checking video scale: {channel} {vwidth}x{vheight}");
                        if vwidth > max_width || vheight > max_height {
                            let vasp = vwidth as f32 / vheight as f32;
                            let mut ow = 0;
                            let mut oh = 0;
                            if vheight > max_height {
                                oh = max_height;
                                ow = (max_height as f32 * vasp) as i64;
                            } else if vwidth > max_width {
                                ow = max_width;
                                oh = (max_width as f32 / vasp) as i64;
                            }
                            //println!("Rescaling video: {channel} {vwidth}x{vheight}");
                            //let sx = ow as f32/vwidth as f32;
                            //let sy = oh as f32/vheight as f32;
                            //mpv.set_property("video-scale-x", sx.to_string()).unwrap();
                            //mpv.set_property("video-scale-y", sy.to_string()).unwrap();
                        }
                    }*/

                    mpv.set_property("pause", "no").unwrap_or_default();
                    let ev = mpv.event_context_mut();
                    ev.disable_deprecated_events().unwrap_or_default();
                    ev.observe_property("playback-time", libmpv::Format::Double, 0).unwrap();

                    loop {
                        let wev = ev.wait_event(0.2);

                        //          println!("RECEIVED PRE EVENT : {:?}", wev);
                        if wev.is_some() {
                            let er = wev.unwrap();
                            if er.is_ok() {
                                let ei = er.unwrap();
                         //       println!("RECEIVED PRE EVENT : {:?}", ei);
                                match ei {
                                    Event::PlaybackRestart => {
                                        started = true;
                                        break;
                                    }
                                    Event::PropertyChange { .. } =>{
                                        started = true;
                                        break;
                                    }
                                    _ => {}
                                }
                            } else { if prepped { break } }
                        } else { if prepped { break } }
                    }
                    println!("Prepped: {channel} {prepped}");
                    if(!prepped) { continue; }
                    clone_cmd = PlayerCommand::None;

                    loop {
                        if clone_cmd != PlayerCommand::None {
                            loop {
                                let wev = ev.wait_event(0.2);
                                if wev.is_some() {
                                    let er = wev.unwrap();
                                    if er.is_ok() {
                                        let ei = er.unwrap();
                                        match ei {
                                            Event::PropertyChange {..}=>{}
                                            _ => {
                                                continue;
                                            }
                                        }
                                    }
                                }
                                break;
                            }

                            println!("!!!");
                            break;
                        }

                        thread::sleep(Duration::from_secs_f64(Self::THREAD_AIR_SPACE));
                        //println!("Mux loop");

                        clone_cmd = Self::poll_command(&ctr);
                        if clone_cmd == PlayerCommand::None {
                                let event = ev.wait_event(Self::MPV_POLL_LIMIT);
                         //       println!(".");
                                if event.is_some() {
                                    let er = event.unwrap();
                                    if er.is_ok() {
                            //            println!("MUX EVENT : {:?}", er);
                                        let ei = er.unwrap();
                                        match ei {
                                            Event::PropertyChange {
                                                name: "playback-time",
                                                change: PropertyData::Double(val), .. } => {
                                                if terminal_pos <= val && terminal_pos > 0.0 {
                                                    clone_cmd = PlayerCommand::Reset;
                                                }
                                            }
                                            Event::Shutdown => {
                                                clone_cmd = PlayerCommand::Reset;
                                            }
                                            Event::EndFile(re) => {
                                                clone_cmd = PlayerCommand::Reset;
                                            }
                                            _ => {}
                                        }
                                    }
                                }

                        }
                    }
                    if clone_cmd != PlayerCommand::None {
                        break;
                    }
                }
            }
        }).unwrap();

        Self {
            channel,
            control_sender,
            event_receiver,
            runner_thread: Some(runner_thread),
            render_ctx
        }
    }
}