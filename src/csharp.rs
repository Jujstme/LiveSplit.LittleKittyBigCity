use asr::{Address64, Process};
use core::marker::PhantomData;

use bytemuck::AnyBitPattern;

#[repr(C)]
#[derive(Copy, Clone, Debug, AnyBitPattern)]
pub struct CSharpList<T: AnyBitPattern> {
    address: Address64,
    phantom_data: PhantomData<T>,
}

impl<T: AnyBitPattern> CSharpList<T> {
    /*
    /// Retrieve the number of elements in the current List object
    pub fn get_count(&self, process: &Process) -> Option<usize> {
        process
            .read_pointer(self.address, PointerSize::Bit64)
            .and_then(|addr| process.read::<u32>(addr + 0x18))
            .map(|val| val as usize)
            .ok()
    }
    */

    /// Iterates over all the elements of the current List
    pub fn iter<'a>(&self, process: &'a Process) -> impl DoubleEndedIterator<Item = T> + 'a {
        let raw_data = process.read::<[u8; 0x1C]>(self.address).ok();

        let data_pointer =
            raw_data.map(|data| unsafe { *(data.as_ptr().byte_add(0x10) as *const Address64) })
            .filter(|val| !val.is_null());

        let count = raw_data
            .map(|data| unsafe { *(data.as_ptr().byte_add(0x18) as *const u32) })
            .filter(|&val| val != 0)
            .map(|val| val as usize);

        let elements = match (data_pointer, count) {
            (Some(data_pointer), Some(count)) => process
                .read_vec::<Address64>(data_pointer + 0x20, count)
                .ok(),
            _ => None,
        };

        (0..count.unwrap_or_default()).filter_map(move |val| {
            elements
                .as_ref()
                .and_then(|element| process.read(element[val]).ok())
        })
    }

    /*
    /// Reads the content of the list
    pub fn read(&self, process: &Process) -> Option<Vec<T>> {
        let data: Vec<T> = self.iter(process).collect();

        match data.len() {
            0 => None,
            _ => Some(data),
        }
    }
    */

    /*
    /// Get the element located at the position specified in the current list (starting from 0)
    pub fn get_element_at(&self, process: &Process, position: usize) -> Option<T> {
        self.iter(process).nth(position)
    }
    */
}
