use std::{ffi::c_void, mem::MaybeUninit, str::FromStr};
use lib_gb::{GB_FREQUENCY, apu::audio_device::{AudioDevice, BUFFER_SIZE, DEFAULT_SAPMPLE, Sample, StereoSample}};
use sdl2::sys::*;
use super::{AudioResampler, ResampledAudioDevice, get_sdl_error_message};

//After twicking those numbers Iv reached this, this will affect fps which will affect sound tearing
const BYTES_TO_WAIT:u32 = BUFFER_SIZE as u32 * 16;

pub struct SdlPushAudioDevice<AR:AudioResampler>{
    device_id: SDL_AudioDeviceID,
    resampler: AR,

    buffer: [Sample;BUFFER_SIZE],
    buffer_index:usize,
}

impl<AR:AudioResampler> SdlPushAudioDevice<AR>{
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

impl<AR:AudioResampler> ResampledAudioDevice<AR> for SdlPushAudioDevice<AR>{
    fn new(frequency:i32, turbo_mul:u8)->Self{
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

    fn get_audio_buffer(&mut self) ->(&mut [Sample;BUFFER_SIZE], &mut usize) {
        (&mut self.buffer, &mut self.buffer_index)
    }

    fn get_resampler(&mut self) ->&mut AR {
        &mut self.resampler
    }

    fn full_buffer_callback(&self)->Result<(), String> {
        self.push_audio_to_device(&self.buffer).map_err(|e|String::from_str(e).unwrap())
    }
}

impl<AR:AudioResampler> AudioDevice for SdlPushAudioDevice<AR>{
    fn push_buffer(&mut self, buffer:&[StereoSample; BUFFER_SIZE]) {
        ResampledAudioDevice::push_buffer(self, buffer);
    }
}