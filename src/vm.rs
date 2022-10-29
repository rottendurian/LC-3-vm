use std::{env, io};
use std::result::Result;
use std::process::exit;
use std::fs::File;
use std::io::{Read, Write, BufReader};


pub enum Registers {
    R0 = 0,
    R1,
    R2,
    R3,
    R4,
    R5,
    R6,
    R7,
    PC, /* program counter */
    COND,
    COUNT
}
#[derive(Debug)]
pub enum Operators {
    BR = 0, /* branch */
    ADD,    /* add  */
    LD,     /* load */
    ST,     /* store */
    JSR,    /* jump register */
    AND,    /* bitwise and */
    LDR,    /* load register */
    STR,    /* store register */
    RTI,    /* unused */
    NOT,    /* bitwise not */
    LDI,    /* load indirect */
    STI,    /* store indirect */
    JMP,    /* jump */
    RES,    /* reserved (unused) */
    LEA,    /* load effective address */
    TRAP    /* execute trap */
}
impl Operators {
    fn from(val:u16) -> Result<Operators,i16> {
        match val {
            0 =>  {return Ok(Operators::BR)},
            1 =>  {return Ok(Operators::ADD)},
            2 =>  {return Ok(Operators::LD)},
            3 =>  {return Ok(Operators::ST)},
            4 =>  {return Ok(Operators::JSR)},
            5 =>  {return Ok(Operators::AND)},
            6 =>  {return Ok(Operators::LDR)},
            7 =>  {return Ok(Operators::STR)},
            8 =>  {return Ok(Operators::RTI)},
            9 =>  {return Ok(Operators::NOT)},
            10 => {return Ok(Operators::LDI)},
            11 => {return Ok(Operators::STI)},
            12 => {return Ok(Operators::JMP)},
            13 => {return Ok(Operators::RES)},
            14 => {return Ok(Operators::LEA)},
            15 => {return Ok(Operators::TRAP)},
            _ => return Err(-1)
        }   
    }
}
pub enum Flags {
    POS = 1 << 0, /* P */
    ZRO = 1 << 1, /* Z */
    NEG = 1 << 2, /* N */
}
pub enum TRAP {
    GETC = 0x20,  //not echoed to terminal
    OUT = 0x21,   //outputs a char
    PUTS = 0x22,  //outputs a word string
    IN = 0x23,    //character from keyboard, echoed onto terminal
    PUTSP = 0x24, //output a byte string
    HALT = 0x25   //halts the program
}
impl TRAP {
    fn from(val:u16) -> Result<TRAP,i16> {
        match val {
            0x20 =>  {return Ok(TRAP::GETC)},
            0x21 =>  {return Ok(TRAP::OUT)},
            0x22 =>  {return Ok(TRAP::PUTS)},
            0x23 =>  {return Ok(TRAP::IN)},
            0x24 =>  {return Ok(TRAP::PUTSP)},
            0x25 =>  {return Ok(TRAP::HALT)},
            _ => return Err(-1)
        }   
    }   
}
pub enum MR {
    KBSR = 0xFE00, //keyboard status
    KBDR = 0xFE02  //keyboard data
}

const MEM_MAX:usize = 1 << 16;


struct Vm {
    pub reg:[u16;Registers::COUNT as usize],
    pub mem:[u16;MEM_MAX]
}


