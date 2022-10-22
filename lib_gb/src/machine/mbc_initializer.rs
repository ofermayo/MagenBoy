use crate::mmu::carts::*;

const CARTRIDGE_TYPE_ADDRESS:usize = 0x147;
const CGB_FLAG_ADDRESS:usize = 0x143;

pub fn initialize_mbc(program:Vec<u8>, save_data:Option<Vec<u8>>)->Box<dyn Mbc>{
    let mbc_type = program[CARTRIDGE_TYPE_ADDRESS];
    log::info!("initializing cartridge of type: {:#X}", mbc_type);
    log::info!("CGB flag: {:#X}", program[CGB_FLAG_ADDRESS]);

    match mbc_type{
        0x0|0x8=>Box::new(Rom::new(program,false, None)),
        0x9=>Box::new(Rom::new(program, true, save_data)),
        0x1|0x2=>Box::new(Mbc1::new(program,false, None)),
        0x3=>Box::new(Mbc1::new(program,true, save_data)),
        0x11|0x12=>Box::new(Mbc3::new(program,false,Option::None)),
        0x13 | 0x1B=>Box::new(Mbc3::new(program, true, save_data)),
        _=>std::panic!("not supported cartridge: {:#X}",mbc_type)
    }
}

impl dyn Mbc{
    pub fn is_cgb_mode(&self)->bool{
        return match self.read_bank0(CGB_FLAG_ADDRESS as u16){
            0x80 | 0xC0=>true,
            _=>false
        };
    }
}