#![warn(clippy::all)]

pub mod basic_types;
pub mod client;
pub mod protocol;
pub mod resource;
pub mod world;

use protocol::v765::prelude::*;

const SERVER_ADDRESS: &str = "localhost";
const SERVER_PORT: u16 = 25565;

fn main() -> anyhow::Result<()> {
    use protocol::v765::configuration;
    env_logger::init();
    println!("{}", request_status(765, SERVER_ADDRESS, SERVER_PORT)?);
    let (mut connection, login_success_packet) = login(
        SERVER_ADDRESS,
        SERVER_PORT,
        configuration::ClientInformation {
            locale: "en_GB",
            view_distance: 8,
            chat_mode: configuration::ChatMode::Enabled,
            chat_colors_enabled: true,
            displayed_skin_parts: 0x7F,
            main_hand: configuration::MainHand::Right,
            text_filtering_enabled: false,
            server_listings_allowed: true,
        },
    )?;
    println!("{login_success_packet:?}");
    let mut test_chunks = ahash::AHashMap::new();
    loop {
        use protocol::v765::play::{serverbound, Clientbound};
        match connection.read_packet()? {
            Clientbound::ErrorDisconnect { reason } => println!("Disconnected: {reason}"),
            Clientbound::BundleDelimiter => unreachable!(),
            Clientbound::LoginPlay(_packet) => println!("Login play: {{<skipped>}}"),
            Clientbound::UpdateRecipes(_recipes) => println!("Update recipes: [<skipped>]"),
            Clientbound::UpdateTags(_tags) => println!("Update tags: [<skipped>]"),
            Clientbound::DeclareCommands(_) => println!("Declare commands: [<todo>]"),
            Clientbound::UpdateRecipeBook(_) => println!("Update recipes: {{<skipped>}}"),
            Clientbound::ServerData(data) => println!("Server MOTD: {:?}", data.motd),
            Clientbound::ChunkDataAndUpdateLight(data) => {
                println!("Chunk data and light update: {{<skipped>}}");
                let (rest, chunk_sections) = count(ChunkSection::deserialize, 24)(&data.chunk_data)
                    .map_err(|err| err.to_owned())?;
                assert_eq!(rest.len(), 0);
                test_chunks.insert((data.chunk_x, data.chunk_z), chunk_sections);
                if test_chunks.len() >= 256 {
                    break;
                }
            }
            Clientbound::ChunkBatchStart => println!("Chunk batch started"),
            Clientbound::ChunkBatchEnd { num_chunks } => {
                println!("Chunk batch ended, received {num_chunks} chunks");
                connection.send_packet(serverbound::ChunkBatchReceived {
                    desired_chunks_per_tick: 8.0,
                })?;
            }
            other => println!("{other:?}"),
        }
    }
    drop(connection);
    pollster::block_on(client::window_run(test_chunks))?;
    Ok(())
}

// TODO We've saved a test chunk, get it rendering

// fn main() -> anyhow::Result<()> {
//     env_logger::init();
//     // tracing_subscriber::fmt::fmt()
//     //     .with_max_level(tracing::Level::TRACE)
//     //     .pretty()
//     //     .init();
//     pollster::block_on(client::window_run(ahash::AHashMap::new()))
// }

use nom::multi::count;
use nom::sequence::pair;
use nom::Parser;
use nom_supreme::tag::complete::tag;
use protocol::v765::prelude::*;
use resource::block::GlobalPaletteIndex;

#[derive(Clone, Deserialize)]
struct ChunkSection {
    pub block_count: u16,
    pub block_states: PalettedContainer<16, 8>,
    pub biomes: PalettedContainer<4, 3>,
}

#[derive(Clone)]
struct PalettedContainer<const AXIS_LEN: usize, const INDIRECT_MAX_BPE: u8> {
    bits_per_entry: u8,
    palette: Palette,
    data: Vec<u64>,
}

impl<const AXIS_LEN: usize, const INDIRECT_MAX_BPE: u8>
    PalettedContainer<AXIS_LEN, INDIRECT_MAX_BPE>
{
    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> GlobalPaletteIndex {
        if let &Palette::SingleValue(value) = &self.palette {
            return value;
        }
        let entries_per_long = 64 / self.bits_per_entry as usize;
        let num_longs = AXIS_LEN.pow(3) / entries_per_long;
        let extra_long = AXIS_LEN.pow(3) % entries_per_long > 0;
        assert_eq!(self.data.len(), num_longs + extra_long as usize);
        let entry_idx = (y * AXIS_LEN.pow(2)) + (z * AXIS_LEN) + x;
        let long_idx = entry_idx / entries_per_long;
        let bitshift = ((entry_idx % entries_per_long) * self.bits_per_entry as usize) as u64;
        let entry_bitmask: u64 = (1 << self.bits_per_entry as u64) - 1;
        let entry_value: u64 = (self.data[long_idx] >> bitshift) & entry_bitmask;
        match &self.palette {
            Palette::Palette(entries) => entries[usize::try_from(entry_value).unwrap()],
            Palette::Direct => entry_value.try_into().unwrap(),
            _ => unreachable!(),
        }
    }

    pub fn palette(&self) -> &Palette {
        &self.palette
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
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        let (rest, bits_per_entry) = u8::deserialize(input)?;
        match bits_per_entry {
            0 => {
                pair(
                    VarInt::deserialize.map(|VarInt(value)| value.try_into().unwrap()),
                    // Length of `data`, zero because there's no data needed for a single value
                    // palette
                    tag(&[0]),
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
            .map(|(palette_entries, data)| Self {
                bits_per_entry,
                palette: Palette::Palette(palette_entries),
                data,
            })
            .parse(rest),
            _ => <Vec<u64>>::deserialize
                .map(|data| Self {
                    bits_per_entry,
                    palette: Palette::Direct,
                    data,
                })
                .parse(rest),
        }
    }
}

#[derive(Clone, Debug)]
enum Palette {
    SingleValue(GlobalPaletteIndex),
    Palette(Vec<GlobalPaletteIndex>),
    Direct,
}

// fn main() -> anyhow::Result<()> {
//     // use protocol::v765::play::ChunkDataAndUpdateLight;
//     // let test_chunk_json = std::fs::read_to_string("test_chunk.json")?;
//     // let test_chunk = serde_json::from_str::<ChunkDataAndUpdateLight>(&test_chunk_json)?;
//     let test_chunk_data_json = std::fs::read_to_string("test_chunk_data.json")?;
//     let test_chunk_data: Vec<u8> = serde_json::from_str(&test_chunk_data_json)?;
//     let (rest, chunk_sections) =
//         count(ChunkSection::deserialize, 24)(&test_chunk_data).map_err(|err| err.to_owned())?;
//     assert_eq!(rest.len(), 0);
//     // for chunk_section in &chunk_sections {
//     //     println!("{}", chunk_section.block_states);
//     // }
//     env_logger::init();
//     pollster::block_on(client::window_run(chunk_sections))?;
//     // TODO Get this test chunk rendering
//     Ok(())
// }