impl Vm {
    pub fn new() -> Vm {
        Vm {
            reg:[0;Registers::COUNT as usize],
            mem:[0;MEM_MAX]
        }
    }
    #[inline]
    pub fn sign_extend(mut x:u16, bit_count:i32) -> u16 {
        if (x>>(bit_count-1) & 1) != 0 {
            x |= 0xFFFF << bit_count;
        }
        return x
    }
    #[inline]
    fn swap16(x:u16) -> u16 {
        (x << 8) | (x >> 8)
    }
    pub fn read_image(&mut self,file:&str)-> std::io::Result<usize> {
        let f = File::open(file).expect("Couldn't open file");

        let f = BufReader::new(f);

        let mut handle = f.take(MEM_MAX as u64*2);
        let mut origin:u16 = 0;
        {
            let origin_ptr = unsafe {std::slice::from_raw_parts_mut(&mut origin as *mut u16 as *mut u8 , 2) };
            let _result = handle.read(origin_ptr)?;

        }
        origin = Self::swap16(origin);

        if origin as usize > MEM_MAX {
            return Result::Err(std::io::Error::new(std::io::ErrorKind::Other,"Origin larger than MEM_MAX"));
        }
        
        let ptr = unsafe {std::slice::from_raw_parts_mut(&mut self.mem[origin as usize] as *mut u16 as *mut u8 , MEM_MAX as usize*2-origin as usize)};
        
        let result = handle.read(ptr).unwrap();
        let mut i = 0;
        while i < (result as f64/2.0+0.5) as usize {
            self.mem[origin as usize + i] = Self::swap16(self.mem[origin as usize + i]);
            i+=1;
        }
        

        Result::Ok(result)
    }
    #[inline]
    pub fn update_flags(&mut self,register:u16) {
        if self.reg[register as usize] == 0 {
            self.reg[Registers::COND as usize] = Flags::ZRO as u16;
        } else if self.reg[register as usize] >> 15 != 0 {
            self.reg[Registers::COND as usize] = Flags::NEG as u16;
        } else {
            self.reg[Registers::COND as usize] = Flags::POS as u16;
        }
    }
    #[inline]
    pub fn mem_write(&mut self,address:u16,val:u16) {
        self.mem[address as usize] = val;
    }
    #[inline]
    pub fn mem_read(&mut self,address:u16) -> u16 {
        if address == MR::KBSR as u16 {
            self.handle_keyboard();
        }

        self.mem[address as usize]

    }
    #[inline]
    fn handle_keyboard(&mut self) {
        let mut buffer = [0; 1];
        std::io::stdin().read_exact(&mut buffer).unwrap();
        if buffer[0] != 0 {
            self.mem[MR::KBSR as usize] = 1 << 15;
            self.mem[MR::KBDR as usize] = buffer[0] as u16;
        } else {
            self.mem[MR::KBSR as usize] = 0;
        }
    }
    #[inline]
    pub fn br(&mut self,instruction:u16) {
        let pc_offset:u16 = Vm::sign_extend(instruction & 0x1FF, 9);
        let cond_flag = (instruction >> 9) & 0x7;
        if cond_flag & self.reg[Registers::COND as usize] != 0 {
            let val:u32 = self.reg[Registers::PC as usize] as u32 + pc_offset as u32;
            self.reg[Registers::PC as usize] = val as u16;
        }
    }
    #[inline]
    pub fn add(&mut self,instruction:u16) {
        let dest:u16 = (instruction >> 9) & 0x7;
        let op1:u16 = (instruction >> 6) & 0x7;
        let imm_flag:u16 = (instruction >> 5) & 0x1;
        if imm_flag == 1 {
            let imm = Vm::sign_extend(instruction & 0x1F,5);
            self.reg[dest as usize] = (self.reg[op1 as usize] as u32 + imm as u32) as u16;
        } else {
            let op2 = instruction & 0x7;
            self.reg[dest as usize] = (self.reg[op1 as usize] as u32 + self.reg[op2 as usize] as u32) as u16;
        }
        self.update_flags(dest);
    }
    #[inline]
    pub fn ld(&mut self, instruction:u16) {
        let r0:u16 = (instruction >> 9) & 0x7;
        let pc_offset:u16 = Vm::sign_extend(instruction & 0x1ff, 9);
        self.reg[r0 as usize] = self.mem_read((self.reg[Registers::PC as usize] as u32 + pc_offset as u32) as u16);
        self.update_flags(r0);
    }
    #[inline]
    pub fn st(&mut self, instruction:u16) {
        let r0:u16 = (instruction >> 9) & 0x7;
        let pc_offset:u16 = Vm::sign_extend(instruction & 0x1FF, 9);
        self.mem_write((self.reg[Registers::PC as usize] as u32+pc_offset as u32) as u16, self.reg[r0 as usize]);
    }
    #[inline]
    pub fn jsr(&mut self, instruction:u16) {
        let long_flag = (instruction >> 11) & 1;
        self.reg[Registers::R7 as usize] = self.reg[Registers::PC as usize];

        if long_flag != 0 {
            let long_pc_offset:u16 = Self::sign_extend(instruction & 0x7FF, 11);
            let temp = (self.reg[Registers::PC as usize] as u32 + long_pc_offset as u32) as u16;
            self.reg[Registers::PC as usize] = temp; //JSR
        } else {
            let r1:u16 = (instruction >> 6) & 0x7;
            self.reg[Registers::PC as usize] = self.reg[r1 as usize]; //JSRR
        }
    }
    #[inline]
    pub fn and(&mut self, instruction:u16) {
        let r0:u16 = (instruction >> 9) & 0x7;
        let r1:u16 = (instruction >> 6) & 0x7;
        let imm_flag:u16 = (instruction >> 5) & 0x1;
        
        if imm_flag == 1 {
            let imm:u16 = Self::sign_extend(instruction & 0x1F, 5);
            self.reg[r0 as usize] = self.reg[r1 as usize] & imm;
        } else {
            let r2:u16 = instruction & 0x7;
            self.reg[r0 as usize] = self.reg[r1 as usize] & self.reg[r2 as usize];
        }
        self.update_flags(r0);
    }
    #[inline]
    pub fn ldr(&mut self, instruction:u16) {
        let r0:u16 = (instruction >> 9) & 0x7;
        let r1:u16 = (instruction >> 6) & 0x7;
        let offset:u16 = Vm::sign_extend(instruction & 0x3F, 6);
        self.reg[r0 as usize] = self.mem_read(self.reg[r1 as usize] + offset).clone();
        self.update_flags(r0);
    }
    #[inline]
    pub fn str(&mut self, instruction:u16) {
        let r0:u16 = (instruction >> 9) & 0x7;
        let r1:u16 = (instruction >> 6) & 0x7;
        let offset:u16 = Vm::sign_extend(instruction & 0x3F,6);
        self.mem_write((self.reg[r1 as usize] as u32 + offset as u32) as u16,self.reg[r0 as usize]);
    }
    #[inline]
    pub fn not(&mut self, instruction:u16) {
        let r0:u16 = (instruction >> 9) & 0x7;
        let r1:u16 = (instruction >> 6) & 0x7;
        self.reg[r0 as usize] = !self.reg[r1 as usize];
        self.update_flags(r0);
    }
    #[inline]
    pub fn ldi(&mut self, instruction:u16) {
        let r0 = (instruction >> 9) & 0x7;
        let pc_offset = Vm::sign_extend(instruction & 0x1ff,9);
        let temp = self.mem_read(self.reg[Registers::PC as usize]+pc_offset);
        self.reg[r0 as usize] = self.mem_read(temp); 
        self.update_flags(r0);
    }
    #[inline]
    pub fn sti(&mut self, instruction:u16) {
        let r0:u16 = (instruction >> 9) & 0x7;
        let pc_offset = Vm::sign_extend(instruction & 0x1FF, 9);
        let temp = self.mem_read((self.reg[Registers::PC as usize] as u32+pc_offset as u32) as u16);
        self.mem_write(temp,self.reg[r0 as usize]);
    }
    #[inline]
    pub fn jmp(&mut self, instruction:u16) {
        let r1:u16 = (instruction >> 6) & 0x7;
        self.reg[Registers::PC as usize] = self.reg[r1 as usize];
    }
    #[inline]
    pub fn lea(&mut self, instruction:u16) {
        let r0:u16 = (instruction >>  9) & 0x7;
        let pc_offset:u16 = Vm::sign_extend(instruction & 0x1FF, 9);
        self.reg[r0 as usize] = (self.reg[Registers::PC as usize] as u32 + pc_offset as u32) as u16;
        self.update_flags(r0);
    }
    #[inline]
    pub fn trap(&mut self, instruction:u16) {
        self.reg[Registers::R7 as usize] = self.reg[Registers::PC as usize];
        match TRAP::from(instruction & 0xFF) {
            Ok(TRAP::GETC) => {
                let mut buffer = [0;1];
                std::io::stdin().read_exact(&mut buffer).unwrap();
                self.reg[Registers::R0 as usize] = buffer[0] as u16;
            },
            Ok(TRAP::OUT) => {
                let c = self.reg[Registers::R0 as usize] as u8;
                print!("{}",c as char);
            },
            Ok(TRAP::PUTS) => {
                let mut index = self.reg[Registers::R0 as usize];
                let mut c = self.mem_read(index);
                while c != 0x0000 {
                    print!("{}",(c as u8) as char);
                    index+=1;
                    c = self.mem_read(index);
                }
                io::stdout().flush().expect("failed to flush");
            },
            Ok(TRAP::IN) => {
                print!("Enter a character :");
                io::stdout().flush().expect("Failed to flush");
                let char = std::io::stdin()
                    .bytes()
                    .next()
                    .and_then(|result| result.ok())
                    .map(|byte| byte as u16)
                    .unwrap();
                self.reg[Registers::R0 as usize] = char;
            },
            Ok(TRAP::PUTSP) => {
                let mut index = self.reg[Registers::R0 as usize];
                let mut c = self.mem_read(index);
                while c != 0x0000 {
                    let c1 = ((c & 0xFF) as u8) as char;
                    print!("{}",c1);
                    let c2 = ((c>>8) as u8) as char;
                    if c2 != '\0' {
                        print!("{}",c2);
                    }
                    index+=1;
                    c = self.mem_read(index);
                }
                io::stdout().flush().expect("failed to flush");
            },
            Ok(TRAP::HALT) => {
                println!("Halt");
                io::stdout().flush().expect("failed to flush");
                exit(1);
            },
            _ => {
                println!("Ended due to invalid instruction");

                exit(1);
            }
        }
    }

}

