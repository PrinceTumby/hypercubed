use super::{QuadWord, dma, gs};
use bitfield::bitfield;
use core::num::NonZeroU8;

#[repr(C, align(64))]
#[derive(Clone, Debug)]
pub struct Packet<const N: usize> {
    fields: PacketFields,
    registers: u64,
    data: [QuadWord; N],
    // packet_end_fields: PacketFields,
}

#[expect(clippy::missing_safety_doc)]
pub unsafe trait GsRegisterPacketData {
    fn register_num(&self) -> u64;

    fn to_dword(&self) -> u64;
}

unsafe impl GsRegisterPacketData for [u64; 2] {
    fn register_num(&self) -> u64 {
        self[1]
    }

    fn to_dword(&self) -> u64 {
        self[0]
    }
}

impl<const N: usize> Packet<N> {
    const _NUM_TAGS_ASSERT: () = assert!(N <= (1 << 15));

    pub fn packed_from_gs_reg_array(tags: [&dyn GsRegisterPacketData; N]) -> Self {
        let mut data: [QuadWord; N] = [QuadWord { dwords: [0; 2] }; N];
        for (i, tag) in tags.into_iter().enumerate() {
            data[i].dwords = [tag.to_dword(), tag.register_num()];
        }
        let mut fields = PacketFields(0);
        fields.set_nloop(N.try_into().unwrap());
        fields.set_end_of_packet(true);
        fields.set_data_format(PacketDataFormat::Packed);
        fields.set_num_regs(NonZeroU8::new(1).unwrap());
        // let mut end_fields = PacketFields(0);
        // end_fields.set_nloop(0);
        // end_fields.set_end_of_packet(true);
        // end_fields.set_data_format(PacketDataFormat::Packed);
        // end_fields.set_num_regs(NonZeroU8::new(1).unwrap());
        Self {
            fields,
            registers: Register::AddressAndData as u64,
            data,
            // packet_end_fields: end_fields,
        }
    }

    pub fn as_dma_packet<'a>(&'a self) -> dma::Packet<'a> {
        unsafe {
            dma::Packet(core::slice::from_raw_parts(
                self as *const Self as *const QuadWord,
                N + 1,
                // N + 2,
            ))
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PacketDataFormat {
    Packed = 0,
    RegList = 1,
    Image = 2,
}

bitfield! {
    #[derive(Clone, Copy)]
    struct PacketFields(u64);
    impl Debug;
    u16;
    pub get_nloop, set_nloop: 14, 0;
    pub get_end_of_packet, set_end_of_packet: 15;
    pub get_prim_enable, set_prim_enable: 46;
    pub get_prim_data_raw, set_prim_data_raw: 57, 47;
    u8;
    pub get_data_format_raw, set_data_format_raw: 59, 58;
    pub get_num_regs_raw, set_num_regs_raw: 63, 60;
}

#[expect(unused)]
impl PacketFields {
    pub fn get_prim_data(&self) -> gs::tag::SetPrimitive {
        gs::tag::SetPrimitive(self.get_prim_data_raw() as u64)
    }

    pub fn set_prim_data(&mut self, prim_data: gs::tag::SetPrimitive) {
        self.set_prim_data_raw(prim_data.0 as u16);
    }

    pub fn get_data_format(&self) -> PacketDataFormat {
        match self.get_data_format_raw() {
            0 => PacketDataFormat::Packed,
            1 => PacketDataFormat::RegList,
            2 | 3 => PacketDataFormat::Image,
            _ => unreachable!(),
        }
    }

    pub fn set_data_format(&mut self, data_format: PacketDataFormat) {
        self.set_data_format_raw(data_format as u8);
    }

    pub fn get_num_regs(&self) -> NonZeroU8 {
        match self.get_num_regs_raw() {
            0 => NonZeroU8::new(16).unwrap(),
            x => NonZeroU8::new(x).unwrap(),
        }
    }

    pub fn set_num_regs(&mut self, num_regs: NonZeroU8) {
        let raw_num_regs = match num_regs.get() {
            16 => 0,
            x => x,
        };
        self.set_num_regs_raw(raw_num_regs);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Register {
    Prim = 0,
    Rgba = 1,
    Stq = 2,
    Uv = 3,
    Xyzf = 4,
    Xyz = 5,
    Fog = 10,
    AddressAndData = 14,
    Nop = 15,
}
