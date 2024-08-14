use super::prelude::*;
use crate::resource::block::GlobalPaletteIndex;
use nom::sequence::pair;
use nom::Parser;
use nom_supreme::tag::complete::tag;
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
                    todo!();
                }
            },
            Palette::Palette(ref mut palette) => match palette.iter().position(|&v| v == value) {
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
                        Palette::Palette(ref palette) => {
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
                            let bit_value: u64 = value.as_raw().try_into().unwrap();
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
                let bit_value: u64 = value.as_raw().try_into().unwrap();
                let old_bit_value =
                    Self::replace_raw(&mut self.data, self.bits_per_entry, x, y, z, bit_value);
                old_bit_value.try_into().unwrap()
            }
        }
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
            (Palette::Palette(ref palette), false) => {
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
                                palette_entry.as_raw().try_into().unwrap(),
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

impl<const AXIS_LEN: usize, const INDIRECT_MAX_BPE: u8> std::fmt::Display
    for PalettedContainer<AXIS_LEN, INDIRECT_MAX_BPE>
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use std::fmt::Write;
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
                    tag(std::slice::from_ref(&0)),
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
                let extra_long = AXIS_LEN.pow(3) % entries_per_long > 0;
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
                    let extra_long = AXIS_LEN.pow(3) % entries_per_long > 0;
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
