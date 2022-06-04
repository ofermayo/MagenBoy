use std::ptr::write_volatile;

use libc::{c_void, c_int};

use super::{*, raw_spi::Bcm2835};

// Mailbox messages need to be 16 byte alligned
#[repr(C, align(16))]
struct MailboxMessage<const PAYLOAD_SIZE:usize>{
    length:u32,
    request:u32,
    tag:u32,
    buffer_length:u32,
    data_length:u32,            // not sure if neccessary
    data:[u32;PAYLOAD_SIZE],
    message_end_indicator:u32
}

impl<const PAYLOAD_SIZE:usize> MailboxMessage<PAYLOAD_SIZE>{
    fn new(tag:u32, data:[u32;PAYLOAD_SIZE])->Self{
        Self{
            length:std::mem::size_of::<Self>() as u32,
            request:0,
            tag,
            buffer_length:(std::mem::size_of::<u32>()*PAYLOAD_SIZE) as u32,
            data_length:(std::mem::size_of::<u32>()*PAYLOAD_SIZE) as u32,
            data,
            message_end_indicator:0
        }
    }
}

// Docs - https://github.com/raspberrypi/firmware/wiki/Mailbox-property-interface
struct Mailbox{
    mbox_fd: c_int,
}

impl Mailbox{
    const MAILBOX_IOCTL_PROPERTY:libc::c_ulong = nix::request_code_readwrite!(100, 0, std::mem::size_of::<*mut libc::c_void>());

    fn new()->Self{
        let fd = unsafe{libc::open(std::ffi::CStr::from_bytes_with_nul(b"/dev/vcio\0").unwrap().as_ptr(), 0)};
        if fd < 0{
            std::panic!("Error while opening vc mailbox");
        }

        Self { mbox_fd: fd }
    }

    fn send_command<const SIZE:usize>(&self, tag:u32, data:[u32;SIZE])->u32{
        let mut message = MailboxMessage::<SIZE>::new(tag, data);
        return self.send_message(&mut message);
    }

    fn send_message<const SIZE:usize>(&self, message:&mut MailboxMessage<SIZE>)->u32{
        let raw_message = message as *mut MailboxMessage<SIZE> as *mut c_void;
        let ret = unsafe{
            // Using libc::ioctl and not nix high level abstraction over it cause Im sending a *void and not more 
            // concrete type and the nix macro will mess the types for us. I belive it could work with nix after some modification 
            // of the way Im handling this but Im leaving this as it for now. sorry!
            libc::ioctl(self.mbox_fd, Self::MAILBOX_IOCTL_PROPERTY, raw_message)
        };
        if ret < 0{
            libc_abort("Error in ioctl call");
        }

        return message.data[0];
    }
}

impl Drop for Mailbox{
    fn drop(&mut self) {
        unsafe{
            let result = libc::close(self.mbox_fd);
            if result != 0{
                libc_abort("Error while closing the mbox fd");
            }
        }
    }
}


// using GpuMemory cause I need a memory that is not cached by the cpu caches (L1, L2)
struct GpuMemory{
    virtual_address_ptr:usize,
    bus_address:u32,
    mailbox_memory_handle:u32,
    size:u32
}

impl GpuMemory{
    const MEM_ALLOC_FLAG_DIRECT:usize = 1 << 2;
    const MEM_ALLOC_FLAG_COHERENT:usize = 1 << 3;
    const ALLOCATE_MEMORY_TAG:u32 = 0x3000C;
    const LOCK_MEMORY_TAG:u32 = 0x3000D;
    const UNLOCK_MEMORY_TAG:u32 = 0x3000E;
    const RELEASE_MEMORY_TAG:u32 = 0x3000E;
    const PAGE_SIZE:u32 = 4096;

    // This function converts the from the bus address of the SDRAM uncached memory to the arm physical address
    // Notice that supposed to work only for this type of memory
    const fn bus_to_phys(bus_address:u32)->u32{bus_address & !0xC000_0000}

