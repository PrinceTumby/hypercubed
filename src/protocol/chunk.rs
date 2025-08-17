use super::prelude::*;
use portable_std::FastHashSet;
use crate::portable_prelude::*;
use resources::block::GlobalPaletteIndex;
use nom::Parser;
use nom::bytes::tag;
use nom::sequence::pair;
use protocol_derive::Deserialize;

#[derive(Clone, Deserialize)]
pub struct ChunkSection {
    pub block_count: u16,
    pub block_states: PalettedContainer<16, 8>,
    pub biomes: PalettedContainer<4, 3>,
}

#[derive(Clone)]
pub struct PalettedContainer<const AXIS_LEN: usize, const INDIRECT_MAX_BPE: u8> {
    bits_per_entry: u8,
    palette: Palette,
    data: Vec<u64>,
}

#[derive(Clone, Debug)]
pub enum Palette {
    SingleValue(GlobalPaletteIndex),
    Palette(Vec<GlobalPaletteIndex>),
    Direct,
}

impl<const AXIS_LEN: usize, const INDIRECT_MAX_BPE: u8>
    PalettedContainer<AXIS_LEN, INDIRECT_MAX_BPE>
{
    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> GlobalPaletteIndex {
        if let &Palette::SingleValue(value) = &self.palette {
            return value;
        }
        let entry_value = Self::get_raw(&self.data, self.bits_per_entry, x, y, z);
        match &self.palette {
            Palette::Palette(entries) => entries[usize::try_from(entry_value).unwrap()],
            Palette::Direct => entry_value.try_into().unwrap(),
            _ => unreachable!(),
        }
    }

    pub fn replace(
        &mut self,
        x: usize,
        y: usize,
        z: usize,
        value: GlobalPaletteIndex,
    ) -> GlobalPaletteIndex {
        // TODO: Keep track of the number of updates made to a container, after enough updates see
        // if we can compact it by counting the number of different values stored.
        match &mut self.palette {
            Palette::SingleValue(palette_value) => match value == *palette_value {
                true => *palette_value,
                false => {
                    // Upgrade to a paletted container with a small number of bits
                    const NEW_BITS_PER_ENTRY: u8 = 4;
                    let current_palette_value = *palette_value;
                    let new_palette = vec![current_palette_value, value];
                    let new_entries_per_long = 64 / NEW_BITS_PER_ENTRY as usize;
                    let mut new_data = vec![0u64; AXIS_LEN.pow(3).div_ceil(new_entries_per_long)];
                    Self::replace_raw(
                        &mut new_data,
                        NEW_BITS_PER_ENTRY,
                        x,
                        y,
                        z,
                        // New replacement value is at index 1 in new palette
                        1,
                    );
                    self.palette = Palette::Palette(new_palette);
                    self.data = new_data;
                    current_palette_value
                }
            },
            Palette::Palette(palette) => match palette.iter().position(|&v| v == value) {
                Some(palette_index) => {
                    let bit_value: u64 = palette_index.try_into().unwrap();
                    let old_bit_value =
                        Self::replace_raw(&mut self.data, self.bits_per_entry, x, y, z, bit_value);
                    palette[usize::try_from(old_bit_value).unwrap()]
                }
                None => {
                    let palette_index = palette.len();
                    palette.push(value);
                    // Check if we have enough bits already, or if we need to convert the data
                    // storage.
                    if palette.len() >= (1 << self.bits_per_entry) {
                        // Increasing number of bits by 1 doubles the palette size, which should be
                        // good enough to ensure resizes aren't too frequently needed.
                        let new_bits_per_entry = self.bits_per_entry + 1;
                        self.data = Self::convert_data(
                            &self.data,
                            &mut self.palette,
                            self.bits_per_entry,
                            new_bits_per_entry,
                        );
                        self.bits_per_entry = new_bits_per_entry;
                    }
                    // Palette might have changed due to a data conversion
                    match &mut self.palette {
                        Palette::Palette(palette) => {
                            let bit_value: u64 = palette_index.try_into().unwrap();
                            let old_bit_value = Self::replace_raw(
                                &mut self.data,
                                self.bits_per_entry,
                                x,
                                y,
                                z,
                                bit_value,
                            );
                            palette[usize::try_from(old_bit_value).unwrap()]
                        }
                        Palette::Direct => {
                            let bit_value: u64 = value.as_raw().into();
                            let old_bit_value = Self::replace_raw(
                                &mut self.data,
                                self.bits_per_entry,
                                x,
                                y,
                                z,
                                bit_value,
                            );
                            old_bit_value.try_into().unwrap()
                        }
                        _ => unreachable!(),
                    }
                }
            },
            Palette::Direct => {
                let bit_value: u64 = value.as_raw().into();
                let old_bit_value =
                    Self::replace_raw(&mut self.data, self.bits_per_entry, x, y, z, bit_value);
                old_bit_value.try_into().unwrap()
            }
        }
    }

    pub fn get_all_of_types(
        &self,
        blockstate_types_set: &FastHashSet<GlobalPaletteIndex>,
    ) -> Vec<([u8; 3], GlobalPaletteIndex)> {
        let axis_len_u8: u8 = AXIS_LEN.try_into().unwrap();
        let mut light_positions = Vec::new();
        match &self.palette {
            Palette::SingleValue(value) => {
                if blockstate_types_set.contains(value) {
                    for x in 0..axis_len_u8 {
                        for y in 0..axis_len_u8 {
                            for z in 0..axis_len_u8 {
                                light_positions.push(([x, y, z], *value));
                            }
                        }
                    }
                }
            }
            Palette::Palette(_) | Palette::Direct => {
                for x in 0..axis_len_u8 {
                    for y in 0..axis_len_u8 {
                        for z in 0..axis_len_u8 {
                            let blockstate = self.get(x as usize, y as usize, z as usize);
                            if blockstate_types_set.contains(&blockstate) {
                                light_positions.push(([x, y, z], blockstate));
                            }
                        }
                    }
                }
            }
        }
        light_positions
    }

    #[inline]
    fn get_raw(data: &[u64], bits_per_entry: u8, x: usize, y: usize, z: usize) -> u64 {
        let entries_per_long = 64 / bits_per_entry as usize;
        let entry_idx = (y * AXIS_LEN.pow(2)) + (z * AXIS_LEN) + x;
        let long_idx = entry_idx / entries_per_long;
        let bitshift = ((entry_idx % entries_per_long) * bits_per_entry as usize) as u64;
        let entry_bitmask: u64 = (1 << bits_per_entry as u64) - 1;
        (data[long_idx] >> bitshift) & entry_bitmask
    }

    #[inline]
    fn set_raw(data: &mut [u64], bits_per_entry: u8, x: usize, y: usize, z: usize, bit_value: u64) {
        let entries_per_long = 64 / bits_per_entry as usize;
        let entry_idx = (y * AXIS_LEN.pow(2)) + (z * AXIS_LEN) + x;
        let long_idx = entry_idx / entries_per_long;
        let bitshift = ((entry_idx % entries_per_long) * bits_per_entry as usize) as u64;
        let entry_bitmask = ((1 << bits_per_entry as u64) - 1) << bitshift;
        let masked_entry_value = data[long_idx] & !entry_bitmask;
        data[long_idx] = masked_entry_value | (bit_value << bitshift);
    }

    #[inline]
    fn replace_raw(
        data: &mut [u64],
        bits_per_entry: u8,
        x: usize,
        y: usize,
        z: usize,
        bit_value: u64,
    ) -> u64 {
        let entries_per_long = 64 / bits_per_entry as usize;
        let entry_idx = (y * AXIS_LEN.pow(2)) + (z * AXIS_LEN) + x;
        let long_idx = entry_idx / entries_per_long;
        let bitshift = ((entry_idx % entries_per_long) * bits_per_entry as usize) as u64;
        let entry_bitmask = ((1 << bits_per_entry as u64) - 1) << bitshift;
        let old_bit_value = (data[long_idx] & entry_bitmask) >> bitshift;
        let masked_entry_value = data[long_idx] & !entry_bitmask;
        data[long_idx] = masked_entry_value | (bit_value << bitshift);
        old_bit_value
    }

    fn convert_data(
        old_data: &[u64],
        old_palette: &mut Palette,
        old_bits_per_entry: u8,
        new_bits_per_entry: u8,
    ) -> Vec<u64> {
        assert!(matches!(old_palette, Palette::Palette(_) | Palette::Direct));
        let new_entries_per_long = 64 / new_bits_per_entry as usize;
        let new_longs_needed = AXIS_LEN.pow(3).div_ceil(new_entries_per_long);
        let mut new_data: Vec<u64> = vec![0; new_longs_needed];
        let is_new_data_paletted = new_bits_per_entry <= INDIRECT_MAX_BPE;
        // palette.iter().position(|&v| v == value)
        match (&old_palette, is_new_data_paletted) {
            (Palette::Direct, false) | (Palette::Palette(_), true) => {
                for y in 0..AXIS_LEN {
                    for z in 0..AXIS_LEN {
                        for x in 0..AXIS_LEN {
                            Self::set_raw(
                                &mut new_data,
                                new_bits_per_entry,
                                x,
                                y,
                                z,
                                Self::get_raw(old_data, old_bits_per_entry, x, y, z),
                            );
                        }
                    }
                }
            }
            (Palette::Direct, true) => {
                let mut new_palette = Vec::new();
                for y in 0..AXIS_LEN {
                    for z in 0..AXIS_LEN {
                        for x in 0..AXIS_LEN {
                            let bit_value = Self::get_raw(old_data, old_bits_per_entry, x, y, z);
                            let palette_entry = GlobalPaletteIndex::try_from(bit_value).unwrap();
                            let palette_idx =
                                match new_palette.iter().position(|&v| v == palette_entry) {
                                    Some(idx) => idx,
                                    None => {
                                        new_palette.push(palette_entry);
                                        new_palette.len() - 1
                                    }
                                };
                            let idx_bit_value: u64 = palette_idx.try_into().unwrap();
                            Self::set_raw(
                                &mut new_data,
                                new_bits_per_entry,
                                x,
                                y,
                                z,
                                idx_bit_value,
                            );
                        }
                    }
                }
                *old_palette = Palette::Palette(new_palette);
            }
            (Palette::Palette(palette), false) => {
                for y in 0..AXIS_LEN {
                    for z in 0..AXIS_LEN {
                        for x in 0..AXIS_LEN {
                            let bit_value = Self::get_raw(old_data, old_bits_per_entry, x, y, z);
                            let palette_idx: usize = bit_value.try_into().unwrap();
                            let palette_entry = palette[palette_idx];
                            Self::set_raw(
                                &mut new_data,
                                new_bits_per_entry,
                                x,
                                y,
                                z,
                                palette_entry.as_raw().into(),
                            );
                        }
                    }
                }
                *old_palette = Palette::Direct;
            }
            _ => unreachable!(),
        }
        new_data
    }
}

