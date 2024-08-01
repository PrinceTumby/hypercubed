#![warn(clippy::all)]

pub mod basic_types;
pub mod client;
pub mod protocol;
pub mod resource;
pub mod world;

use std::sync::Arc;

const SERVER_ADDRESS: &str = "localhost";
const SERVER_PORT: u16 = 25565;

fn main() -> anyhow::Result<()> {
    use protocol::v765::configuration;
    env_logger::init();
    #[cfg(feature = "tracy")]
    {
        use tracing_subscriber::layer::SubscriberExt;
        tracing::subscriber::set_global_default(
            tracing_subscriber::registry().with(tracing_tracy::TracyLayer::default())
        ).unwrap();
    }
    println!("{}", request_status(765, SERVER_ADDRESS, SERVER_PORT)?);
    let (server_connection, login_success_packet) = login(
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
    // TODO: Refactor this:
    // - Change PlayConnection to have an internal thread sending packets on a channel
    // - Make read_packet pull from the internal channel receiver
    // Then we no longer need an Arc<PlayConnection> with a confusing read_packet method, and we
    // can more easily make a try_read_packet method
    let server_connection = Arc::new(server_connection);
    let (clientbound_tx, clientbound_rx) = std::sync::mpsc::channel();
    {
        let server_connection = server_connection.clone();
        std::thread::spawn(move || loop {
            let packet = server_connection.read_packet()
                .map_err(|err| format!("{err:.02X?}"))
                .unwrap();
            if let Err(_) = clientbound_tx.send(packet) {
                break;
            };
        });
    }
    pollster::block_on(client::window_run(server_connection, clientbound_rx))?;
    Ok(())
}

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