    // Using the Mailbox interface to allocate memory on the gpu
    fn allocate(mbox:&Mailbox, size:u32, mem_fd:c_int)->GpuMemory{
        let flags = (Self::MEM_ALLOC_FLAG_COHERENT | Self::MEM_ALLOC_FLAG_DIRECT) as u32;
        let handle = mbox.send_command(Self::ALLOCATE_MEMORY_TAG, [size, Self::PAGE_SIZE, flags]);

        let bus_address = mbox.send_command(Self::LOCK_MEMORY_TAG, [handle]);
        let virtual_address = unsafe{libc::mmap(
            std::ptr::null_mut(),
            size as libc::size_t,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            mem_fd,
            Self::bus_to_phys(bus_address) as libc::off_t
        )};

        return GpuMemory { virtual_address_ptr: virtual_address as usize, bus_address, mailbox_memory_handle:handle, size }
    }

    fn release(&self, mbox:&Mailbox){
        unsafe{
            let result = libc::munmap(self.virtual_address_ptr as *mut c_void, self.size as libc::size_t);
            if result != 0 {
                libc_abort("Error while trying to un map gpu memory");
            }
        }
        let status = mbox.send_command(Self::UNLOCK_MEMORY_TAG, [self.mailbox_memory_handle]);
        if status != 0{
            std::panic!("Error while trying to unlock gpu memory using mailbox");
        }
        let status = mbox.send_command(Self::RELEASE_MEMORY_TAG, [self.mailbox_memory_handle]);
        if status != 0{
            std::panic!("Error while to release gpu memory using mailbox");
        }
    }
}

// The DMA control block registers are in a 32 byte alligned addresses so the stracture mapping them needs to be as well
// in order for me to cast some bytes to this stuct (at least I think so)
// Not valid for DMA4 channels
#[repr(C, align(32))]
struct DmaControlBlock{
    transfer_information:u32,
    source_address:u32,
    destination_address:u32,
    trasnfer_length:u32,
    _stride:u32,                 // Not avalibale on the lite channels
    next_control_block_address:u32,
    _reserved:[u32;2]
}

impl DmaControlBlock{
    decl_write_volatile_field!(write_ti, transfer_information);
    decl_write_volatile_field!(write_source_ad, source_address);
    decl_write_volatile_field!(write_dest_ad, destination_address);
    decl_write_volatile_field!(write_txfr_len, trasnfer_length);
    decl_write_volatile_field!(write_nextconbk, next_control_block_address);
}


// Since Im casting an arbitary pointer to this struct it must be alligned by 4 bytes (with no gaps as well)
#[repr(C, align(4))]
struct DmaRegistersAccess{
    control_status:u32,
    control_block_address:u32,
    control_block:DmaControlBlock,
    debug:u32
}

impl DmaRegistersAccess{
    decl_write_volatile_field!(write_cs, control_status);
    decl_read_volatile_field!(read_cs, control_status);
    decl_write_volatile_field!(write_conblk_ad, control_block_address);
}

pub struct DmaTransferer<const CHUNK_SIZE:usize, const NUM_CHUNKS:usize>{
    tx_dma:*mut DmaRegistersAccess,
    rx_dma:*mut DmaRegistersAccess,
    mbox:Mailbox,
    tx_control_block_memory:GpuMemory,
    rx_control_block_memory:GpuMemory,
    source_buffer_memory:GpuMemory,
    dma_data_memory:GpuMemory,
    dma_const_data_memory:GpuMemory,
    tx_channel_number:u8,
    rx_channel_number:u8,
    dma_enable_register_ptr:*mut u32,
}

impl<const CHUNK_SIZE:usize, const NUM_CHUNKS:usize> DmaTransferer<CHUNK_SIZE, NUM_CHUNKS>{
    const BCM2835_DMA0_OFFSET:usize = 0x7_000;
    const BCM2835_DMA_ENABLE_REGISTER_OFFSET:usize = Self::BCM2835_DMA0_OFFSET + 0xFF;

    const DMA_CS_RESET:u32 = 1 << 31;
    const DMA_CS_END:u32 = 1 << 1;
    const DMA_CS_ACTIVE:u32 = 1;

