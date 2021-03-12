use super::channel::Channel;
use super::wave_sample_producer::WaveSampleProducer;
use super::tone_sweep_sample_producer::ToneSweepSampleProducer;
use super::audio_device::AudioDevice;
use super::sound_terminal::SoundTerminal;
use super::frame_sequencer::{
    FrameSequencer,
    TickType,
};
use crate::{mmu::memory::UnprotectedMemory, utils::memory_registers::{NR10_REGISTER_ADDRESS, NR52_REGISTER_ADDRESS}};

pub const AUDIO_BUFFER_SIZE:usize = 0x400;

pub struct GbApu<Device: AudioDevice>{
    pub wave_channel:Channel<WaveSampleProducer>,
    pub sweep_tone_channel:Channel<ToneSweepSampleProducer>,

    frame_sequencer: FrameSequencer,
    audio_buffer:[f32;AUDIO_BUFFER_SIZE],
    current_t_cycle:u32,
    device:Device,
    pub terminal1:SoundTerminal,
    pub terminal2:SoundTerminal,
    pub enabled:bool,

    last_enabled_state:bool
}

impl<Device: AudioDevice> GbApu<Device>{
    pub fn new(device: Device) -> Self {
        GbApu{
            frame_sequencer:FrameSequencer::default(),
            sweep_tone_channel: Channel::<ToneSweepSampleProducer>::new(),
            wave_channel:Channel::<WaveSampleProducer>::new(),
            audio_buffer:[0.0; AUDIO_BUFFER_SIZE],
            current_t_cycle:0,
            device:device,
            terminal1: SoundTerminal::default(),
            terminal2: SoundTerminal::default(),
            enabled:false, 
            last_enabled_state: false
        }
    }

    pub fn cycle(&mut self, memory:&mut impl UnprotectedMemory, m_cycles_passed:u8){
        //converting m_cycles to t_cycles
        let t_cycles = m_cycles_passed * 4;

        if self.enabled{
            for _ in 0..t_cycles{   
                if self.current_t_cycle as usize >= AUDIO_BUFFER_SIZE{
                    self.current_t_cycle = 0;
                    self.device.push_buffer(&self.audio_buffer);
                }

                let tick = self.frame_sequencer.cycle();
                self.update_channels_for_frame_squencer(tick);
            
                let sample = self.sweep_tone_channel.get_audio_sample();
            
                self.audio_buffer[self.current_t_cycle as usize] = sample;
                
                self.current_t_cycle += 1;
            }

            self.update_registers(memory);
        }
        else{
            self.current_t_cycle += t_cycles as u32;
            for i in NR10_REGISTER_ADDRESS..NR52_REGISTER_ADDRESS{
                memory.write_unprotected(i, 0);
            }
        }            

        self.last_enabled_state = self.enabled;
    }

    fn update_channels_for_frame_squencer(&mut self, tick:TickType){
        if tick.frequency_sweep{
            if self.sweep_tone_channel.enabled{
                let sweep = &mut self.sweep_tone_channel.sample_producer.sweep;
                if sweep.time_sweep != 0 && sweep.sweep_shift != 0{
                    let mut shifted_freq:i32 = (sweep.shadow_frequency >> sweep.sweep_shift) as i32;

                    if sweep.sweep_decrease{
                        shifted_freq *= -1;
                    }

                    shifted_freq += sweep.shadow_frequency as i32;

                    if shifted_freq >= 2048 || shifted_freq <= 0{
                        self.sweep_tone_channel.enabled = false;
                    }
                    else{
                        sweep.time_sweep -= 1;
                        self.sweep_tone_channel.frequency = shifted_freq as u16;
                        self.sweep_tone_channel.timer.cycles_to_tick = (2048 - self.sweep_tone_channel.frequency).wrapping_mul(4);
                    }
                }
            }
        }
        if tick.length_counter{
            if self.sweep_tone_channel.length_enable && self.sweep_tone_channel.enabled{
                self.sweep_tone_channel.sound_length -= 1;
                if self.sweep_tone_channel.sound_length == 0{
                    self.sweep_tone_channel.enabled = false;
                }
            }
        }
        if tick.volume_envelope{
            if self.sweep_tone_channel.enabled{
                let envelop = &mut self.sweep_tone_channel.sample_producer.envelop;

                if envelop.number_of_envelope_sweep > 0 {
                    envelop.envelop_duration_counter += 1;
                    if envelop.envelop_duration_counter == envelop.number_of_envelope_sweep{
                        if envelop.increase_envelope{
                            let new_vol = self.sweep_tone_channel.volume + 1;
                            self.sweep_tone_channel.volume = std::cmp::min(new_vol, 0xF);
                        }
                        else{
                            let new_vol = self.sweep_tone_channel.volume as i8 - 1;
                            self.sweep_tone_channel.volume = std::cmp::max::<i8>(new_vol, 0) as u8;
                        }

                        envelop.envelop_duration_counter = 0;
                    }
                }
            }
        }
    }

    fn update_registers(&mut self, memory:&mut impl UnprotectedMemory){
        memory.write_unprotected(0xFF1B, self.wave_channel.sound_length);

        let mut control_register = memory.read_unprotected(0xFF26);
        Self::set_bit(&mut control_register, 3, self.wave_channel.enabled);
    }

    fn set_bit(value:&mut u8, bit_number:u8, set:bool){
        let mask = 1 << bit_number;
        if set{
            *value |= mask;
        }
        else{
            let inverse_mask = !mask;
            *value &= inverse_mask;
        }
    }
}

