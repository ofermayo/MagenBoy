use std::{ffi::{CStr, c_void}, mem::MaybeUninit};
use lib_gb::{GB_FREQUENCY, apu::audio_device::*};
use sdl2::{sys::*,libc::c_char};

#[cfg(not(feature = "sdl-resample"))]
use crate::audio_resampler::AudioResampler;
#[cfg(feature = "sdl-resample")]
use crate::sdl_audio_resampler::SdlAudioResampler as AudioResampler;

#[cfg(not(feature = "push-audio"))]
use crossbeam_channel::{Receiver, SendError, Sender, bounded};

//After twicking those numbers Iv reached this, this will affect fps which will affect sound tearing
#[cfg(feature = "push-audio")]
const BYTES_TO_WAIT:u32 = BUFFER_SIZE as u32 * 16;
const VOLUME:Sample = 10 as Sample;

fn get_sdl_error_message()->&'static str{
    unsafe{
        let error_message:*const c_char = SDL_GetError();
        
        return CStr::from_ptr(error_message).to_str().unwrap();
    }
}

#[cfg(not(feature = "push-audio"))]
struct Data{
    rx: Receiver<[Sample;BUFFER_SIZE]>,
    current_buf: Option<[Sample;BUFFER_SIZE]>,
    current_buf_index:usize,
}

#[cfg(not(feature = "push-audio"))]
pub struct SdlPullAudioDevice{
    resampler: AudioResampler,
    buffer: [Sample;BUFFER_SIZE],
    buffer_index:usize,

    tarnsmiter: Sender<[Sample;BUFFER_SIZE]>,

    userdata: Data
}

#[cfg(not(feature = "push-audio"))]
impl SdlPullAudioDevice{
    pub fn new(frequency:i32, turbo_mul:u8)->Self{

        // cap of less than 2 hurts the fps
        let(s,r) = bounded(2);
        let data = Data{
            current_buf:Option::None,
            current_buf_index:0,
            rx:r
        };

        let mut device = SdlPullAudioDevice{
            buffer:[DEFAULT_SAPMPLE;BUFFER_SIZE],
            buffer_index:0,
            resampler: AudioResampler::new(GB_FREQUENCY * turbo_mul as u32, frequency as u32),
            tarnsmiter:s,
            userdata:data
        };
        
        let desired_audio_spec = SDL_AudioSpec{
            freq: frequency,
            format: AUDIO_S16SYS as u16,
            channels: 2,
            silence: 0,
            samples: BUFFER_SIZE as u16,
            padding: 0,
            size: 0,
            callback: Option::Some(audio_callback),
            userdata: (&mut device.userdata) as *mut Data as *mut c_void
        };

        
        let mut uninit_audio_spec:MaybeUninit<SDL_AudioSpec> = MaybeUninit::uninit();

        unsafe{
            SDL_Init(SDL_INIT_AUDIO);
            SDL_ClearError();
            let id = SDL_OpenAudioDevice(std::ptr::null(), 0, &desired_audio_spec, uninit_audio_spec.as_mut_ptr() , 0);

            if id == 0{
                std::panic!("{}", get_sdl_error_message());
            }

            let init_audio_spec:SDL_AudioSpec = uninit_audio_spec.assume_init();

            if init_audio_spec.freq != frequency {
                std::panic!("Error initializing audio could not use the frequency: {}", frequency);
            }

            //This will start the audio processing
            SDL_PauseAudioDevice(id, 0);
        };

        return device;
    }

    fn push_audio_to_device(&self, audio:&[Sample; BUFFER_SIZE])->Result<(), SendError<[Sample; BUFFER_SIZE]>>{
        self.tarnsmiter.send(audio.clone())
    }
}

#[cfg(not(feature = "push-audio"))]
unsafe extern "C" fn audio_callback(userdata:*mut c_void, buffer:*mut u8, length:i32){
    let length = length as usize;
    let s = &mut *(userdata as *mut Data);

    if s.current_buf.is_none(){
        s.current_buf = Some(s.rx.recv().unwrap());
    }

    let samples = s.current_buf.unwrap();
    let samples_size = (samples.len() * std::mem::size_of::<Sample>()) - s.current_buf_index;
    let samples_ptr = (samples.as_ptr() as *mut u8).add(s.current_buf_index);
    std::ptr::copy_nonoverlapping(samples_ptr, buffer, std::cmp::min(length, samples_size));

    if length > samples_size && s.rx.is_empty(){
        s.current_buf = Option::None;
        s.current_buf_index = 0;
        std::ptr::write_bytes(buffer.add(samples.len() as usize), 0, length  - samples_size);
    }
    else if length > samples_size{
        s.current_buf = Option::None;
        s.current_buf_index = 0;
        audio_callback(userdata, buffer.add(samples_size), (length - samples_size) as i32);
    }
    else{
        s.current_buf_index = length;
    }
}

