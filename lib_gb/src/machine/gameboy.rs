use crate::{
    apu::{audio_device::AudioDevice, gb_apu::GbApu},
    cpu::gb_cpu::GbCpu,
    mmu::{carts::mbc::Mbc, gb_mmu::{GbMmu, BOOT_ROM_SIZE}, memory::Memory}, 
    ppu::gfx_device::GfxDevice, keypad::joypad_provider::JoypadProvider
};
use std::boxed::Box;

//CPU frequrncy: 4,194,304 / 59.727~ / 4 == 70224 / 4
pub const CYCLES_PER_FRAME:u32 = 17556;

pub struct GameBoy<'a, JP: JoypadProvider, AD:AudioDevice, GFX:GfxDevice> {
    cpu: GbCpu,
    mmu: GbMmu::<'a, AD, GFX, JP>
}

impl<'a, JP:JoypadProvider, AD:AudioDevice, GFX:GfxDevice> GameBoy<'a, JP, AD, GFX>{

    pub fn new_with_bootrom(mbc:&'a mut Box<dyn Mbc>,joypad_provider:JP, audio_device:AD, gfx_device:GFX, boot_rom:[u8;BOOT_ROM_SIZE])->GameBoy<JP, AD, GFX>{
        GameBoy{
            cpu:GbCpu::default(),
            mmu:GbMmu::new_with_bootrom(mbc, boot_rom, GbApu::new(audio_device), gfx_device, joypad_provider),
        }
    }

    pub fn new(mbc:&'a mut Box<dyn Mbc>,joypad_provider:JP, audio_device:AD, gfx_device:GFX)->GameBoy<JP, AD, GFX>{
        let mut cpu = GbCpu::default();
        //Values after the bootrom
        *cpu.af.value() = 0x190;
        *cpu.bc.value() = 0x13;
        *cpu.de.value() = 0xD8;
        *cpu.hl.value() = 0x14D;
        cpu.stack_pointer = 0xFFFE;
        cpu.program_counter = 0x100;

        GameBoy{
            cpu:cpu,
            mmu:GbMmu::new(mbc, GbApu::new(audio_device), gfx_device, joypad_provider)
        }
    }

    pub fn cycle_frame(&mut self){
        while self.mmu.m_cycle_counter < CYCLES_PER_FRAME{
            self.mmu.poll_joypad_state();

            //CPU
            let mut cpu_cycles_passed = 1;
            if !self.cpu.halt{
                cpu_cycles_passed = self.execute_opcode();
            }
            if cpu_cycles_passed != 0{
                self.mmu.cycle(cpu_cycles_passed);
            }
            
            //interrupts
            let interrupt_request = self.mmu.handle_interrupts(self.cpu.mie);
            let interrupt_cycles = self.cpu.execute_interrupt_request(&mut self.mmu, interrupt_request);
            if interrupt_cycles != 0{
                self.mmu.cycle(interrupt_cycles);
            }
        }

        self.mmu.m_cycle_counter = 0;
    }

    fn execute_opcode(&mut self)->u8{
        let pc = self.cpu.program_counter;

        log::trace!("A: {:02X} F: {:02X} B: {:02X} C: {:02X} D: {:02X} E: {:02X} H: {:02X} L: {:02X} SP: {:04X} PC: 00:{:04X} ({:02X} {:02X} {:02X} {:02X})",
            {*self.cpu.af.high()}, *self.cpu.af.low(),
            {*self.cpu.bc.high()}, *self.cpu.bc.low(),
            {*self.cpu.de.high()}, *self.cpu.de.low(),
            {*self.cpu.hl.high()}, *self.cpu.hl.low(),
            self.cpu.stack_pointer, pc,
            self.mmu.read(pc,0), self.mmu.read(pc+1,0), self.mmu.read(pc+2,0), self.mmu.read(pc+3,0)
        );
    
        self.cpu.run_opcode(&mut self.mmu)
    }
}

