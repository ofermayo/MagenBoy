use crate::{mmu::vram::VRam, ppu::attributes::SpriteAttributes, utils::{self, bit_masks::{BIT_0_MASK, BIT_2_MASK}, fixed_size_queue::FixedSizeQueue}};
use super::{FIFO_SIZE, SPRITE_WIDTH, fetcher_state_machine::FetcherStateMachine, fetching_state::*};

pub const NORMAL_SPRITE_HIGHT:u8 = 8;
pub const EXTENDED_SPRITE_HIGHT:u8 = 16;
pub const MAX_SPRITES_PER_LINE:usize = 10;

pub struct SpriteFetcher{
    pub fifo:FixedSizeQueue<(u8, u8), FIFO_SIZE>,
    pub oam_entries:[SpriteAttributes; 10],
    pub oam_entries_len:u8,
    pub rendering:bool,

    fetcher_state_machine:FetcherStateMachine,
    current_oam_entry:u8,
    cgb_mode:bool
}

impl SpriteFetcher{
    pub fn new(cgb_mode:bool)->Self{
        let oam_entries:[SpriteAttributes; MAX_SPRITES_PER_LINE] = utils::create_array(|| SpriteAttributes::new_gb(0,0,0,0));
        let state_machine:[FetchingState;8] = [FetchingState::FetchTileNumber, FetchingState::Sleep, FetchingState::Sleep, FetchingState::FetchLowTile, FetchingState::Sleep, FetchingState::FetchHighTile, FetchingState::Sleep, FetchingState::Push];
        
        SpriteFetcher{
            fetcher_state_machine:FetcherStateMachine::new(state_machine),
            current_oam_entry:0,
            oam_entries_len:0,
            oam_entries,
            fifo:FixedSizeQueue::<(u8,u8), 8>::new(),
            rendering:false,
            cgb_mode
        }
    }

    pub fn reset(&mut self){
        self.current_oam_entry = 0;
        self.oam_entries_len = 0;
        self.fetcher_state_machine.reset();
        self.fifo.clear();
        self.rendering = false;
    }

    pub fn fetch_pixels(&mut self, vram:&VRam, lcd_control:u8, ly_register:u8, current_x_pos:u8){
        let sprite_size = if lcd_control & BIT_2_MASK == 0 {NORMAL_SPRITE_HIGHT} else{EXTENDED_SPRITE_HIGHT};

        match self.fetcher_state_machine.current_state(){
            FetchingState::FetchTileNumber=>{
                self.try_fetch_tile_number(current_x_pos, lcd_control);
            }
            FetchingState::FetchLowTile=>{
                let tile_num = self.fetcher_state_machine.data.tile_data;
                let oam_attribute = &self.oam_entries[self.current_oam_entry as usize];
                let current_tile_data_address = Self::get_current_tile_data_address(ly_register, oam_attribute, sprite_size, tile_num);
                let low_data = vram.read_bank(current_tile_data_address,oam_attribute.attribute.bank as u8);
                self.fetcher_state_machine.data.low_tile_data = low_data;
                self.fetcher_state_machine.advance();
            }
            FetchingState::FetchHighTile=>{
                let tile_num= self.fetcher_state_machine.data.tile_data;
                let oam_attribute = &self.oam_entries[self.current_oam_entry as usize];
                let current_tile_data_address = Self::get_current_tile_data_address(ly_register, oam_attribute, sprite_size, tile_num);
                let high_data = vram.read_bank(current_tile_data_address + 1, oam_attribute.attribute.bank as u8);
                self.fetcher_state_machine.data.high_tile_data = high_data;
                self.fetcher_state_machine.advance();
            }
            FetchingState::Push=>{
                let low_data = self.fetcher_state_machine.data.low_tile_data;
                let high_data = self.fetcher_state_machine.data.high_tile_data;
                let oam_attribute = &self.oam_entries[self.current_oam_entry as usize];
                let start_x = self.fifo.len();
                let skip_x = 8 - (oam_attribute.x - current_x_pos) as usize;

                if oam_attribute.attribute.flip_x{
                    for i in (0 + skip_x)..SPRITE_WIDTH as usize{
                        if self.is_sprite_lose_priority(current_x_pos, i, oam_attribute) {
                            break;
                        }
                        let pixel = Self::get_decoded_pixel(i, low_data, high_data);
                        if i >= start_x {
                            self.fifo.push((pixel, self.current_oam_entry));
                        }
                        else if self.fifo[i].0 == 0{
                            self.fifo[i] = (pixel, self.current_oam_entry);
                        }
                    }
                }
                else{
                    let fifo_max_index = FIFO_SIZE as usize - 1;
                    let number_of_pixels_to_push = SPRITE_WIDTH as usize - skip_x;
                    for i in (0..number_of_pixels_to_push).rev(){
                        if self.is_sprite_lose_priority(current_x_pos, number_of_pixels_to_push - i - 1, oam_attribute) {
                            break;
                        }
                        let pixel = Self::get_decoded_pixel(i, low_data, high_data);
                        if fifo_max_index - skip_x - i >= start_x {
                            self.fifo.push((pixel, self.current_oam_entry));
                        }
                        else if self.fifo[fifo_max_index - skip_x - i].0 == 0{
                            self.fifo[fifo_max_index - skip_x - i] = (pixel, self.current_oam_entry);
                        }
                    }
                }

                self.current_oam_entry += 1;
                self.fetcher_state_machine.advance();
            }
            FetchingState::Sleep=>self.fetcher_state_machine.advance()
        }
    }

