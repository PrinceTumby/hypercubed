use crate::portable_prelude::*;
use crate::protocol::configuration::Property as GameProfileProperty;
use crate::protocol::prelude::*;
use nom::Parser;
use nom::multi::count;
use nom::sequence::pair;
use portable_std::FastHashMap;
use protocol_derive::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct Recipe {
    pub id: Identifier,
    pub data: RecipeData,
}

#[derive(Clone, Debug, Deserialize)]
#[repr(u8)]
pub enum RecipeData {
    Shaped(ShapedRecipe) = 0,
    Shapeless(ShapelessRecipe) = 1,
    SpecialArmorDye(Category) = 2,
    SpecialBookCloning(Category) = 3,
    SpecialMapCloning(Category) = 4,
    SpecialMapExtending(Category) = 5,
    SpecialFireworkRocket(Category) = 6,
    SpecialFireworkStar(Category) = 7,
    SpecialFireworkStarFade(Category) = 8,
    SpecialRepairItem(Category) = 9,
    SpecialTippedArrow(Category) = 10,
    SpecialBannerDuplicate(Category) = 11,
    SpecialShieldDecoration(Category) = 12,
    SpecialShulkerBoxColoring(Category) = 13,
    SpecialSuspiciousStew(Category) = 14,
    Smelting(HeatRecipe) = 15,
    Blasting(HeatRecipe) = 16,
    Smoking(HeatRecipe) = 17,
    CampfireCooking(HeatRecipe) = 18,
    Stonecutting(StonecuttingRecipe) = 19,
    SmithingTransform(SmithingTransformRecipe) = 20,
    SmithingTrim(SmithingTrimRecipe) = 21,
    DecoratedPot(Category) = 22,
}

#[derive(Clone, Debug)]
pub struct ShapedRecipe {
    pub group: String,
    pub category: Category,
    pub width: i32,
    pub height: i32,
    pub ingredients: Vec<Ingredient>,
    pub result: Slot,
    pub show_notification: bool,
}

