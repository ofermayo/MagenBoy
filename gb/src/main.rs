mod mbc_handler;
mod sdl_joypad_provider;
mod sdl_gfx_device;
mod mpmc_gfx_device;
mod audio;

use crate::{audio::{ChosenResampler, multi_device_audio::*, ResampledAudioDevice}, mbc_handler::*, mpmc_gfx_device::MpmcGfxDevice, sdl_joypad_provider::*};
use lib_gb::{GB_FREQUENCY, apu::audio_device::*, keypad::button::Button, machine::gameboy::GameBoy, mmu::gb_mmu::BOOT_ROM_SIZE, ppu::{gb_ppu::{BUFFERS_NUMBER, SCREEN_HEIGHT, SCREEN_WIDTH}, gfx_device::GfxDevice}};
use std::{fs, env, result::Result, vec::Vec};
use log::info;
use sdl2::sys::*;

const SCREEN_SCALE:u8 = 4;
const TURBO_MUL:u8 = 1;

fn init_logger(debug:bool)->Result<(), fern::InitError>{
    let level = if debug {log::LevelFilter::Debug} else {log::LevelFilter::Info};
    let mut fern_logger = fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}] {}",
                chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                record.level(),
                message
            ))
        })
        .level(level);

    if !debug{
        fern_logger = fern_logger.chain(std::io::stdout());
    }
    else{
        fern_logger = fern_logger.chain(fern::log_file("output.log")?);
    }

    fern_logger.apply()?;

    Ok(())
}

fn buttons_mapper(button:Button)->SDL_Scancode{
    match button{
        Button::A       => SDL_Scancode::SDL_SCANCODE_X,
        Button::B       => SDL_Scancode::SDL_SCANCODE_Z,
        Button::Start   => SDL_Scancode::SDL_SCANCODE_S,
        Button::Select  => SDL_Scancode::SDL_SCANCODE_A,
        Button::Up      => SDL_Scancode::SDL_SCANCODE_UP,
        Button::Down    => SDL_Scancode::SDL_SCANCODE_DOWN,
        Button::Right   => SDL_Scancode::SDL_SCANCODE_RIGHT,
        Button::Left    => SDL_Scancode::SDL_SCANCODE_LEFT
    }
}

fn check_for_terminal_feature_flag(args:&Vec::<String>, flag:&str)->bool{
    args.len() >= 3 && args.contains(&String::from(flag))
}

fn main() {
    let args: Vec<String> = env::args().collect();    

    let debug_level = check_for_terminal_feature_flag(&args, "--log");
    
    match init_logger(debug_level){
        Result::Ok(())=>{},
        Result::Err(error)=>std::panic!("error initing logger: {}", error)
    }

    let mut sdl_gfx_device = sdl_gfx_device::SdlGfxDevice::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32,
         "MagenBoy", SCREEN_SCALE, TURBO_MUL, check_for_terminal_feature_flag(&args, "--no-vsync"));
    
    let (s,r) = crossbeam_channel::bounded(BUFFERS_NUMBER - 1);
    let mpmc_device = MpmcGfxDevice::new(s);

    let program_name = args[1].clone();

    let mut running = true;
    // Casting to ptr cause you cant pass a raw ptr (*const/mut T) to another thread
    let running_ptr:usize = (&running as *const bool) as usize;
    
    let emualation_thread = std::thread::Builder::new().name("Emualtion Thread".to_string()).spawn(
        move || emulation_thread_main(args, program_name, mpmc_device, running_ptr)
    ).unwrap();

    unsafe{
        let mut event: std::mem::MaybeUninit<SDL_Event> = std::mem::MaybeUninit::uninit();
        loop{

            if SDL_PollEvent(event.as_mut_ptr()) != 0{
                let event: SDL_Event = event.assume_init();
                if event.type_ == SDL_EventType::SDL_QUIT as u32{
                    break;
                }
            }
            
            let buffer = r.recv().unwrap();
            sdl_gfx_device.swap_buffer(&*(buffer as *const [u32; SCREEN_WIDTH * SCREEN_HEIGHT]));
        }

        drop(r);
        std::ptr::write_volatile(&mut running as *mut bool, false);
        emualation_thread.join().unwrap();

        SDL_Quit();
    }
}

// Receiving usize and not raw ptr cause in rust you cant pass a raw ptr to another thread
fn emulation_thread_main(args: Vec<String>, program_name: String, spsc_gfx_device: MpmcGfxDevice, running_ptr: usize) {
    let audio_device = audio::ChosenAudioDevice::<ChosenResampler>::new(44100, TURBO_MUL);
    
    let mut devices: Vec::<Box::<dyn AudioDevice>> = Vec::new();
    devices.push(Box::new(audio_device));
    if check_for_terminal_feature_flag(&args, "--file-audio"){
        let wav_ad = audio::wav_file_audio_device::WavfileAudioDevice::<ChosenResampler>::new(44100, GB_FREQUENCY, "output.wav");
        devices.push(Box::new(wav_ad));
        log::info!("Writing audio to file: output.wav");
    }
    let audio_devices = MultiAudioDevice::new(devices);
    let mut mbc = initialize_mbc(&program_name);
    let joypad_provider = SdlJoypadProvider::new(buttons_mapper);
    let bootrom_path = if check_for_terminal_feature_flag(&args, "--bootrom"){
        let index = args.iter().position(|v| *v == String::from("--bootrom")).unwrap();
        args.get(index + 1).expect("Error! you must specify a value for the --bootrom parameter").clone()
    }else{
        String::from("dmg_boot.bin")
    };

    let mut gameboy = match fs::read(bootrom_path){
        Result::Ok(file)=>{
            info!("found bootrom!");
    
            let mut bootrom:[u8;BOOT_ROM_SIZE] = [0;BOOT_ROM_SIZE];
            for i in 0..BOOT_ROM_SIZE{
                bootrom[i] = file[i];
            }
        
            GameBoy::new_with_bootrom(&mut mbc, joypad_provider,audio_devices, spsc_gfx_device, bootrom)
        }
        Result::Err(_)=>{
            info!("could not find bootrom... booting directly to rom");
    
            GameBoy::new(&mut mbc, joypad_provider, audio_devices, spsc_gfx_device)
        }
    };
    info!("initialized gameboy successfully!");

    unsafe{
        while std::ptr::read_volatile(running_ptr as *const bool){
            gameboy.cycle_frame();
        }
    }
    drop(gameboy);
    release_mbc(&program_name, mbc);
    log::info!("released the gameboy succefully");
}