#[cfg(not(feature = "push-audio"))]
impl AudioDevice for SdlPullAudioDevice{
    fn push_buffer(&mut self, buffer:&[StereoSample; BUFFER_SIZE]){
        let resample = self.resampler.resample(buffer);
        for sample in resample{

            self.buffer[self.buffer_index] = sample.left_sample * VOLUME;
            self.buffer[self.buffer_index + 1] = sample.right_sample * VOLUME;
            self.buffer_index += 2;
            if self.buffer_index == BUFFER_SIZE{
                self.push_audio_to_device(&self.buffer).unwrap();
                self.buffer_index = 0;
            }
        }
    }
}


#[cfg(feature = "push-audio")]
pub struct SdlPushAudioDevice{
    device_id: SDL_AudioDeviceID,
    resampler: AudioResampler,

    buffer: [Sample;BUFFER_SIZE],
    buffer_index:usize,
}

#[cfg(feature = "push-audio")]
impl SdlPushAudioDevice{
    pub fn new(frequency:i32, turbo_mul:u8)->Self{
        let desired_audio_spec = SDL_AudioSpec{
            freq: frequency,
            format: AUDIO_S16SYS as u16,
            channels: 2,
            silence: 0,
            samples: BUFFER_SIZE as u16,
            padding: 0,
            size: 0,
            callback: Option::None,
            userdata: std::ptr::null_mut()
        };

        
        let mut uninit_audio_spec:MaybeUninit<SDL_AudioSpec> = MaybeUninit::uninit();

        let device_id = unsafe{
            SDL_Init(SDL_INIT_AUDIO);
            SDL_ClearError();
            let id = SDL_OpenAudioDevice(std::ptr::null(), 0, &desired_audio_spec, uninit_audio_spec.as_mut_ptr() , 0);

            if id == 0{
                std::panic!("{}", get_sdl_error_message());
            }

            let init_audio_spec:SDL_AudioSpec = uninit_audio_spec.assume_init();

            if init_audio_spec.freq != frequency {
                std::panic!("Error initializing audio could not use the frequency: {}", frequency);
            }

            //This will start the audio processing
            SDL_PauseAudioDevice(id, 0);

            id
        };
        return SdlPushAudioDevice{
            device_id: device_id,
            buffer:[DEFAULT_SAPMPLE;BUFFER_SIZE],
            buffer_index:0,
            resampler: AudioResampler::new(GB_FREQUENCY * turbo_mul as u32, frequency as u32)
        };
    }

    fn push_audio_to_device(&self, audio:&[Sample; BUFFER_SIZE])->Result<(),&str>{
        let audio_ptr: *const c_void = audio.as_ptr() as *const c_void;
        let data_byte_len = (audio.len() * std::mem::size_of::<Sample>()) as u32;

        unsafe{
            while SDL_GetQueuedAudioSize(self.device_id) > BYTES_TO_WAIT{
                SDL_Delay(1);
            }

            SDL_ClearError();
            if SDL_QueueAudio(self.device_id, audio_ptr, data_byte_len) != 0{
                return Err(get_sdl_error_message());
            }
            
            Ok(())
        }
    }
}

#[cfg(feature = "push-audio")]
impl AudioDevice for SdlPushAudioDevice{
    fn push_buffer(&mut self, buffer:&[StereoSample; BUFFER_SIZE]){
        let resample = self.resampler.resample(buffer);
        for sample in resample{

            self.buffer[self.buffer_index] = sample.left_sample * VOLUME;
            self.buffer[self.buffer_index + 1] = sample.right_sample * VOLUME;
            self.buffer_index += 2;
            if self.buffer_index == BUFFER_SIZE{
                self.push_audio_to_device(&self.buffer).unwrap();
                self.buffer_index = 0;
            }
        }
    }
}