impl<const AXIS_LEN: usize, const INDIRECT_MAX_BPE: u8> core::fmt::Display
    for PalettedContainer<AXIS_LEN, INDIRECT_MAX_BPE>
{
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use core::fmt::Write;
        write!(
            f,
            "PalettedContainer(bits_per_entry: {}, palette: {:?})",
            self.bits_per_entry, self.palette
        )?;
        if matches!(self.palette, Palette::SingleValue(_)) {
            return Ok(());
        }
        writeln!(f, " {{")?;
        // Generate data debug string
        let mut current_row = String::new();
        for y in 0..AXIS_LEN {
            writeln!(f, "Layer {y}")?;
            for z in 0..AXIS_LEN {
                for x in 0..AXIS_LEN {
                    write!(current_row, " {:04X}", self.get(x, y, z).as_raw())?;
                }
                writeln!(f, "{current_row}")?;
                current_row.clear();
            }
        }
        writeln!(f, "}}")?;
        Ok(())
    }
}

impl<const AXIS_LEN: usize, const INDIRECT_MAX_BPE: u8> Deserialize
    for PalettedContainer<AXIS_LEN, INDIRECT_MAX_BPE>
{
    fn deserialize(input: InputSpan) -> IResult<Self> {
        let (rest, bits_per_entry) = u8::deserialize(input)?;
        match bits_per_entry {
            0 => {
                pair(
                    VarInt::deserialize.map(|VarInt(value)| value.try_into().unwrap()),
                    // Length of `data`, zero because there's no data needed for a single value
                    // palette
                    tag(core::slice::from_ref(&0)),
                )
                .map(|(palette_index, _data_len)| Self {
                    bits_per_entry,
                    palette: Palette::SingleValue(palette_index),
                    data: Vec::new(),
                })
                .parse(rest)
            }
            _ if bits_per_entry <= INDIRECT_MAX_BPE => pair(
                <Vec<VarInt>>::deserialize.map(|indices| {
                    indices
                        .into_iter()
                        .map(|VarInt(index)| index.try_into().unwrap())
                        .collect()
                }),
                <Vec<u64>>::deserialize,
            )
            .map(|(palette_entries, data)| {
                let entries_per_long = 64 / bits_per_entry as usize;
                let num_longs = AXIS_LEN.pow(3) / entries_per_long;
                let extra_long = !AXIS_LEN.pow(3).is_multiple_of(entries_per_long);
                assert_eq!(data.len(), num_longs + extra_long as usize);
                Self {
                    bits_per_entry,
                    palette: Palette::Palette(palette_entries),
                    data,
                }
            })
            .parse(rest),
            _ => <Vec<u64>>::deserialize
                .map(|data| {
                    let entries_per_long = 64 / bits_per_entry as usize;
                    let num_longs = AXIS_LEN.pow(3) / entries_per_long;
                    let extra_long = !AXIS_LEN.pow(3).is_multiple_of(entries_per_long);
                    assert_eq!(data.len(), num_longs + extra_long as usize);
                    Self {
                        bits_per_entry,
                        palette: Palette::Direct,
                        data,
                    }
                })
                .parse(rest),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct RawChunkLightInfo {
    pub sky_light_mask: BitSet,
    pub block_light_mask: BitSet,
    pub empty_sky_light_mask: BitSet,
    pub empty_block_light_mask: BitSet,
    pub sky_light_sections: Vec<Vec<u8>>,
    pub block_light_sections: Vec<Vec<u8>>,
}

#[derive(Clone, Debug)]
pub struct ChunkLightInfo {
    pub sky_light_sections: Vec<SectionLightData>,
    pub block_light_sections: Vec<SectionLightData>,
}

#[derive(Clone, Debug)]
pub enum SectionLightData {
    AllZeroes,
    AllOnes,
    RawData(Vec<u8>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightType {
    Sky,
    Block,
}

impl ChunkLightInfo {
    pub fn from_raw(info: RawChunkLightInfo, num_sections: usize) -> Self {
        let mut converted_sky_sections = Vec::new();
        let mut converted_block_sections = Vec::new();
        let mut raw_sky_sections = info.sky_light_sections.into_iter();
        let mut raw_block_sections = info.block_light_sections.into_iter();
        for section_i in 0..num_sections {
            if info.sky_light_mask[section_i] {
                let raw_sky_section = raw_sky_sections.next().unwrap();
                if raw_sky_section.iter().all(|&byte| byte == 0xFF) {
                    converted_sky_sections.push(SectionLightData::AllOnes);
                } else {
                    converted_sky_sections.push(SectionLightData::RawData(raw_sky_section));
                }
            } else if info.empty_sky_light_mask[section_i] {
                converted_sky_sections.push(SectionLightData::AllZeroes);
            } else {
                // Sections after bit masks end are implied to be all ones for sky lighting.
                converted_sky_sections.push(SectionLightData::AllOnes);
            }
            if info.block_light_mask[section_i] {
                let raw_block_section = raw_block_sections.next().unwrap();
                if raw_block_section.iter().all(|&byte| byte == 0xFF) {
                    converted_block_sections.push(SectionLightData::AllOnes);
                } else {
                    converted_block_sections.push(SectionLightData::RawData(raw_block_section));
                }
            } else {
                // Sections after bit masks end are implied to be all zeroes for block lighting.
                converted_block_sections.push(SectionLightData::AllZeroes);
            }
        }
        ChunkLightInfo {
            sky_light_sections: converted_sky_sections,
            block_light_sections: converted_block_sections,
        }
    }

    #[inline]
    pub fn get_section<'a>(
        &'a self,
        min_height: i32,
        subchunk_y: i32,
    ) -> Option<ChunkSectionLightInfo<'a>> {
        // Light arrays contain an extra section above and below the world chunk sections
        let adjusted_min_height = min_height - 16;
        let min_subchunk_y = adjusted_min_height / 16;
        let section_i: usize = (subchunk_y - min_subchunk_y).try_into().ok()?;
        Some(ChunkSectionLightInfo {
            raw_sky_light_data: &self.sky_light_sections[section_i],
            raw_block_light_data: &self.block_light_sections[section_i],
        })
    }

    #[inline]
    pub fn get_section_channel_mut<'a>(
        &'a mut self,
        min_height: i32,
        subchunk_y: i32,
        channel: LightType,
    ) -> Option<ChunkSectionLightChannelInfoMut<'a>> {
        // Light arrays contain an extra section above and below the world chunk sections
        let adjusted_min_height = min_height - 16;
        let min_subchunk_y = adjusted_min_height / 16;
        let section_i: usize = (subchunk_y - min_subchunk_y).try_into().ok()?;
        let data = match channel {
            LightType::Sky => self.sky_light_sections.get_mut(section_i)?,
            LightType::Block => self.block_light_sections.get_mut(section_i)?,
        };
        Some(ChunkSectionLightChannelInfoMut(data))
    }

    pub fn update_from_raw(&mut self, info: RawChunkLightInfo) {
        let mut raw_sky_sections = info.sky_light_sections.into_iter();
        for (section_i, section) in self.sky_light_sections.iter_mut().enumerate() {
            if info.sky_light_mask[section_i] {
                *section = SectionLightData::RawData(raw_sky_sections.next().unwrap());
            } else if info.empty_sky_light_mask[section_i] {
                *section = SectionLightData::AllZeroes;
            }
        }
        let mut raw_block_sections = info.block_light_sections.into_iter();
        for (section_i, section) in self.block_light_sections.iter_mut().enumerate() {
            if info.block_light_mask[section_i] {
                *section = SectionLightData::RawData(raw_block_sections.next().unwrap());
            } else if info.empty_block_light_mask[section_i] {
                *section = SectionLightData::AllZeroes;
            }
        }
    }
}

pub struct ChunkSectionLightInfo<'a> {
    raw_sky_light_data: &'a SectionLightData,
    raw_block_light_data: &'a SectionLightData,
}

