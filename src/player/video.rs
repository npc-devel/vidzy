use std::cmp::PartialEq;
use slint::JoinHandle;

#[derive(Debug)]
pub struct VideoPlaybackThread {
    control_sender: smol::channel::Sender<ControlCommand>,
    packet_sender: smol::channel::Sender<ffmpeg_next::codec::packet::packet::Packet>,
    receiver_thread: Option<std::thread::JoinHandle<()>>,
    dead: bool
}

impl VideoPlaybackThread {
    fn is_dead(&self) -> bool {
        self.dead
    }
    
    pub fn null()->Self {
        let (control_sender, control_receiver) = smol::channel::unbounded();
        let (packet_sender, packet_receiver) = smol::channel::bounded(VIDEO_Q_SIZE);
        Self {
            dead: true,
            control_sender,
            packet_sender,
            receiver_thread: None
        }        
    }
    
    pub fn start(
        mut css:f64,
        ces:f64,
        options: &Options,
        channel: i32,
        stream: &ffmpeg_next::format::stream::Stream,
        mut video_frame_callback: Box<dyn FnMut(i32,&ffmpeg_next::util::frame::Video) + Send>,
    ) -> Result<Self, anyhow::Error> {
        let (control_sender, control_receiver) = smol::channel::unbounded();
        let (packet_sender, packet_receiver) = smol::channel::bounded(VIDEO_Q_SIZE);

        let decoder_context = ffmpeg_next::codec::Context::from_parameters(stream.parameters())?;
        let mut packet_decoder = decoder_context.decoder().video()?;

        let tf = options.time_factor;
        let mut clock = StreamClock::new(stream,tf);
        let opt = options.clone();

        let receiver_thread =
            std::thread::Builder::new().name("video playback thread".into()).spawn(move || {
                smol::block_on(async move {
                    let packet_receiver_impl = async {
                        loop {
                            let Ok(packet) = packet_receiver.recv().await else { break };
                            smol::future::yield_now().await;
                            packet_decoder.send_packet(&packet).unwrap_or_default();
                            let mut decoded_frame = ffmpeg_next::util::frame::Video::empty();
                            let mut first = true;
                            //loop {
                                while packet_decoder.receive_frame(&mut decoded_frame).is_ok() {
                                    //if first && decoded_frame.pts().is_some() {
                                    
                                      //  first = false;
                                    //}
                                    
                                    if clock.hit_end(decoded_frame.pts(), ces) {
                                        println!("Hit end");
                                        return 
                                    }
                                    
                                    if let Some(delay) =
                                        clock.convert_pts_to_instant(decoded_frame.pts(), css)
                                    {
                                        //  smol::Timer::after(Duration::from_millis(1)).await;   
                                        //} else {
                                        
                                        if delay.as_millis() as f64 > 75.0*tf {
                                            css = clock.pts_to_secs(decoded_frame.pts());
                                            smol::Timer::after(Duration::from_millis(2)).await;
                                        } else {
                                            smol::Timer::after(delay).await;
                                        }
                                        //}
                                    }
                                    video_frame_callback(channel, &decoded_frame);
                                }
                                //smol::Timer::after(Duration::from_millis(100)).await;
                            //}
                        }
                    }
                    .fuse()
                    .shared();

                    let mut playing = true;

                    loop {
                        let packet_receiver: OptionFuture<_> =
                            if playing { Some(packet_receiver_impl.clone()) } else { None }.into();

                        smol::pin!(packet_receiver);

                        futures::select! {
                            _ = packet_receiver => { return  },
                            received_command = control_receiver.recv().fuse() => {
                                match received_command {
                                    Ok(ControlCommand::Pause) => {
                                        playing = false;
                                    }
                                    Ok(ControlCommand::Play) => {
                                        playing = true;
                                    }
                                    Ok(ControlCommand::Die) => {
                                        return;
                                    }
                                    Ok(_) => {}
                                    Err(_) => {
                                        // Channel closed -> quit
                                        return;
                                    }
                                }
                            }
                        }
                    }
                })
            })?;
        
        Ok(Self { control_sender, packet_sender, receiver_thread: Some(receiver_thread), dead: false })
    }

    pub async fn receive_packet(&self, packet: ffmpeg_next::codec::packet::packet::Packet) -> bool {
        if self.receiver_thread.is_none() { return true }
        match self.packet_sender.send(packet).await {
            Ok(_) => return true,
            Err(smol::channel::SendError(_)) => return false,
        }
    }

    pub async fn send_control_message(&self, message: ControlCommand) {         
        if self.receiver_thread.is_none() { return }
        self.control_sender.send(message).await.unwrap_or_default();                 
    }

    pub fn wait_exit(&mut self) {

        //self.packet_sender.close();
        //self.control_sender.close();
        //if let Some(receiver_join_handle) = self.receiver_thread.take() {
          //  receiver_join_handle.join().unwrap();
       // }
    }
}

impl Drop for VideoPlaybackThread {
    fn drop(&mut self) {
        self.control_sender.close();
        if let Some(receiver_join_handle) = self.receiver_thread.take() {
            receiver_join_handle.join().unwrap();
        }
    }
}
