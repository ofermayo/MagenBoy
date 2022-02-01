pub struct FixedSizeQueue<T:Copy, const SIZE:usize>{
    // According to the docs Vec should not be moved in memory if it not modified
    // Im modifing it but not increasing its allocated size once its allocated so I hope this will work for me
    // and I wont get weird memory issues
    _data: Vec<T>,
    end_data_pointer: *mut T,
    start_data_pointer: *mut T,
    data_pointer: *mut T,
    base_data_pointer: *mut T,
    length: usize,
}

impl<T:Copy, const SIZE:usize> FixedSizeQueue<T, SIZE>{
    pub fn new()->Self{
        let data = Vec::with_capacity(SIZE);
        let mut s = Self{
            _data:data,
            length:0,
            base_data_pointer: std::ptr::null_mut(),
            data_pointer: std::ptr::null_mut(),
            end_data_pointer: std::ptr::null_mut(),
            start_data_pointer: std::ptr::null_mut(),
        };

        s.base_data_pointer = s._data.as_mut_ptr();
        s.data_pointer = s._data.as_mut_ptr();
        s.start_data_pointer = s._data.as_mut_ptr();
        unsafe{s.end_data_pointer = s._data.as_mut_ptr().add(SIZE)};

        return s;
    }

    pub fn push(&mut self, t:T){
        if self.length < SIZE{
            unsafe{
                if self.data_pointer == self.end_data_pointer{
                    self.data_pointer = self.start_data_pointer;
                }
                *self.data_pointer = t;
                self.data_pointer = self.data_pointer.add(1);
            }
            self.length += 1;
        }
        else{
            std::panic!("queue is already full, size: {}", SIZE);
        }
    }

    pub fn remove(&mut self)->T{
        if self.length > 0{
            unsafe{
                let t = *self.base_data_pointer;
                self.base_data_pointer = self.base_data_pointer.add(1);
                if self.base_data_pointer == self.end_data_pointer{
                    self.base_data_pointer = self.start_data_pointer;
                }

                self.length -= 1;
                return t;
            }
        }
        
        std::panic!("The fifo is empty");
    }

    pub fn clear(&mut self){
        self.length = 0;
        self.data_pointer = self.start_data_pointer;
        self.base_data_pointer = self.start_data_pointer;
    }

    pub fn len(&self)->usize{
        self.length
    }
}

impl<T:Copy, const SIZE:usize> std::ops::Index<usize> for FixedSizeQueue<T, SIZE>{
    type Output = T;

    fn index(&self, mut index: usize) -> &Self::Output {
        if index < self.length{
            unsafe{
                if self.base_data_pointer.add(index) >= self.end_data_pointer{
                    index -= self.end_data_pointer.offset_from(self.base_data_pointer) as usize;
                }
                // casting a *mut T to a &T
                return &*(self.base_data_pointer.add(index));
            }
        }

        std::panic!("Index is out of range");
    }
}

impl<T:Copy, const SIZE:usize> std::ops::IndexMut<usize> for FixedSizeQueue<T, SIZE>{
    fn index_mut(&mut self, mut index: usize) -> &mut Self::Output {
        if index < self.length{
            unsafe{
                if self.base_data_pointer.add(index) >= self.end_data_pointer{
                    index -= self.end_data_pointer.offset_from(self.base_data_pointer) as usize;
                }
                // casting a *mut T to a &mut T
                return &mut *(self.base_data_pointer.add(index));
            }
        }

        std::panic!("Index is out of range");
    }
}