impl ChunkSectionLightInfo<'_> {
    /// Returns the sky and block light levels for a block in the chunk section.
    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> [u8; 2] {
        debug_assert!(x < 16);
        debug_assert!(y < 16);
        debug_assert!(z < 16);
        let sky_light_level = match &self.raw_sky_light_data {
            SectionLightData::AllZeroes => 0,
            SectionLightData::AllOnes => 15,
            SectionLightData::RawData(data) => {
                let value_index = (y << 8) | (z << 4) | x;
                let data_index = value_index / 2;
                match value_index % 2 {
                    0 => data[data_index] & 0x0F,
                    _ => (data[data_index] & 0xF0) >> 4,
                }
            }
        };
        let block_light_level = match &self.raw_block_light_data {
            SectionLightData::AllZeroes => 0,
            SectionLightData::AllOnes => 15,
            SectionLightData::RawData(data) => {
                let value_index = (y << 8) | (z << 4) | x;
                let data_index = value_index / 2;
                match value_index % 2 {
                    0 => data[data_index] & 0x0F,
                    _ => (data[data_index] & 0xF0) >> 4,
                }
            }
        };
        [sky_light_level, block_light_level]
    }
}

pub struct ChunkSectionLightChannelInfoMut<'a>(pub &'a mut SectionLightData);

