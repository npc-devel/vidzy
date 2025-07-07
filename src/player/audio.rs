use cpal::{Device, SampleFormat, Stream};
use ffmpeg_the_third::{ChannelLayout, ChannelOrder};
use ffmpeg_the_third::ffi::AVChannelOrder::AV_CHANNEL_ORDER_UNSPEC;
use ffmpeg_the_third::format::sample::Type::{Packed, Planar};

//use tinyaudio::prelude::*;

pub struct AudioPlaybackThread {
    control_sender: smol::channel::Sender<ControlCommand>,
    packet_sender: smol::channel::Sender<ffmpeg_next::codec::packet::packet::Packet>,
    receiver_thread: Option<std::thread::JoinHandle<()>>,
    dead: bool
}

impl AudioPlaybackThread {
    fn is_dead(&self) -> bool {
        self.dead
    }
    
    pub fn null()->Self {
        let (control_sender, control_receiver) = smol::channel::unbounded();
        let (packet_sender, packet_receiver) = smol::channel::bounded(VIDEO_Q_SIZE);
        Self {
            control_sender,
            packet_sender,
            receiver_thread: None,
            dead: true
        }
    }


    pub fn start(stream: &ffmpeg_next::format::stream::Stream) -> Result<Self, anyhow::Error> {
        let (control_sender, control_receiver) = smol::channel::unbounded();

        let (packet_sender, packet_receiver) = smol::channel::bounded(AUDIO_Q_SIZE);

        let decoder_context = ffmpeg_next::codec::Context::from_parameters(stream.parameters())?;
        //let decoder_context = ffmpeg_next::codec::Context::new();
        let packet_decoder = decoder_context.decoder().audio()?;

        let host = cpal::default_host();
        let device = host.default_output_device().expect("no output device available");
        
        let mut stor = ffmpeg_next::util::format::sample::Type::Packed;
        //let t = packet_decoder.
        let named_fmt = packet_decoder.format();
      //      if named_fmt.is_planar() { stor = ffmpeg_next::util::format::sample::Type::Planar; }

        for op in device.supported_input_configs()? {
            
                
          //      println!("Inout config: {:?}", op);
                
            }
        
        // println!("Looking for: {:?}", named_fmt0.name());
        //println!("Output config: {:?}", stream.parameters().);
        let mut config = device.default_output_config()?;
        if  packet_decoder.ch_layout().channels()!=config.channels() as u32 {
            for op in device.supported_output_configs()? {
                if op.channels() as u32 == packet_decoder.ch_layout().channels() 
                { 
                 
                    config = op.with_sample_rate(config.sample_rate());
             //       println!("Output config: {:?}", config);
                    break;
                }
            }
        }

        let receiver_thread =
            std::thread::Builder::new().name("audio playback thread".into()).spawn(move || {
                smol::block_on(async move {
                    let output_channel_layout = match config.channels() {
                        //_ => ffmpeg_next::util::channel_layout::ChannelLayout::default_for_channels(config.channels() as u32)
                        1 => ffmpeg_next::util::channel_layout::ChannelLayout::MONO,
                        _ => ffmpeg_next::util::channel_layout::ChannelLayout::STEREO,
                        //_ => todo!(),
                    };

                    let mut ffmpeg_to_cpal_forwarder = match config.sample_format() {
                            cpal::SampleFormat::F64 => FFmpegToCPalForwarder::new::<f64>(
                                config,
                                &device,
                                packet_receiver,
                                packet_decoder,
                                ffmpeg_next::util::format::sample::Sample::F64(
                                    stor,
                                ),
                                output_channel_layout
                            ),
                            cpal::SampleFormat::F32 => FFmpegToCPalForwarder::new::<f32>(
                                config,
                                &device,
                                packet_receiver,
                                packet_decoder,
                                ffmpeg_next::util::format::sample::Sample::F32(
                                    stor,
                                ),
                                output_channel_layout
                            ),
                            cpal::SampleFormat::U8 => FFmpegToCPalForwarder::new::<u8>(
                                config,
                                &device,
                                packet_receiver,
                                packet_decoder,
                                ffmpeg_next::util::format::sample::Sample::U8(
                                    stor,
                                ),
                                output_channel_layout
                            ),
                            _ => todo!("bad output")
                        };
                    

                    let packet_receiver_impl =
                        async { ffmpeg_to_cpal_forwarder.stream().await }.fuse().shared();

                    let mut playing = true;

                    loop {
                        let packet_receiver: OptionFuture<_> =
                            if playing { Some(packet_receiver_impl.clone()) } else { None }.into();

                        smol::pin!(packet_receiver);

                        futures::select! {
                            _ = packet_receiver => {},
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

        Ok(Self { control_sender, packet_sender, receiver_thread: Some(receiver_thread),dead: false })
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
        while !self.packet_sender.is_empty() {
            thread::sleep(std::time::Duration::from_millis(100));
        }
        self.packet_sender.close();
        
    }
}

impl Drop for AudioPlaybackThread {
    fn drop(&mut self) {
        self.control_sender.close();
        if let Some(receiver_join_handle) = self.receiver_thread.take() {
            receiver_join_handle.join().unwrap();
        }
    }
}

trait FFMpegToCPalSampleForwarder {

    fn forward(
        &mut self,
        audio_frame: ffmpeg_next::frame::Audio,
    ) -> Pin<Box<dyn Future<Output = ()> + '_>>;
    fn fluahed(&self)->bool;
}

impl<T: Pod, R: RbRef> FFMpegToCPalSampleForwarder for ringbuf::Producer<T, R>
where
    <R as RbRef>::Rb: RbWrite<T>,
{
    fn fluahed(&self)->bool {
        self.is_empty()
    }
    
    fn forward(
        &mut self,
        audio_frame: ffmpeg_the_third::frame::Audio,
    ) -> Pin<Box<dyn Future<Output = ()> + '_>> {
        Box::pin(async move {
            // Audio::plane() returns the wrong slice size, so correct it by hand. See also
            // for a fix https://github.com/zmwangx/rust-ffmpeg/pull/104.
            let expected_bytes = 
                //audio_frame.data(0).len();
                audio_frame.samples() * audio_frame.ch_layout().channels() as usize * core::mem::size_of::<T>();
               // let expected_bytes = audio_frame.length();
            let cpal_sample_data: &[T] =
                bytemuck::cast_slice(&audio_frame.data(0)[..expected_bytes]);

            while self.free_len() < cpal_sample_data.len() {
                //print!(".");
                smol::Timer::after(std::time::Duration::from_millis(16)).await;
            }
            // Buffer the samples for playback
            self.push_slice(cpal_sample_data);

            //smol::Timer::after(std::time::Duration::from_millis(20)).await;
        })
    }
}

struct FFmpegToCPalForwarder {
    out_stream: Stream,
    ffmpeg_to_cpal_pipe: Box<dyn FFMpegToCPalSampleForwarder>,
    packet_receiver: smol::channel::Receiver<ffmpeg_next::codec::packet::packet::Packet>,
    packet_decoder: ffmpeg_next::decoder::Audio,
    resampler: ffmpeg_next::software::resampling::Context,
}

impl FFmpegToCPalForwarder {

    fn new<T: Send + Pod + SizedSample + 'static>(
        config: cpal::SupportedStreamConfig,
        device: &cpal::Device,
        packet_receiver: smol::channel::Receiver<ffmpeg_next::codec::packet::packet::Packet>,
        packet_decoder: ffmpeg_next::decoder::Audio,
        output_format: ffmpeg_the_third::util::format::sample::Sample,
        output_channel_layout: ffmpeg_the_third::util::channel_layout::ChannelLayout,
    ) -> Self {
        let buffer = HeapRb::new(CPAL_BUFFER_SIZE);
        let (sample_producer, mut sample_consumer) = buffer.split();
   
        let oc = packet_decoder.ch_layout().channels() as usize;
        let ora = packet_decoder.rate() as usize; 
        let out_stream = 
        /*run_output_device(
            OutputDeviceParameters {
                channels_count: 2,
                sample_rate: ora,
                channel_sample_count: ora
            },
            move |buffer| {
                let channels = buffer.chunks_mut(8);
                for data in channels {
                    let filled = sample_consumer.pop_slice(data);
                    data[filled..].fill(0.0);
                }
            }).unwrap();*/
        
        device.build_output_stream(
                &config.config(),
                move |data: &mut [T], _| {
                    let filled = sample_consumer.pop_slice(data);
                    data[filled..].fill(T::EQUILIBRIUM);

                },
                move |err| {
                    eprintln!("error feeding audio stream to cpal: {}", err);
                },
                None,
            )
            .unwrap();

        out_stream.play().unwrap();
        
        let mut layout = packet_decoder.ch_layout();
        if layout.order() == ChannelOrder::from(AV_CHANNEL_ORDER_UNSPEC) {
            //       layout = ChannelLayout::default_for_channels(2);
            //     println!("Forced layout: {:?}",layout);
        }

        let resampler =
            packet_decoder.resampler2(            output_format,
                                                  layout,
                                                  config.sample_rate().0).ok().unwrap();
            
            /*ffmpeg_next::software::resampling::Context::get2(
                packet_decoder.format(),
            ChannelLayout::default_for_channels(packet_decoder.ch_layout().channels()),
            packet_decoder.rate(),
            ffmpeg_next::util::format::sample::Sample::F32(Packed),
            ChannelLayout::default_for_channels(2),
                48000
        )
        .unwrap();*/

        Self {
            out_stream,
            ffmpeg_to_cpal_pipe: Box::new(sample_producer),
            packet_receiver,
            packet_decoder,
            resampler,
        }
    }

    async fn stream(&mut self) {
        loop {
            // Receive the next packet from the packet receiver channel.
            let Ok(packet) = self.packet_receiver.recv().await else {
                while !self.ffmpeg_to_cpal_pipe.fluahed() {
                    thread::sleep(std::time::Duration::from_millis(100));
                }
                
                //smol::Timer::after(std::time::Duration::from_millis(2000)).await;
                return
            };
    
            // Send the packet to the decoder.
            self.packet_decoder.send_packet(&packet).unwrap();
            // Create an empty frame to hold the decoded audio data.
            let mut decoded_frame = ffmpeg_next::util::frame::Audio::empty();
            // Continue receiving decoded frames until there are no more available.
            while self.packet_decoder.receive_frame(&mut decoded_frame).is_ok() {
                // Create an empty frame to hold the resampled audio data.
                let mut resampled_frame = ffmpeg_next::util::frame::Audio::empty();
                // Resample the decoded audio frame to match the output format and channel layout.
                self.resampler.run(&decoded_frame, &mut resampled_frame).unwrap_or_default();
                // Forward the resampled audio frame to the CPAL audio output.
                self.ffmpeg_to_cpal_pipe.forward(resampled_frame).await;
            }
        }
    }
}