    const DMA_TI_SRC_DREQ:u32 = 1 << 10;
    const DMA_TI_SRC_INC:u32 = 1 << 8;
    const DMA_TI_DEST_IGNORE:u32 = 1 << 7;
    const DMA_TI_DEST_DREQ:u32 = 1 << 6;
    const DMA_TI_DEST_INC:u32 = 1 << 4;
    const DMA_TI_WAIT_RESP:u32 = 1 << 3;

    const DMA_DMA0_CB_PHYS_ADDRESS:u32 = 0x7E00_7000;
    const fn dma_ti_permap(peripherial_mapping:u8)->u32{(peripherial_mapping as u32) << 16}

    pub fn new(bcm2835:&Bcm2835, tx_channel_number:u8, rx_channel_number:u8)->Self{
        let mbox = Mailbox::new();
        let tx_registers = bcm2835.get_ptr(Self::BCM2835_DMA0_OFFSET + (tx_channel_number as usize * 0x100)) as *mut DmaRegistersAccess;
        let rx_registers = bcm2835.get_ptr(Self::BCM2835_DMA0_OFFSET + (rx_channel_number as usize * 0x100)) as *mut DmaRegistersAccess;
        let dma_tx_control_block_memory = GpuMemory::allocate(&mbox, std::mem::size_of::<DmaControlBlock>() as u32 * 4 * NUM_CHUNKS as u32, bcm2835.get_fd());
        let dma_rx_control_block_memory = GpuMemory::allocate(&mbox, std::mem::size_of::<DmaControlBlock>() as u32 * NUM_CHUNKS as u32, bcm2835.get_fd());
        let dma_source_buffer_memory = GpuMemory::allocate(&mbox, (NUM_CHUNKS * CHUNK_SIZE) as u32, bcm2835.get_fd());
        let dma_data_memory = GpuMemory::allocate(&mbox, (std::mem::size_of::<u32>() * NUM_CHUNKS) as u32, bcm2835.get_fd());
        let dma_const_data_memory = GpuMemory::allocate(&mbox, (std::mem::size_of::<u32>() * 2) as u32, bcm2835.get_fd());

        let dma_enable_register = bcm2835.get_ptr(Self::BCM2835_DMA_ENABLE_REGISTER_OFFSET) as *mut u32;

        unsafe{
            // setup constant data
            let ptr = dma_const_data_memory.virtual_address_ptr as *mut u32;
            write_volatile(ptr, 0x100); // spi_dma enable
            write_volatile(ptr.add(1), Self::DMA_CS_ACTIVE | Self::DMA_CS_END);

            // enable the rx & tx dma channels
            write_volatile(dma_enable_register, *dma_enable_register | 1 << tx_channel_number | 1<< rx_channel_number);

            //reset the dma channels
            (*tx_registers).write_cs(Self::DMA_CS_RESET);
            (*rx_registers).write_cs(Self::DMA_CS_RESET);

            // memset the memory
            std::ptr::write_bytes(dma_rx_control_block_memory.virtual_address_ptr as *mut u8, 0, dma_rx_control_block_memory.size as usize);
            std::ptr::write_bytes(dma_tx_control_block_memory.virtual_address_ptr as *mut u8, 0, dma_tx_control_block_memory.size as usize);
            std::ptr::write_bytes(dma_source_buffer_memory.virtual_address_ptr as *mut u8, 0, dma_source_buffer_memory.size as usize);
            std::ptr::write_bytes(dma_data_memory.virtual_address_ptr as *mut u8, 0, dma_data_memory.size as usize);
        }

        Self { 
            tx_dma: tx_registers,
            rx_dma: rx_registers,
            mbox,
            rx_control_block_memory:dma_rx_control_block_memory,
            tx_control_block_memory:dma_tx_control_block_memory,
            source_buffer_memory:dma_source_buffer_memory,
            dma_data_memory,
            rx_channel_number,
            tx_channel_number,
            dma_const_data_memory,
            dma_enable_register_ptr:dma_enable_register
        }
    }


    const DMA_SPI_CS_PHYS_ADDRESS:u32 = 0x7E20_4000;

