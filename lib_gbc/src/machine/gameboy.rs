use crate::cpu::gbc_cpu::GbcCpu;
use crate::mmu::memory::Memory;
use crate::mmu::gbc_mmu::{
    GbcMmu,
    BOOT_ROM_SIZE
};
use crate::opcodes::opcode_resolver::*;
use crate::ppu::gbc_ppu::GbcPpu;
use crate::machine::registers_handler::update_registers_state;
use crate::mmu::mbc::Mbc;
use std::vec::Vec;
use std::boxed::Box;

pub struct GameBoy {
    cpu: GbcCpu,
    mmu: GbcMmu,
    opcode_resolver:OpcodeResolver,
    ppu:GbcPpu,
    cycles_per_frame:u32
}



impl GameBoy{

    pub fn new(mbc:Box<dyn Mbc>, boot_rom:[u8;BOOT_ROM_SIZE],cycles:u32)->GameBoy{
        GameBoy{
            cpu:GbcCpu::default(),
            mmu:GbcMmu::new(mbc, boot_rom),
            opcode_resolver:OpcodeResolver::default(),
            ppu:GbcPpu::default(),
            cycles_per_frame:cycles
        }
    }

    pub fn cycle_frame(&mut self)->Vec<u32>{
        for i in 0..self.cycles_per_frame{
            self.execute_opcode();
            update_registers_state(&mut self.mmu, &mut self.cpu, &mut self.ppu, i);
        }

        return self.ppu.get_gb_screen(&mut self.mmu);
    }

    fn fetch_next_byte(&mut self)->u8{
        let byte:u8 = self.mmu.read(self.cpu.program_counter);
        self.cpu.program_counter+=1;
        return byte;
    }

    fn execute_opcode(&mut self){
        let pc = self.cpu.program_counter;
        let opcode:u8 = self.fetch_next_byte();
        println!("handling opcode: {:#X?} at address {:#X?}", opcode, pc);
        //println!("{:#X?}", self.cpu.af.low());
        let opcode_func:OpcodeFuncType = self.opcode_resolver.get_opcode(opcode, &self.mmu, self.cpu.program_counter);
        match opcode_func{
            OpcodeFuncType::OpcodeFunc(func)=>func(&mut self.cpu),
            OpcodeFuncType::MemoryOpcodeFunc(func)=>func(&mut self.cpu, &mut self.mmu),
            OpcodeFuncType::U8OpcodeFunc(func)=>func(&mut self.cpu, opcode),
            OpcodeFuncType::U8MemoryOpcodeFunc(func)=>func(&mut self.cpu, &mut self.mmu, opcode),
            OpcodeFuncType::MemoryOpcodeFunc2Bytes(func)=>func(&mut self.cpu, &mut self.mmu),
            OpcodeFuncType::U16OpcodeFunc(func)=>{
                let u16_opcode:u16 = ((opcode as u16)<<8) | (self.fetch_next_byte() as u16);
                func(&mut self.cpu, u16_opcode);
            },
            OpcodeFuncType::U16MemoryOpcodeFunc(func)=>{
                let u16_opcode:u16 = ((opcode as u16)<<8) | (self.fetch_next_byte() as u16);
                func(&mut self.cpu, &mut self.mmu, u16_opcode);
            },
            OpcodeFuncType::U32OpcodeFunc(func)=>{
                let mut u32_opcode:u32 = ((opcode as u32)<<8) | (self.fetch_next_byte() as u32);
                u32_opcode <<= 8;
                u32_opcode |= self.fetch_next_byte() as u32;
                func(&mut self.cpu, u32_opcode);
            },
            OpcodeFuncType::U32MemoryOpcodeFunc(func)=>{
                let mut u32_opcode:u32 = ((opcode as u32)<<8) | (self.fetch_next_byte() as u32);
                u32_opcode <<= 8;
                u32_opcode |= self.fetch_next_byte() as u32;
                func(&mut self.cpu, &mut self.mmu, u32_opcode);
            }
        }
    }
}

