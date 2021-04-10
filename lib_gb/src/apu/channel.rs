use super::{freq_sweep::FreqSweep, sample_producer::SampleProducer, tone_sweep_sample_producer::ToneSweepSampleProducer};
use super::timer::Timer;

pub struct Channel<Procuder: SampleProducer>{
    pub enabled:bool,
    pub frequency:u16,
    pub sound_length:u16,
    pub volume:u8,
    pub current_volume:u8,
    pub length_enable:bool,
    pub sample_producer:Procuder,
    pub timer:Timer,

    last_sample:i8,
}

impl<Procuder: SampleProducer> Channel<Procuder>{
    pub fn new()->Self{
        Channel{
            enabled:false,
            frequency:0,
            sound_length:0,
            volume:0,
            current_volume:0,
            length_enable:false,
            sample_producer:Procuder::default(),
            timer: Timer::new(Procuder::get_updated_frequency_ticks(0)),

            last_sample: 0
        }   
    }

    pub fn update_length_register(&mut self){
        if self.length_enable{
            if self.sound_length > 0{
                self.sound_length -= 1;
            }
            if self.sound_length == 0{
                self.enabled = false;
            }
        }
    }

    pub fn reset(&mut self){
        self.enabled = false;
        self.frequency = 0;
        self.length_enable = false;
        self.sound_length = 0;
        self.timer.update_cycles_to_tick(Procuder::get_updated_frequency_ticks(self.frequency));
        self.volume = 0;
        self.current_volume = 0;

        self.last_sample = 0;
    }

    pub fn get_audio_sample(&mut self)->f32{
        if self.enabled{

            let sample = if self.timer.cycle(){
                self.timer.update_cycles_to_tick(Procuder::get_updated_frequency_ticks(self.frequency));
                self.sample_producer.produce()
            }
            else{
                self.last_sample
            };

            self.last_sample = sample;
    
            return self.convert_digtial_to_analog(self.last_sample);
        }
        else{
            //self.last_sample = 0;
        }
        
        return 0.0;
    }

    fn convert_digtial_to_analog(&self, sample:i8)->f32{
        ((sample * self.current_volume as i8) as f32 / 7.5 ) - 1.0
        //(sample * self.current_volume as i8) as f32 / 15.0
    }
}

pub fn update_sweep_frequency(channel:&mut Channel<ToneSweepSampleProducer>){
    let sweep:&mut FreqSweep = &mut channel.sample_producer.sweep;
    if sweep.enabled && sweep.sweep_period != 0{
        //calculate a new freq
        let mut new_freq = sweep.calculate_new_frequency();
        if FreqSweep::check_overflow(new_freq){
            channel.enabled = false;
        }

        //load shadow and freq register with new value
        if new_freq <= 2047 && sweep.sweep_shift > 0{
            sweep.shadow_frequency = new_freq;
            channel.frequency = new_freq;

            //Another overflow check
            new_freq = sweep.calculate_new_frequency();
            if FreqSweep::check_overflow(new_freq){
                channel.enabled = false;
            }
        }
    }
}