    fn is_sprite_lose_priority(&self, current_x_pos: u8, i: usize, oam_attribute: &SpriteAttributes)->bool {
        return if self.cgb_mode && self.oam_entries_len > self.current_oam_entry + 1 {
            let next_oam_attributes = &self.oam_entries[self.current_oam_entry as usize + 1];
            next_oam_attributes.x - SPRITE_WIDTH == current_x_pos + i as u8 && next_oam_attributes.oam_index < oam_attribute.oam_index
        }
        else{
            false
        };
    }

    //This is a function on order to abort if rendering
    fn try_fetch_tile_number(&mut self, current_x_pos: u8, lcd_control: u8) {
        if self.oam_entries_len > self.current_oam_entry{
            let oam_entry = &self.oam_entries[self.current_oam_entry as usize];
            if oam_entry.x <= current_x_pos + SPRITE_WIDTH && current_x_pos < oam_entry.x{
                let mut tile_number = oam_entry.tile_number;
                if lcd_control & BIT_2_MASK != 0{
                    tile_number &= !BIT_0_MASK
                }
                self.rendering = true;
                self.fetcher_state_machine.data.reset();
                self.fetcher_state_machine.data.tile_data = tile_number;
                self.fetcher_state_machine.advance();
                return;
            }
        }
        self.rendering = false;
    }

    // Receiving the tile_num since in case of extended sprite this could change (the first bit is reset)
    fn get_current_tile_data_address(ly_register:u8, sprite_attrib:&SpriteAttributes, sprite_size:u8, tile_num:u8)->u16{
        return if sprite_attrib.attribute.flip_y{
            // Since Im flipping but dont know for what rect (8X8 or 8X16) I need sub this from the size (minus 1 casue im starting to count from 0 in the screen lines).
            tile_num as u16 * 16 + (2 * (sprite_size - 1 - (16 - (sprite_attrib.y - ly_register)))) as u16
        }
        else{
            // Since the sprite attribute y pos is the right most dot of the rect 
            // Im subtracting this from 16 (since the rects are 8X16)
            tile_num as u16 * 16 + (2 * (16 - (sprite_attrib.y - ly_register))) as u16
        };
    }

    fn get_decoded_pixel(index: usize, low_data: u8, high_data: u8) -> u8 {
        let mask = 1 << index;
        let mut pixel = (low_data & mask) >> index;
        pixel |= ((high_data & mask) >> index) << 1;
        pixel
    }
}