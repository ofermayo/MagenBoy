use crate::mmu::carts::*;

pub fn initialize_mbc(mbc_type:u8, program:Vec<u8>, save_data:Option<Vec<u8>>)->Box<dyn Mbc>{
    match mbc_type{
        0x0|0x8=>Box::new(Rom::new(program,false, None)),
        0x9=>Box::new(Rom::new(program, true, save_data)),
        0x1|0x2=>Box::new(Mbc1::new(program,false, None)),
        0x3=>Box::new(Mbc1::new(program,true, save_data)),
        0x11|0x12=>Box::new(Mbc3::new(program,false,Option::None)),
        0x13=>Box::new(Mbc3::new(program, true, save_data)),
        _=>std::panic!("not supported cartridge: {}",mbc_type)
    }
}