use super::super::super::prelude::*;
use nom::branch::alt;
use nom::combinator::verify;
use nom::multi::count;
use nom::sequence::tuple;
use nom::Parser;
use nom_supreme::tag::complete::tag;

#[derive(Clone, Debug)]
pub struct Recipe {
    // TODO Make identifier
    pub id: String,
    pub data: RecipeData,
}

#[derive(Clone, Debug)]
pub enum RecipeData {
    Shapeless(ShapelessRecipe),
    Shaped(ShapedRecipe),
    SpecialArmorDye(Category),
    SpecialBookCloning(Category),
    SpecialMapCloning(Category),
    SpecialMapExtending(Category),
    SpecialFireworkRocket(Category),
    SpecialFireworkStar(Category),
    SpecialFireworkStarFade(Category),
    SpecialRepairItem(Category),
    SpecialTippedArrow(Category),
    SpecialBannerDuplicate(Category),
    SpecialShieldDecoration(Category),
    SpecialShulkerBoxColoring(Category),
    SpecialSuspiciousStew(Category),
    DecoratedPot(Category),
    Smelting(HeatRecipe),
    Blasting(HeatRecipe),
    Smoking(HeatRecipe),
    CampfireCooking(HeatRecipe),
    Stonecutting(StonecuttingRecipe),
    SmithingTransform(SmithingTransformRecipe),
    SmithingTrim(SmithingTrimRecipe),
}

impl Deserialize for Recipe {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        // TODO Make identifier
        macro_rules! recipe_parser {
            ($( $tag_value:expr => ($deserializer:expr, $data_func:expr) $(,)? )+) => {{
                alt(( $( tuple((
                    verify(VarInt::deserialize, |VarInt(value)| *value == $tag_value.len() as i32),
                    tag($tag_value.as_slice()),
                    String::deserialize,
                    $deserializer,
                )).map(|(_len, _type, id, data)| Recipe { id, data: $data_func(data) }), )+ ))
            }}
        }
        // `alt` only handles tuples up to 21 elements, so split into 2 groups
        alt((
            recipe_parser!(
                b"minecraft:crafting_shapeless" =>
                    (ShapelessRecipe::deserialize, RecipeData::Shapeless),
                b"minecraft:crafting_shaped" => (ShapedRecipe::deserialize, RecipeData::Shaped),
                b"minecraft:crafting_special_armordye" =>
                    (Category::deserialize, RecipeData::SpecialArmorDye),
                b"minecraft:crafting_special_bookcloning" =>
                    (Category::deserialize, RecipeData::SpecialBookCloning),
                b"minecraft:crafting_special_mapcloning" =>
                    (Category::deserialize, RecipeData::SpecialMapCloning),
                b"minecraft:crafting_special_mapextending" =>
                    (Category::deserialize, RecipeData::SpecialMapExtending),
                b"minecraft:crafting_special_firework_rocket" =>
                    (Category::deserialize, RecipeData::SpecialFireworkRocket),
                b"minecraft:crafting_special_firework_star" =>
                    (Category::deserialize, RecipeData::SpecialFireworkStar),
                b"minecraft:crafting_special_firework_star_fade" =>
                    (Category::deserialize, RecipeData::SpecialFireworkStarFade),
                b"minecraft:crafting_special_repairitem" =>
                    (Category::deserialize, RecipeData::SpecialRepairItem),
                b"minecraft:crafting_special_tippedarrow" =>
                    (Category::deserialize, RecipeData::SpecialTippedArrow),
            ),
            recipe_parser!(
                b"minecraft:crafting_special_bannerduplicate" =>
                    (Category::deserialize, RecipeData::SpecialBannerDuplicate),
                b"minecraft:crafting_special_shielddecoration" =>
                    (Category::deserialize, RecipeData::SpecialShieldDecoration),
                b"minecraft:crafting_special_shulkerboxcoloring" =>
                    (Category::deserialize, RecipeData::SpecialShulkerBoxColoring),
                b"minecraft:crafting_special_suspiciousstew" =>
                    (Category::deserialize, RecipeData::SpecialSuspiciousStew),
                b"minecraft:crafting_decorated_pot" =>
                    (Category::deserialize, RecipeData::DecoratedPot),
                b"minecraft:smelting" => (HeatRecipe::deserialize, RecipeData::Smelting),
                b"minecraft:blasting" => (HeatRecipe::deserialize, RecipeData::Blasting),
                b"minecraft:smoking" => (HeatRecipe::deserialize, RecipeData::Smoking),
                b"minecraft:campfire_cooking" =>
                    (HeatRecipe::deserialize, RecipeData::CampfireCooking),
                b"minecraft:stonecutting" =>
                    (StonecuttingRecipe::deserialize, RecipeData::Stonecutting),
                b"minecraft:smithing_transform" =>
                    (SmithingTransformRecipe::deserialize, RecipeData::SmithingTransform),
                b"minecraft:smithing_trim" =>
                    (SmithingTrimRecipe::deserialize, RecipeData::SmithingTrim),
            ),
        ))(input)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct ShapelessRecipe {
    pub group: String,
    pub category: Category,
    pub ingredients: Vec<Ingredient>,
    pub result: Slot,
}

#[derive(Clone, Debug)]
pub struct ShapedRecipe {
    pub width: i32,
    pub height: i32,
    pub group: String,
    pub category: Category,
    pub ingredients: Vec<Ingredient>,
    pub result: Slot,
    pub show_notification: bool,
}

impl Deserialize for ShapedRecipe {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        let (rest, (VarInt(width), VarInt(height), group, category)) = tuple((
            VarInt::deserialize,
            VarInt::deserialize,
            String::deserialize,
            Category::deserialize,
        ))(input)?;
        let (rest, ingredients) = count(Ingredient::deserialize, (width * height) as usize)(rest)?;
        let (rest, (result, show_notification)) =
            tuple((Slot::deserialize, bool::deserialize))(rest)?;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Building = 0,
    Redstone = 1,
    Equipment = 2,
    Misc = 3,
}

impl Deserialize for Category {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        verify(VarInt::deserialize, |VarInt(value)| {
            0 <= *value && *value <= 3
        })
        .map(|VarInt(value)| match value {
            0 => Self::Building,
            1 => Self::Redstone,
            2 => Self::Equipment,
            3 => Self::Misc,
            _ => unreachable!(),
        })
        .parse(input)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeatCategory {
    Food = 0,
    Blocks = 1,
    Misc = 2,
}

impl Deserialize for HeatCategory {
    fn deserialize(input: &[u8]) -> IResult<&[u8], Self> {
        verify(VarInt::deserialize, |VarInt(value)| {
            0 <= *value && *value <= 2
        })
        .map(|VarInt(value)| match value {
            0 => Self::Food,
            1 => Self::Blocks,
            2 => Self::Misc,
            _ => unreachable!(),
        })
        .parse(input)
    }
}

pub type Ingredient = Vec<Slot>;

pub type Slot = Option<PresentSlot>;

#[derive(Clone, Debug, Deserialize)]
pub struct PresentSlot {
    pub id: VarInt,
    pub count: u8,
    pub nbt: OptionalNbt,
}