impl Deserialize for ShapedRecipe {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        let (rest, (group, category, VarInt(width), VarInt(height))) = (
            String::deserialize,
            Category::deserialize,
            VarInt::deserialize,
            VarInt::deserialize,
        )
            .parse(input)?;
        let (rest, ingredients) =
            count(Ingredient::deserialize, (width * height) as usize).parse(rest)?;
        let (rest, (result, show_notification)) =
            (Slot::deserialize, bool::deserialize).parse(rest)?;
        Ok((
            rest,
            Self {
                width,
                height,
                group,
                category,
                ingredients,
                result,
                show_notification,
            },
        ))
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct ShapelessRecipe {
    pub group: String,
    pub category: Category,
    pub ingredients: Vec<Ingredient>,
    pub result: Slot,
}

#[derive(Clone, Debug, Deserialize)]
pub struct HeatRecipe {
    pub group: String,
    pub category: HeatCategory,
    pub ingredient: Ingredient,
    pub result: Slot,
    pub experience: f32,
    pub cooking_time: VarInt,
}

#[derive(Clone, Debug, Deserialize)]
pub struct StonecuttingRecipe {
    pub group: String,
    pub ingredient: Ingredient,
    pub result: Slot,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SmithingTransformRecipe {
    pub template: Ingredient,
    pub base: Ingredient,
    pub addition: Ingredient,
    pub result: Slot,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SmithingTrimRecipe {
    pub template: Ingredient,
    pub base: Ingredient,
    pub addition: Ingredient,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum Category {
    Building = 0,
    Redstone = 1,
    Equipment = 2,
    Misc = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum HeatCategory {
    Food = 0,
    Blocks = 1,
    Misc = 2,
}

pub type Ingredient = Vec<Slot>;

#[derive(Clone, Debug)]
pub struct Slot {
    pub count: u32,
    pub info: Option<SlotInfo>,
}

impl Deserialize for Slot {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        let (rest, VarInt(count)) = nom_context("Slot::count", VarInt::deserialize).parse(input)?;
        let count: u32 = count.try_into().unwrap();
        match count {
            0 => Ok((rest, Self { count, info: None })),
            _ => {
                let (rest, slot_info) =
                    nom_context("Slot::info", SlotInfo::deserialize).parse(rest)?;
                Ok((
                    rest,
                    Self {
                        count,
                        info: Some(slot_info),
                    },
                ))
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct SlotInfo {
    pub id: VarInt,
    pub add_components: Vec<Component>,
    pub remove_components: Vec<VarInt>,
}

impl Deserialize for SlotInfo {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        fn components_flat_map<'a>(
            (id, num_add_components, num_remove_components): (VarInt, VarInt, VarInt),
        ) -> impl Parser<InputSpan<'a>, Output = SlotInfo, Error = IErr<'a>> {
            pair(
                nom_context(
                    "Component::add_components",
                    count(
                        Component::deserialize,
                        num_add_components.0.try_into().unwrap(),
                    ),
                ),
                nom_context(
                    "Component::remove_components",
                    count(
                        VarInt::deserialize,
                        num_remove_components.0.try_into().unwrap(),
                    ),
                ),
            )
            .map(move |(add_components, remove_components)| SlotInfo {
                id,
                add_components,
                remove_components,
            })
        }
        nom_context(
            "SlotInfo",
            (
                nom_context("SlotInfo::id", VarInt::deserialize),
                nom_context("SlotInfo::num_add_components", VarInt::deserialize),
                nom_context("SlotInfo::num_remove_components", VarInt::deserialize),
            ),
        )
        .flat_map(components_flat_map)
        .parse(input)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[repr(i32)]
pub enum Component {
    CustomData(NetworkNbt) = 0,
    MaxStackSize(VarInt) = 1,
    MaxDamage(VarInt) = 2,
    Damage(VarInt) = 3,
    Unbreakable {
        show_in_tooltip: bool,
    } = 4,
    CustomName(TextComponent) = 5,
    ItemName(TextComponent) = 6,
    Lore(Vec<TextComponent>) = 7,
    Rarity(ItemRarity) = 8,
    Enchantments {
        enchantments: Vec<ItemEnchantment>,
        show_in_tooltip: bool,
    } = 9,
    CanPlaceOn {
        predicates: Vec<BlockPredicate>,
        show_in_tooltip: bool,
    } = 10,
    CanBreak {
        predicates: Vec<BlockPredicate>,
        show_in_tooltip: bool,
    } = 11,
    AttributeModifiers {
        modifiers: Vec<AttributeModifier>,
        show_in_tooltip: bool,
    } = 12,
    CustomModelData {
        value: VarInt,
    } = 13,
    HideAdditionalToolTip = 14,
    HideToolTip = 15,
    RepairCost(VarInt) = 16,
    CreativeSlotLock = 17,
    EnchantmentGlintOverride(bool) = 18,
    IntangibleProjectile = 19,
    Food {
        nutrition: VarInt,
        saturation_modifier: f32,
        can_always_eat: bool,
        seconds_to_eat: f32,
        using_converts_to: Box<SlotInfo>,
        effects: Vec<PotionEffectChance>,
    } = 20,
    FireResistant = 21,
    Tool {
        rules: Vec<ToolRule>,
        default_mining_speed: f32,
        damage_per_block: VarInt,
    } = 22,
    StoredEnchantments {
        enchantments: Vec<ItemEnchantment>,
        show_in_tooltip: bool,
    } = 23,
    DyedColor {
        rgba: [u8; 4],
        show_in_tooltip: bool,
    } = 24,
    MapColor([u8; 4]) = 25,
    MapId(VarInt) = 26,
    MapDecorations(NetworkNbt) = 27,
    MapPostProcessing {
        _ty: VarInt,
    } = 28,
    ChargedProjectiles(Vec<Slot>) = 29,
    BundleContents(Vec<Slot>) = 30,
    PotionContents {
        potion_id: Option<VarInt>,
        custom_color: Option<[u8; 4]>,
        custom_effects: Vec<PotionEffect>,
    } = 31,
    SuspiciousStewEffects(Vec<SuspiciousStewEffect>) = 32,
    WritableBookContent(Vec<BookPage>) = 33,
    WrittenBookContent {
        raw_title: String,
        filtered_title: Option<String>,
        author: String,
        generation: VarInt,
        pages: Vec<BookPage>,
        entity_selectors_resolved: bool,
    } = 34,
    ArmorTrim {
        material: TrimMaterial,
        pattern: TrimPattern,
        show_in_tooltip: bool,
    } = 35,
    DebugStickState(NetworkNbt) = 36,
    EntityData(NetworkNbt) = 37,
    BucketEntityData(NetworkNbt) = 38,
    BlockEntityData(NetworkNbt) = 39,
    Instrument(InstrumentInfo) = 40,
    OminousBottleAmplifier(VarInt) = 41,
    JukeboxPlayable(JukeboxSong) = 42,
    RecipeUnlocks(NetworkNbt) = 43,
    LodestoneTracker {
        global_position: GlobalPosition,
        is_tracked: bool,
    } = 44,
    FireworkExplosion(FireworkExplosion) = 45,
    Firework {
        flight_duration: VarInt,
        explosions: Vec<FireworkExplosion>,
    } = 46,
    PlayerHeadProfile {
        name: Option<String>,
        uuid: Option<Uuid>,
        properties: Vec<GameProfileProperty>,
    } = 47,
    NoteBlockSound(Identifier) = 48,
    BannerPatterns(Vec<BannerPatternLayer>) = 49,
    BaseColor(DyeColor) = 50,
    PotDecorations(Vec<VarInt>) = 51,
    ContainerItems(Vec<Slot>) = 52,
    BlockState {
        properties: FastHashMap<String, String>,
    } = 53,
    HiveBees(Vec<HiveBee>) = 54,
    ContainerLock(NetworkNbt) = 55,
    ContainerLoot(NetworkNbt) = 56,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum ItemRarity {
    Common = 0,
    Uncommon = 1,
    Rare = 2,
    Epic = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub struct ItemEnchantment {
    pub id: VarInt,
    pub level: VarInt,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct BlockPredicate {
    pub blocks: Option<BlockPredicateBlockSet>,
    pub properties: Option<Vec<BlockPredicateProperty>>,
    pub nbt_data: Option<NetworkNbt>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockPredicateBlockSet {
    Direct(Vec<VarInt>),
    Tagged(Identifier),
}

impl Deserialize for BlockPredicateBlockSet {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        let (rest, type_and_len) = VarInt::deserialize(input)?;
        match type_and_len.0 {
            0 => nom_context("BlockPredicateBlockSet", Identifier::deserialize)
                .map(Self::Tagged)
                .parse(rest),
            len => {
                let num_ids: usize = (len - 1).try_into().unwrap();
                nom_context(
                    "BlockPredicateBlockSet",
                    count(VarInt::deserialize, num_ids),
                )
                .map(Self::Direct)
                .parse(rest)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct BlockPredicateProperty {
    pub name: String,
    pub match_data: BlockPredicatePropertyMatch,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[repr(u8)]
pub enum BlockPredicatePropertyMatch {
    Ranged { min: String, max: String } = 0,
    Exact(String) = 1,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct AttributeModifier {
    pub attribute_id: VarInt,
    pub unique_id: Uuid,
    pub name: String,
    pub value: f64,
    pub operation: AttributeModifierOperation,
    pub required_slot: AttributeModifierSlot,
    pub show_in_tooltip: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum AttributeModifierOperation {
    Add = 0,
    MultiplyBase = 1,
    MultiplyTotal = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub enum AttributeModifierSlot {
    Any = 0,
    MainHand = 1,
    OffHand = 2,
    AnyHand = 3,
    Feet = 4,
    Legs = 5,
    Chest = 6,
    Head = 7,
    AnyArmour = 8,
    AnyNotHead = 9,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct PotionEffectChance {
    pub effect: PotionEffect,
    pub probability: f32,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct PotionEffect {
    pub id: VarInt,
    pub info: PotionEffectInfo,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct PotionEffectInfo {
    pub amplifier: VarInt,
    pub duration: VarInt,
    pub make_particles_translucent: bool,
    pub show_particles: bool,
    pub show_in_inventory: bool,
    pub hidden_effect: Option<Box<PotionEffect>>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ToolRule {
    pub blocks: BlockPredicateBlockSet,
    pub speed: Option<f32>,
    pub correct_drop_for_blocks: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
pub struct SuspiciousStewEffect {
    pub id: VarInt,
    pub duration: VarInt,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct BookPage {
    pub raw_content: String,
    pub filtered_content: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TrimMaterial {
    Direct {
        asset_name: String,
        ingredient: VarInt,
        item_model_index: f32,
        overrides: Vec<TrimMaterialOverride>,
        description: TextComponent,
    },
    Registry(VarInt),
}

impl Deserialize for TrimMaterial {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        let (rest, reg_id) = VarInt::deserialize(input)?;
        match reg_id.0 {
            0 => nom_context(
                "TrimMaterial",
                <(
                    String,
                    VarInt,
                    f32,
                    Vec<TrimMaterialOverride>,
                    TextComponent,
                )>::deserialize,
            )
            .map(
                |(asset_name, ingredient, item_model_index, overrides, description)| Self::Direct {
                    asset_name,
                    ingredient,
                    item_model_index,
                    overrides,
                    description,
                },
            )
            .parse(rest),
            registry_id => Ok((rest, Self::Registry(VarInt(registry_id - 1)))),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct TrimMaterialOverride {
    pub material_type: VarInt,
    pub overriden_asset_name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TrimPattern {
    Direct {
        asset_name: String,
        template_item: VarInt,
        description: TextComponent,
        is_decal: bool,
    },
    Registry(VarInt),
}

impl Deserialize for TrimPattern {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        let (rest, reg_id) = VarInt::deserialize(input)?;
        match reg_id.0 {
            0 => nom_context(
                "TrimPattern",
                <(String, VarInt, TextComponent, bool)>::deserialize,
            )
            .map(
                |(asset_name, template_item, description, is_decal)| Self::Direct {
                    asset_name,
                    template_item,
                    description,
                    is_decal,
                },
            )
            .parse(rest),
            registry_id => Ok((rest, Self::Registry(VarInt(registry_id - 1)))),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum InstrumentInfo {
    Direct {
        sound_event: SoundEvent,
        use_duration: f32,
        range: f32,
    },
    Registry(VarInt),
}

impl Deserialize for InstrumentInfo {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        let (rest, reg_id) = VarInt::deserialize(input)?;
        match reg_id.0 {
            0 => nom_context("InstrumentInfo", <(SoundEvent, f32, f32)>::deserialize)
                .map(|(sound_event, use_duration, range)| Self::Direct {
                    sound_event,
                    use_duration,
                    range,
                })
                .parse(rest),
            registry_id => Ok((rest, Self::Registry(VarInt(registry_id - 1)))),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum SoundEvent {
    Direct {
        name: Identifier,
        fixed_range: Option<f32>,
    },
    Registry(VarInt),
}

impl Deserialize for SoundEvent {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        let (rest, reg_id) = VarInt::deserialize(input)?;
        match reg_id.0 {
            0 => nom_context("SoundEvent", <(Identifier, Option<f32>)>::deserialize)
                .map(|(name, fixed_range)| Self::Direct { name, fixed_range })
                .parse(rest),
            registry_id => Ok((rest, Self::Registry(VarInt(registry_id - 1)))),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[repr(u8)]
pub enum JukeboxSong {
    Named(Identifier) = 0,
    DirectOrRegistry(JukeboxSongInfo) = 1,
}

#[derive(Clone, Debug, PartialEq)]
pub enum JukeboxSongInfo {
    Direct {
        sound_event: SoundEvent,
        description: TextComponent,
        duration: f32,
        comparator_output_strength: u8,
    },
    Registry(VarInt),
}

impl Deserialize for JukeboxSongInfo {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        let (rest, reg_id) = VarInt::deserialize(input)?;
        match reg_id.0 {
            0 => nom_context(
                "JukeboxSongInfo",
                <(SoundEvent, TextComponent, f32, u8)>::deserialize,
            )
            .map(
                |(sound_event, description, duration, comparator_output_strength)| Self::Direct {
                    sound_event,
                    description,
                    duration,
                    comparator_output_strength,
                },
            )
            .parse(rest),
            registry_id => Ok((rest, Self::Registry(VarInt(registry_id - 1)))),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct FireworkExplosion {
    shape: VarInt,
    colors: Vec<[u8; 4]>,
    fade_colors: Vec<[u8; 4]>,
    has_trail: bool,
    has_twinkle: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct BannerPatternLayer {
    pub info: BannerPatternLayerInfo,
    pub color: DyeColor,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BannerPatternLayerInfo {
    Direct {
        asset_id: Identifier,
        translation_key: String,
    },
    Registry(VarInt),
}

impl Deserialize for BannerPatternLayerInfo {
    fn deserialize(input: InputSpan) -> IResult<Self> {
        let (rest, reg_id) = VarInt::deserialize(input)?;
        match reg_id.0 {
            0 => nom_context(
                "BannerPatternLayerInfo",
                <(Identifier, String)>::deserialize,
            )
            .map(|(asset_id, translation_key)| Self::Direct {
                asset_id,
                translation_key,
            })
            .parse(rest),
            registry_id => Ok((rest, Self::Registry(VarInt(registry_id - 1)))),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[repr(u8)]
pub enum DyeColor {
    White = 0,
    Orange = 1,
    Magenta = 2,
    LightBlue = 3,
    Yellow = 4,
    Lime = 5,
    Pink = 6,
    Gray = 7,
    LightGray = 8,
    Cyan = 9,
    Purple = 10,
    Blue = 11,
    Brown = 12,
    Green = 13,
    Red = 14,
    Black = 15,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct HiveBee {
    pub entity_data: NetworkNbt,
    pub ticks_in_hive: VarInt,
    pub min_ticks_in_hive: VarInt,
}