impl ChunkSectionLightChannelInfoMut<'_> {
    /// Returns the light level for a block in the chunk section channel.
    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> u8 {
        debug_assert!(x < 16);
        debug_assert!(y < 16);
        debug_assert!(z < 16);
        match &self.0 {
            SectionLightData::AllZeroes => 0,
            SectionLightData::AllOnes => 15,
            SectionLightData::RawData(data) => {
                let value_index = (y << 8) | (z << 4) | x;
                let data_index = value_index / 2;
                match value_index % 2 {
                    0 => data[data_index] & 0x0F,
                    _ => (data[data_index] & 0xF0) >> 4,
                }
            }
        }
    }

    /// Sets the light level for a block in the chunk section channel.
    #[inline]
    pub fn set(&mut self, x: usize, y: usize, z: usize, level: u8) {
        debug_assert!(x < 16);
        debug_assert!(y < 16);
        debug_assert!(z < 16);
        debug_assert!(level < 16);
        match &mut self.0 {
            SectionLightData::RawData(_) => {}
            SectionLightData::AllZeroes if level != 0 => {
                *self.0 = SectionLightData::RawData(vec![0x00; 2048]);
            }
            SectionLightData::AllOnes if level != 15 => {
                *self.0 = SectionLightData::RawData(vec![0xFF; 2048]);
            }
            _ => return,
        }
        let SectionLightData::RawData(byte_data) = &mut self.0 else {
            unreachable!();
        };
        let value_index = (y << 8) | (z << 4) | x;
        let byte = &mut byte_data[value_index / 2];
        match value_index % 2 {
            0 => {
                *byte &= 0xF0;
                *byte |= level;
            }
            _ => {
                *byte &= 0x0F;
                *byte |= level << 4;
            }
        }
    }
}
