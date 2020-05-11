use crate::cpu::gbc_cpu::{GbcCpu};



pub fn get_src_register(cpu: &mut GbcCpu, opcode:u8)-> &mut u8{
    let reg_num = opcode & 0b111;
    return match reg_num{
        0x0=>cpu.bc.high(),
        0x1=>cpu.bc.low(),
        0x2=>cpu.de.high(),
        0x3=>cpu.de.low(),
        0x4=>cpu.hl.high(),
        0x5=>cpu.hl.low(),
        0x7=>cpu.af.high(),
        _=>panic!("no register: {}",reg_num)
    };
}

pub fn get_arithmetic_16reg(cpu:&mut GbcCpu, reg:u8)->&mut u16{
    return match reg{
        0x0=>&mut cpu.bc.value,
        0x1=>&mut cpu.de.value,
        0x2=>&mut cpu.hl.value,
        0x3=>&mut cpu.stack_pointer,
        _=>panic!("no register")
    };
}

pub fn check_for_half_carry_third_nible(a:u16, b:u16)->bool{
    ((a & 0xFFF) + (b & 0xFFF)) & 0x1000 == 0x1000
}

pub fn check_for_half_carry_first_nible_add(a:u8, b:u8)->bool{
    ((a & 0xF) + (b & 0xF)) & 0x10 == 0x10
}

pub fn check_for_half_carry_first_nible_sub(a:u8, b:u8)->bool{
    let sa = a as i16;
    let sb = b as i16;
    ((sa & 0xF) - (sb & 0xF)) < 0
}

pub fn get_cb_opcode(cb_opcode:u16)->u8{
    (cb_opcode & 0xFF) as u8
}

pub fn get_reg_two_rows(cpu: &mut GbcCpu, mut reg:u8)->&mut u8{
    reg &= 0b11111000;
    reg >>= 3;
    match reg{
        0b00=>cpu.bc.high(),
        0b01=>cpu.bc.low(),
        0b10=>cpu.de.high(),
        0b11=>cpu.de.low(),
        0b100=>cpu.hl.high(),
        0b101=>cpu.hl.low(),
        0b111=>cpu.af.high(),
        _=>panic!("no register")
    }
}