pub fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        exit(-2)
    }
    
    let mut vm = Vm::new();
    vm.reg[Registers::PC as usize] = 0x3000; //PC register to start

    for i in 1..args.len() {
        let result = vm.read_image(args[i].as_str()).unwrap();
        if result == 0 {
            println!("Failed to read from {}",args[i]);
        } else {
            println!("Read count: {}",result);
        }
    }
    

    
    'l: loop {
        let instruction:u16 = vm.mem_read(vm.reg[Registers::PC as usize]);
        vm.reg[Registers::PC as usize] += 1;
        let op = Operators::from(instruction >> 12);

        match op {
            Ok(Operators::BR)  =>   {
                vm.br(instruction);
            }
            Ok(Operators::ADD)  =>  {
                vm.add(instruction);
            }
            Ok(Operators::LD)   =>  {
                vm.ld(instruction);
            }
            Ok(Operators::ST)   =>  {
                vm.st(instruction);
            }
            Ok(Operators::JSR)  =>  {
                vm.jsr(instruction);
            }
            Ok(Operators::AND)  =>  {
                vm.and(instruction);
            }
            Ok(Operators::LDR)  =>  {
                vm.ldr(instruction);
            }
            Ok(Operators::STR)  =>  {
                vm.str(instruction);
            }
            Ok(Operators::RTI)  =>  {}
            Ok(Operators::NOT)  =>  {
                vm.not(instruction);
            }
            Ok(Operators::LDI)  =>  {
                vm.ldi(instruction);

            }
            Ok(Operators::STI)  =>  {
                vm.sti(instruction);
            }
            Ok(Operators::JMP)  =>  {
                vm.jmp(instruction);
            }
            Ok(Operators::RES)  =>  {}
            Ok(Operators::LEA)  =>  {
                vm.lea(instruction);
            }
            Ok(Operators::TRAP) =>  {
                vm.trap(instruction);
            }
            _ => {break 'l;}
        }
    }
    println!("Ended due to invalid instruction");
    exit(0);
}