use ringbuf::ring_buffer::RbBase;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SizedSample;
use ringbuf::ring_buffer::RbRef;
use ringbuf::ring_buffer::RbWrite;
use ringbuf::HeapRb;

include!("clock.rs");
include!("rescale.rs");
include!("audio.rs");
include!("video.rs");

use ffmpeg_the_third as ffmpeg_next;
use ffmpeg_next::{rescale,Rescale};
use ffmpeg_next::format::Pixel;
use ffmpeg_next::Error;
use ffmpeg_next::Error::OptionNotFound;
use ffmpeg_next::media::Type::Video;
use ffmpeg_the_third::Dictionary;

pub struct FFMpegPlayer {
    pub(crate) control_sender: smol::channel::Sender<ControlCommand>,
    pub(crate) event_receiver: smol::channel::Receiver<ControlCommand>,
    runner_thread: Option<std::thread::JoinHandle<()>>
//    playing: bool
}

impl FFMpegPlayer {
    fn demux(cmd: smol::channel::Receiver<ControlCommand>,
             feed: Feeder,
             lib: &str,
             output: i32,
             options: Options,
             app: Weak<App>) {
        
            smol::block_on(async move {
                let mut clone_cmd = ControlCommand::None;
                loop {
                    let mut video_playback_thread_r: Result<VideoPlaybackThread, _>;
                    let mut path = "".to_string();
                    match clone_cmd {
                        ControlCommand::More => {
                            path = feed.clone_more(output as usize, &lib);
                        }
                        ControlCommand::Diff => {
                            path = feed.clone_diff(output as usize, &lib);
                        }
                        ControlCommand::Clone_4 => {
                            path = feed.clone_tagged(4,output as usize, &lib);    
                        }
                        _ => {
                            path = feed.next_tagged(output as usize, &lib);    
                        }
                    }
                    clone_cmd = ControlCommand::None;
                    println!("Next: {output}->{path}");
                    
                    //let input_context = ic.unwrap();
                    let mut video_stream_index = usize::MAX;
                    let mut audio_stream_index = usize::MAX;
                    let mut opt_secs = options.start_secs;
                    let mut opt_tb = Rescale::rescale(&(opt_secs), (1, 1), rescale::TIME_BASE);;
    
                    let mut opt_esecs = options.end_secs;
                    let mut opt_etb = Rescale::rescale(&(opt_esecs), (1, 1), rescale::TIME_BASE);
    
                    let mut audio_playback_thread = crate::player::AudioPlaybackThread::null();
                    let mut video_playback_thread = VideoPlaybackThread::null();
    
                    let mut dic = Dictionary::new();
                    //-filter_complex "[0:a]channelsplit=channel_layout=stereo:channels=FR[right]" -map "[right]" front_right.wav
                    // -map a:0 -c:a:0 aac -b:a:0 256 -ac:a:0 2 -af "aresample=matrix_encoding=dplii" 
                    //dic.set("-i", "pipe:0");
                   // println!("Dictionary: {:?}",&dic);
                    
                    let input_context_r = ffmpeg_next::format::input_with_dictionary(&path, dic);
                    if input_context_r.is_err() { return }
    
                    let mut input_context = input_context_r.unwrap();
                    
                    let dur = input_context.duration();
                    let mut eff_tb: i64 = 0;
                    let delta = Rescale::rescale(&(30), (1, 1), rescale::TIME_BASE);
                    let max = dur - delta / 3;
                    let min = delta / 3;
    
                    if opt_etb != 0 {
                        if opt_etb < 0 { if dur > -opt_etb { opt_etb = dur + opt_etb; } } else if dur < opt_etb { opt_etb = dur / 2; }
                    }
    
                    loop {
                        let ap = app.clone();
    
                        if opt_tb != 0 {
                            if opt_tb < 0 { if dur > -opt_tb { eff_tb = dur + opt_tb; } } else if dur < opt_tb { eff_tb = dur / 2; } else { eff_tb = opt_tb; }
                        }
    
                        if eff_tb != 0 {
                            println!("Starting tb: {}", eff_tb);
                            let mut limit = 5;
                            while limit > 0 && input_context.seek(eff_tb as i64, 0..dur).is_err() {
                                println!("Error seeking: {}", eff_tb);
                                thread::sleep(Duration::from_millis(1));
                                limit -= 1;
                            }
                            println!("Starting tb done!");
                        }
    
                        let ss = Rescale::rescale(&eff_tb, rescale::TIME_BASE, (1, 1)) as f64;
                        let es = Rescale::rescale(&opt_etb, rescale::TIME_BASE, (1, 1)) as f64;
    
                        let mut has_any = false;
    
                        if options.with_audio {
                            let audio_stream = input_context.streams().best(ffmpeg_next::media::Type::Audio).unwrap();
                            let audio_playback_thread_r =
                                AudioPlaybackThread::start(&audio_stream);
                            if !audio_playback_thread_r.is_err() {
                                has_any = true;
                                audio_playback_thread = audio_playback_thread_r.unwrap();
                                audio_stream_index = audio_stream.index();
                            }
                        }
    
                        if options.with_video {
                            let video_stream =
                                input_context.streams().best(ffmpeg_next::media::Type::Video).unwrap_or(input_context.streams().nth(0).unwrap());
    
                            let mut qs = 1.0;
                            if options.time_factor != 1.0 {
                                qs = 0.5;
                            }
    
                            video_playback_thread_r = VideoPlaybackThread::start(
                                ss,
                                es,
                                &options,
                                output,
                                &video_stream,
                                Box::new(vframe_cb!(qs,ap))
                            );
    
                            if !video_playback_thread_r.is_err() {
                                has_any = true;
                                video_stream_index = video_stream.index();
                                video_playback_thread = video_playback_thread_r.unwrap();
                            }
                        }
    
                        if (!has_any) { return }
                        let mut playing = true;
                        let (control_sender2, control_receiver2) = smol::channel::unbounded();
                        let mut packet_forwarder_impl = async {
                            let mut pts = 0;
                            for ifc in input_context.packets() {
                                if ifc.is_err() { continue }
    
                                let (stream, packet) = ifc.unwrap();
                                if packet.is_corrupt() { continue }
                              //  print!("{}",stream.index());
                                if stream.index() == audio_stream_index {
                                    audio_playback_thread.receive_packet(packet).await;
                                } else if stream.index() == video_stream_index {
                                    if packet.pts().is_some() {
                                        pts = packet.pts().unwrap_or_else(|| pts);
                                        video_playback_thread.receive_packet(packet).await;
                                    }
                                }
                                if !control_receiver2.is_empty() {
                                    audio_playback_thread.send_control_message(ControlCommand::Die).await;
                                    video_playback_thread.send_control_message(ControlCommand::Die).await;
                                    let time_base_seconds = stream.time_base();
                                    let time_base_seconds =
                                        time_base_seconds.numerator() as f64 / time_base_seconds.denominator() as f64;
    
    
                                    let secs_since_start =
                                        std::time::Duration::from_secs_f64(pts as f64 * time_base_seconds);
    
                                    println!("Returning: {pts}");
                                    return secs_since_start.as_millis();
                                }
                            }
                            audio_playback_thread.wait_exit();
                            video_playback_thread.wait_exit();
                            
                            audio_playback_thread.send_control_message(ControlCommand::Die).await;
                            video_playback_thread.send_control_message(ControlCommand::Die).await;
                            return u128::MAX;
                        }.fuse().shared();
    
                        let mut seeking = false;
                        loop {
                            let packet_forwarder: OptionFuture<_> =
                                if playing { Some(packet_forwarder_impl.clone()) } else { None }.into();
    
                            smol::pin!(packet_forwarder);
                            futures::select! {
                                pts = packet_forwarder => {
                                    return
                                }, // playback finished
                                received_command = cmd.recv().fuse() => {
                                    match received_command {
                                        Ok(command) => {
                                            //video_playback_thread.send_control_message(command).await;
                                            //audio_playback_thread.send_control_message(command).await;
                                            match command {
                                                ControlCommand::Diff => {
                                                    control_sender2.send(ControlCommand::Die).await.unwrap_or_default();
                                                    clone_cmd = ControlCommand::Diff;
                                                    break;
                                                },
                                                ControlCommand::More => {
                                                    control_sender2.send(ControlCommand::Die).await.unwrap_or_default();
                                                    clone_cmd = ControlCommand::More;
                                                    break;
                                                },
                                                ControlCommand::Clone_4 => {
                                                    control_sender2.send(ControlCommand::Die).await.unwrap_or_default();
                                                    clone_cmd = ControlCommand::Clone_4;
                                                    break;
                                                },
                                                ControlCommand::Play => {
                                                    // Continue in the loop, polling the packet forwarder future to forward
                                                    // packets
                                                    playing = true;
                                                },
                                                ControlCommand::Skip_F => {
                                                    //println!("Skip fwd");
                                                    control_sender2.send(ControlCommand::Die).await.unwrap_or_default();
                                                    let epts: u64 = Rescale::rescale(&((packet_forwarder_impl.await/1000) as i64), (1, 1), rescale::TIME_BASE) as u64;
                                                    opt_tb = epts as i64 + delta;
                                                    if opt_tb > max {
                                                        opt_tb = max;
                                                    }
                                                    //println!("Skip fwd: {epts} -> {opt_pts}");
                                                    break;
                                                }
                                                ControlCommand::Skip_R => {
                                                    //println!("Skip rev");
                                                    control_sender2.send(ControlCommand::Die).await.unwrap_or_default();
                                                    let epts: u64 = Rescale::rescale(&((packet_forwarder_impl.await/1000) as i64), (1, 1), rescale::TIME_BASE) as u64;
                                                    opt_tb = epts as i64 - delta;
                                                    if opt_tb < min {
                                                        opt_tb = min;
                                                    }
                                                    //println!("Skip rev: {epts} --> {opt_pts}");
                                                    break;
                                                }
                                                ControlCommand::Pause => {
                                                    playing = false;
                                                }
                                                ControlCommand::Reset => {
                                                    return;
                                                }
                                                _ => {}
                                            }
                                        }
                                        Err(_) => {
                                            // Channel closed -> quit
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                        match clone_cmd  {
                            ControlCommand::None => {}
                            _ => { break }
                        }
                    }
                    match clone_cmd  {
                        ControlCommand::None => { break }
                        _ => {  }
                    }
                }
            });
    }

    pub fn run(feed: Feeder,
                   lib: &str,
                   output: i32,
                   options: Options,
                   app: &App) -> Self {
            let (control_sender, control_receiver) = smol::channel::unbounded();
            let (event_sender, event_receiver) = smol::channel::unbounded();
            //et vfc = Arc::new(video_frame_callback);

            //let ct = control_receiver.clone();
            //let ev = event_sender.clone();

            let lis = lib.to_string().to_owned();
            let aw = app.as_weak();
            let runner_thread =
                thread::Builder::new().name("runner thread".into()).spawn(move || {
                    loop {
                        println!("FFmpeg run loop");
                        let ap = aw.clone();
                        let fed = feed.clone();
                        let li = lis.clone();
                        let opt = options.clone();
                        let cmd = control_receiver.clone();
                        let dem_t =
                            thread::Builder::new().name("demuxer thread".into()).spawn(move || {
                                Self::demux(cmd, fed, li.as_str(), output, opt, ap);
                                //})
                            });

                        //if dem_t.is_ok() { dem_t.unwrap().join().unwrap(); }
                        dem_t.unwrap().join().unwrap_or_default();
                        println!("Demux exit");
                        thread::sleep(Duration::from_millis(100));
                    }
                }).unwrap();

            /*if !event_receiver.is_empty() {
                Err(anyhow::anyhow!("Player event receiver dropped"))
            } else {*/

            Self {
                event_receiver,
                control_sender,
                runner_thread: Some(runner_thread)
                //playing,
                //playing_changed_callback: Box::new(playing_changed_callback),
            }
        }
    }
/*
pub struct Player2 {
    control_sender: smol::channel::Sender<ControlCommand>,
    pub(crate) event_receiver: smol::channel::Receiver<ControlCommand>,
    //demuxer_thread: Result<std::thread::JoinHandle<()>,Error>,
    playing: bool,
 //   playing_changed_callback: Box<dyn Fn(bool)>,
}

impl Player2 {
    pub fn start(
        //app: &App,
        feed: Feeder,
        lib: &str,
        options: Options,
        video_frame_callback: impl FnMut(i32,&ffmpeg_next::util::frame::Video) + Send + 'static,
        //playing_changed_callback: impl Fn(bool) + 'static,
    ) -> Result<(Result<std::thread::JoinHandle<()>,ffmpeg_next::util::error::Error>,Self), anyhow::Error> {
        let (control_sender, control_receiver) = smol::channel::unbounded();
        let (event_sender, event_receiver) = smol::channel::unbounded();
        let mylib = lib.to_string();
        //et vfc = Arc::new(video_frame_callback);
        let demuxer_thread =
            std::thread::Builder::new().name("demuxer thread".into()).spawn(move || {
                    smol::block_on(async move {
                            let mut input_context;
                            let mut video_playback_thread_r: Result<VideoPlaybackThread, _>;

                            let path = feed.next(&mylib);
                            let ic = ffmpeg_next::format::input(&path);
                            if ic.is_err() { return }

                            input_context = ic.unwrap();
                            let mut vpt: Option<VideoPlaybackThread> = None;
                            let mut video_stream_index;
                            let mut start_pts = options.start_pts();
                            if options.with_video {
                                let dur = input_context.duration();
                                let mut eff_pts: u64 = 0;
                                if start_pts != 0 {
                                    if start_pts < 0 { if dur > -start_pts { eff_pts = (dur - start_pts) as u64 } }
                                }

                                let video_stream =
                                    input_context.streams().best(ffmpeg_next::media::Type::Video).unwrap_or(input_context.streams().nth(0).unwrap());
                                video_stream_index = video_stream.index();
                                video_playback_thread_r = VideoPlaybackThread::start(
                                    eff_pts,
                                    options.output,
                                    &video_stream,
                                    Box::new(video_frame_callback)
                                );

                                if video_playback_thread_r.is_err() { return }

                                if start_pts != 0 {
                                    if start_pts < 0 { start_pts += input_context.duration()  }
                                    if start_pts < 0 { start_pts = 0 }
                                    println!("Starting pts: {}", start_pts);
                                    input_context.seek(start_pts, ..start_pts).unwrap_or_default();
                                }

                                let video_playback_thread = video_playback_thread_r.unwrap();
                                vpt = Some(video_playback_thread);
                            }

                            let audio_stream_r = input_context.streams().best(ffmpeg_next::media::Type::Audio);
                            if audio_stream_r.is_none() { return  }
                            let audio_stream = audio_stream_r.unwrap();
                            let audio_stream_index = audio_stream.index();
                            let audio_playback_thread =
                                AudioPlaybackThread::start(&audio_stream).unwrap();

                            let mut playing = true;
                            // This is sub-optimal, as reading the packets from ffmpeg might be blocking
                            // and the future won't yield for that. So while ffmpeg sits on some blocking
                            // I/O operation, the caller here will also block and we won't end up polling
                            // the control_receiver future further down.
                            let packet_forwarder_impl = async {
                                if vpt.is_some() {
                                    let video_playback_thread = vpt.take().unwrap();
                                    for (stream, packet) in input_context.packets() {
                                        if stream.index() == audio_stream_index {
                                            if options.with_audio { audio_playback_thread.receive_packet(packet).await; }
                                        } else {
                                            if options.with_video { video_playback_thread.receive_packet(packet).await; }
                                        }
                                    }
                                } else {
                                    for (stream, packet) in input_context.packets() {
                                        if stream.index() == audio_stream_index {
                                            if options.with_audio { audio_playback_thread.receive_packet(packet).await; }
                                        }
                                    }
                                }
                            }.fuse().shared();

                            loop {
                                // This is sub-optimal, as reading the packets from ffmpeg might be blocking
                                // and the future won't yield for that. So while ffmpeg sits on some blocking
                                // I/O operation, the caller here will also block and we won't end up polling
                                // the control_receiver future further down.
                                let packet_forwarder: OptionFuture<_> =
                                    if playing { Some(packet_forwarder_impl.clone()) } else { None }.into();

                                smol::pin!(packet_forwarder);

                                futures::select! {
                                    _ = packet_forwarder => {}, // playback finished
                                    received_command = control_receiver.recv().fuse() => {
                                        match received_command {
                                            Ok(command) => {
                                                //video_playback_thread.send_control_message(command).await;
                                                //audio_playback_thread.send_control_message(command).await;
                                                match command {
                                                    ControlCommand::Play => {
                                                        // Continue in the loop, polling the packet forwarder future to forward
                                                        // packets
                                                        playing = true;
                                                    },
                                                    ControlCommand::Pause => {
                                                        playing = false;
                                                    }
                                                    _ => {}
                                                }
                                            }
                                            Err(_) => {
                                                // Channel closed -> quit
                                                return;

                                            }
                                        }
                                    }
                                }
                            
                                //         event_sender.send(ControlCommand::Finished).await.unwrap();
                            }
                    });
                event_sender.send_blocking(ControlCommand::Finished).unwrap();
            });

        let playing = true;
      //  playing_changed_callback(playing);
        thread::sleep(Duration::from_millis(150));
        
        if !event_receiver.is_empty() {
            Err(anyhow::anyhow!("Player event receiver dropped"))    
        } else {
            Ok((Ok(demuxer_thread?),Self {
                event_receiver,
                control_sender,
                //demuxer_thread: ,
                playing,
                //playing_changed_callback: Box::new(playing_changed_callback),
            }))
        }
    }

    pub fn toggle_pause_playing(&mut self) {
        if self.playing {
            self.playing = false;
            self.control_sender.send_blocking(ControlCommand::Pause).unwrap();
        } else {
            self.playing = true;
            self.control_sender.send_blocking(ControlCommand::Play).unwrap();
        }
        //(self.playing_changed_callback)(self.playing);
    }
}
*/
/*impl Drop for Player {
    fn drop(&mut self) {
        self.control_sender.send_blocking(ControlCommand::Die).unwrap();
          //if let Some(runner_thread) = self.runner_thread.take() {
            //self.runner_thread.join().unwrap();
        //}
    }
}*/