    pub fn start_dma_transfer<const SIZE:usize>(&mut self, data:&[u8; SIZE], tx_peripherial_mapping:u8, tx_physical_destination_address:u32, rx_peripherial_mapping:u8, rx_physical_destination_address:u32){
        if SIZE != NUM_CHUNKS * CHUNK_SIZE{
            std::panic!("bad SIZE param");
        }

        unsafe{
            std::ptr::copy_nonoverlapping(data.as_ptr(), self.source_buffer_memory.virtual_address_ptr as *mut u8, SIZE);

            let mut rx_control_block = &mut *(self.rx_control_block_memory.virtual_address_ptr as *mut DmaControlBlock);
            rx_control_block.write_ti(Self::dma_ti_permap(rx_peripherial_mapping) | Self::DMA_TI_SRC_DREQ | Self::DMA_TI_DEST_IGNORE);
            rx_control_block.write_source_ad(rx_physical_destination_address);
            rx_control_block.write_dest_ad(0);
            rx_control_block.write_txfr_len(CHUNK_SIZE as u32 - 4);       // without the 4 byte header
            rx_control_block.write_nextconbk(0);

            let tx_control_block = &mut *(self.tx_control_block_memory.virtual_address_ptr as *mut DmaControlBlock);
            tx_control_block.write_ti(Self::dma_ti_permap(tx_peripherial_mapping) | Self::DMA_TI_DEST_DREQ | Self::DMA_TI_SRC_INC | Self::DMA_TI_WAIT_RESP);
            tx_control_block.write_source_ad(self.source_buffer_memory.bus_address);
            tx_control_block.write_dest_ad(tx_physical_destination_address);
            tx_control_block.write_txfr_len(CHUNK_SIZE as u32);
            tx_control_block.write_nextconbk(0);

            for i in 1..NUM_CHUNKS{
                let tx_cb_index = i * 4;
                let tx_control_block = &mut *((self.tx_control_block_memory.virtual_address_ptr as *mut DmaControlBlock).add(tx_cb_index));
                tx_control_block.write_ti(Self::dma_ti_permap(tx_peripherial_mapping) | Self::DMA_TI_DEST_DREQ | Self::DMA_TI_SRC_INC | Self::DMA_TI_WAIT_RESP);
                tx_control_block.write_source_ad(self.source_buffer_memory.bus_address + (i * CHUNK_SIZE) as u32);
                tx_control_block.write_dest_ad(tx_physical_destination_address);
                tx_control_block.write_txfr_len(CHUNK_SIZE as u32);
                tx_control_block.write_nextconbk(0);

                let set_dma_tx_address = &mut *((self.tx_control_block_memory.virtual_address_ptr as *mut DmaControlBlock).add(tx_cb_index + 1));
                let disable_dma_tx_address = &mut *((self.tx_control_block_memory.virtual_address_ptr as *mut DmaControlBlock).add(tx_cb_index + 2));
                let start_dma_tx_address = &mut *((self.tx_control_block_memory.virtual_address_ptr as *mut DmaControlBlock).add(tx_cb_index + 3));

                rx_control_block.write_nextconbk(self.tx_control_block_memory.bus_address + ((tx_cb_index + 1) * std::mem::size_of::<DmaControlBlock>()) as u32);

                write_volatile((self.dma_data_memory.virtual_address_ptr as *mut u32).add(i), self.tx_control_block_memory.bus_address + (tx_cb_index * std::mem::size_of::<DmaControlBlock>()) as u32);

                set_dma_tx_address.write_ti(Self::DMA_TI_SRC_INC | Self::DMA_TI_DEST_INC | Self::DMA_TI_WAIT_RESP);
                set_dma_tx_address.write_source_ad(self.dma_data_memory.bus_address + (i as u32 * 4));
                set_dma_tx_address.write_dest_ad(Self::DMA_DMA0_CB_PHYS_ADDRESS + (self.tx_channel_number as u32 * 0x100) + 4);  // channel control block address register
                set_dma_tx_address.write_txfr_len(4);
                set_dma_tx_address.write_nextconbk(self.tx_control_block_memory.bus_address + ((tx_cb_index + 2) * std::mem::size_of::<DmaControlBlock>()) as u32);


                disable_dma_tx_address.write_ti(Self::DMA_TI_SRC_INC | Self::DMA_TI_DEST_INC | Self::DMA_TI_WAIT_RESP);
                disable_dma_tx_address.write_source_ad(self.dma_const_data_memory.bus_address);
                disable_dma_tx_address.write_dest_ad(Self::DMA_SPI_CS_PHYS_ADDRESS);
                disable_dma_tx_address.write_txfr_len(4);
                disable_dma_tx_address.write_nextconbk(self.tx_control_block_memory.bus_address + ((tx_cb_index + 3) * std::mem::size_of::<DmaControlBlock>()) as u32);

                
                start_dma_tx_address.write_ti(Self::DMA_TI_SRC_INC | Self::DMA_TI_DEST_INC | Self::DMA_TI_WAIT_RESP);
                start_dma_tx_address.write_source_ad(self.dma_const_data_memory.bus_address + 4);
                start_dma_tx_address.write_dest_ad(Self::DMA_DMA0_CB_PHYS_ADDRESS + (self.tx_channel_number as u32 * 0x100) as u32);
                start_dma_tx_address.write_txfr_len(4);
                start_dma_tx_address.write_nextconbk(self.rx_control_block_memory.bus_address + (i * std::mem::size_of::<DmaControlBlock>()) as u32);


                rx_control_block = &mut *((self.rx_control_block_memory.virtual_address_ptr as *mut DmaControlBlock).add(i));
                rx_control_block.write_ti(Self::dma_ti_permap(rx_peripherial_mapping) | Self::DMA_TI_SRC_DREQ | Self::DMA_TI_DEST_IGNORE);
                rx_control_block.write_source_ad(rx_physical_destination_address);
                rx_control_block.write_dest_ad(0);
                rx_control_block.write_txfr_len(CHUNK_SIZE as u32 - 4);       // without the 4 byte header
                rx_control_block.write_nextconbk(0);
            }

            
            (*self.tx_dma).write_conblk_ad(self.tx_control_block_memory.bus_address);
            (*self.rx_dma).write_conblk_ad(self.rx_control_block_memory.bus_address);

            // Starting the dma transfer
            (*self.tx_dma).write_cs(Self::DMA_CS_ACTIVE | Self::DMA_CS_END);
            (*self.rx_dma).write_cs(Self::DMA_CS_ACTIVE | Self::DMA_CS_END);
        }
    }

