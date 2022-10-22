use crate::utils::bit_masks::*;

#[derive(Clone, Copy)]
pub enum Pallete{
    GbPallete(bool),
    GbcPallete(u8)
}

#[derive(Clone, Copy)]
pub struct Attributes{
    pub priority:bool,
    pub flip_y:bool,
    pub flip_x:bool,
    pub bank:bool,
}

impl Attributes{
    pub fn new_gb(attribute:u8)->Self{
        Self{
            priority: (attribute & BIT_7_MASK) != 0,
            flip_y: (attribute & BIT_6_MASK) != 0,
            flip_x: (attribute & BIT_5_MASK) != 0,
            bank: false,
        }
    }
    
    pub fn new_gbc(attribute:u8)->Self{
        Self{
            priority: (attribute & BIT_7_MASK) != 0,
            flip_y: (attribute & BIT_6_MASK) != 0,
            flip_x: (attribute & BIT_5_MASK) != 0,
            bank:(attribute & BIT_3_MASK) != 0,
        }
    }
}

#[derive(Clone, Copy)]
pub struct BackgroundAttributes{
    pub attribute:Attributes,
    pub cgb_pallete_number:u8,
}

impl BackgroundAttributes{
    pub fn new(attribute:u8)->Self{
        Self{
            attribute: Attributes::new_gbc(attribute),
            cgb_pallete_number: attribute & 0b111
        }
    }
}

pub struct SpriteAttributes{
    pub y:u8,
    pub x:u8,
    pub tile_number:u8,
    pub palette_number:Pallete,
    pub attribute:Attributes,
    pub oam_index:u8
}

impl SpriteAttributes{
    pub fn new_gb(y:u8, x:u8, tile_number:u8, attributes:u8)->Self{
        Self::new(y,x,tile_number, Attributes::new_gb(attributes), Pallete::GbPallete((attributes & BIT_4_MASK) != 0), 0)
    }

    pub fn new_gbc(y:u8, x:u8, tile_number:u8, attributes:u8, oam_index:u8)->Self{
        Self::new(y,x,tile_number, Attributes::new_gbc(attributes),Pallete::GbcPallete(attributes & 0b111),oam_index)
    }

    fn new(y:u8, x:u8, tile_number:u8, attribute:Attributes, palette_number:Pallete, oam_index:u8)->Self{
        SpriteAttributes{y, x, tile_number, attribute, palette_number, oam_index}
    }
}