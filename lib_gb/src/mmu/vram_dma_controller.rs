use crate::{utils::bit_masks::BIT_7_MASK, ppu::{gb_ppu::GbPpu, gfx_device::GfxDevice, ppu_state::PpuState}};

use super::external_memory_bus::ExternalMemoryBus;

enum TransferMode{
    GeneralPurpose,
    Hblank,
    Terminated
}

const TRANSFER_CHUNK_SIZE:u8 = 0x10;

pub struct VramDmaController{
    source_address:u16,
    dest_address:u16,
    mode:TransferMode,
    remaining_length:u8,

    last_ly:u8,
    m_cycle_counter:u32
}

impl VramDmaController{
    pub fn set_source_high(&mut self, value:u8){
        self.source_address = (self.source_address & 0x00FF) | (value as u16) << 8;
    }
    
    pub fn set_source_low(&mut self, value:u8){
        // Ignores the last 4 bits of the source
        let value = value & 0xF0;
        self.source_address = (self.source_address & 0xFF00) | value as u16;
    }

    pub fn set_dest_high(&mut self, value:u8){
        // Upper 3 bits are ignored since the dest are always in the vram
        let value = value & 0b0001_1111;
        self.dest_address = (self.dest_address & 0x00FF) | (value as u16) << 8;
    }
    
    pub fn set_dest_low(&mut self, value:u8){
        // Ignores the last 4 bits of the dest
        let value = value & 0xF0;
        self.dest_address = (self.dest_address & 0xFF00) | value as u16;
    }

    pub fn set_mode_length(&mut self, value:u8){
        match self.mode{
            TransferMode::Hblank |
            TransferMode::GeneralPurpose=>self.mode = TransferMode::Terminated,
            TransferMode::Terminated=>{
                self.mode = if (value & BIT_7_MASK) == 0{TransferMode::GeneralPurpose}else{TransferMode::Hblank};
                self.remaining_length = value & !BIT_7_MASK;
            }
        }
    }

    pub fn get_mode_length(&self)->u8{
        self.remaining_length.wrapping_sub(1)
    }

    pub fn cycle<G:GfxDevice>(&mut self, m_cycles:u32, exteranl_memory_bus:&mut ExternalMemoryBus, ppu:&mut GbPpu<G>){
        match self.mode{
            TransferMode::Hblank=>self.handle_hblank_transfer(ppu, m_cycles, exteranl_memory_bus),
            TransferMode::GeneralPurpose=>self.handle_general_purpose_transfer(exteranl_memory_bus, ppu),
            TransferMode::Terminated=>{}
        }
    }

    fn handle_general_purpose_transfer<G:GfxDevice>(&mut self, exteranl_memory_bus: &mut ExternalMemoryBus, ppu: &mut GbPpu<G>) {
        while self.remaining_length != 0{
            for _ in 0..TRANSFER_CHUNK_SIZE{
                let source_value = exteranl_memory_bus.read(self.source_address);
                ppu.vram.write_current_bank(self.dest_address, source_value);

                self.source_address += 1;
                self.dest_address += 1;
            }

            self.remaining_length -= 1;
        }
    }

    fn handle_hblank_transfer<G:GfxDevice>(&mut self, ppu: &mut GbPpu<G>, m_cycles: u32, exteranl_memory_bus: &mut ExternalMemoryBus) {
        if ppu.ly_register != self.last_ly && ppu.state as u8 == PpuState::Hblank as u8{
            while self.m_cycle_counter < m_cycles && self.m_cycle_counter < TRANSFER_CHUNK_SIZE as u32{
                let source_value = exteranl_memory_bus.read(self.source_address);
                ppu.vram.write_current_bank(self.dest_address, source_value);

                self.source_address += 1;
                self.dest_address += 1;
                self.m_cycle_counter += 1;
            }

            if self.m_cycle_counter == TRANSFER_CHUNK_SIZE as u32{
                self.m_cycle_counter = 0;
                self.last_ly = ppu.ly_register;
                self.remaining_length -= 1;
                if self.remaining_length == 0{
                    self.mode = TransferMode::Terminated;
                }
            }
        }
    }
}