    pub fn end_dma_transfer(&self){
        unsafe{
            // Wait for the last trasfer to end
            while (*self.tx_dma).read_cs() & Self::DMA_CS_ACTIVE != 0 {
                // Self::sleep_ms(250);
                // log::info!("Waiting for the tx channel");
            }
            while (*self.rx_dma).read_cs() & Self::DMA_CS_ACTIVE != 0 {
                // Self::sleep_ms(250);
                // log::info!("Waiting for the rx channel");
            }
        }
    }

    fn sleep_ms(milliseconds_to_sleep:u64){
        std::thread::sleep(std::time::Duration::from_millis(milliseconds_to_sleep));
    }
}

impl<const CHUNK_SIZE:usize, const NUM_CHUNKS:usize> Drop for DmaTransferer<CHUNK_SIZE, NUM_CHUNKS>{
    fn drop(&mut self) {
        // reset the program before releasing the memory
        unsafe{
            // reset the dma channels
            (*self.tx_dma).write_cs(Self::DMA_CS_RESET);
            (*self.rx_dma).write_cs(Self::DMA_CS_RESET);
            // disable the channels I used
            let mask = !((1 << self.tx_channel_number) | (1 << self.rx_channel_number));
            *self.dma_enable_register_ptr &= mask;
        }

        self.dma_const_data_memory.release(&self.mbox);
        self.dma_data_memory.release(&self.mbox);
        self.rx_control_block_memory.release(&self.mbox);
        self.source_buffer_memory.release(&self.mbox);
        self.tx_control_block_memory.release(&self.mbox);
    }
}