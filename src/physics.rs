use crate::client::{Player, RawChunk, MIN_HEIGHT_I32, SUBCHUNK_AXIS_LEN_I32};
use crate::protocol::chunk::ChunkSection;
use crate::resource::block::blockstate::CollisionInfo;
use crate::resource::block::{Registry as BlockRegistry, RightAngleRotation};
use ahash::AHashMap;
use nalgebra::{Point3, Rotation3, Vector3};
use serde::Deserialize;
use std::sync::Arc;

// Most of this implementation was ported from PrismarineJS's physics implementation.

// FIXME: Running speed is too fast, these constants probably just need fixing.
const GRAVITY: f32 = 0.08;
const AIR_DRAG_COEF: f32 = 1.0 - 0.02;
const BASE_PLAYER_SPEED: f32 = 0.1;
const SNEAK_SPEED: f32 = 0.3;
const AIR_ACCELERATION: f32 = 0.02;
const AIR_INERTIA: f32 = 0.91;
const STEP_HEIGHT: f32 = 0.6;
const NEGLIGIBLE_VELOCITY: f32 = 0.003;
const AUTO_JUMP_COOLDOWN_TICKS: u32 = 10;
// TODO: Check this is accurate to vanilla
const MINOR_COLLISION_ANGLE_THRESHOLD: f32 = 8.0;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub struct AABB {
    pub corner_1: Point3<f32>,
    pub corner_2: Point3<f32>,
}

impl AABB {
    pub fn apply_blockstate_rotations(
        &mut self,
        x_rotation: RightAngleRotation,
        y_rotation: RightAngleRotation,
    ) {
        let x_y_axis_angles = [
            (Vector3::x_axis(), x_rotation),
            (Vector3::y_axis(), y_rotation),
        ];
        let [x_blockstate_rot, y_blockstate_rot] =
            x_y_axis_angles.map(|(axis, angle)| match angle {
                RightAngleRotation::Zero => Rotation3::identity(),
                RightAngleRotation::Ninety => {
                    Rotation3::from_axis_angle(&axis, -std::f32::consts::FRAC_PI_2)
                }
                RightAngleRotation::OneEighty => {
                    Rotation3::from_axis_angle(&axis, std::f32::consts::PI)
                }
                RightAngleRotation::TwoSeventy => {
                    Rotation3::from_axis_angle(&axis, std::f32::consts::FRAC_PI_2)
                }
            });
        let xy_blockstate_rot = (y_blockstate_rot * x_blockstate_rot).to_homogeneous();
        let complete_mat = xy_blockstate_rot
            .prepend_translation(&Vector3::new(-0.5, -0.5, -0.5))
            .append_translation(&Vector3::new(0.5, 0.5, 0.5));
        let [p1, p2] = [self.corner_1, self.corner_2].map(|p| complete_mat.transform_point(&p));
        fn quantise_256(n: f32) -> f32 {
            (n * 256.0).round() / 256.0
        }
        self.corner_1 = Point3::new(
            quantise_256(f32::min(p1.x, p2.x)),
            quantise_256(f32::min(p1.y, p2.y)),
            quantise_256(f32::min(p1.z, p2.z)),
        );
        self.corner_2 = Point3::new(
            quantise_256(f32::max(p1.x, p2.x)),
            quantise_256(f32::max(p1.y, p2.y)),
            quantise_256(f32::max(p1.z, p2.z)),
        );
    }

    pub fn intersects(&self, other: &Self) -> bool {
        self.corner_1.x < other.corner_2.x
            && self.corner_2.x > other.corner_1.x
            && self.corner_1.y < other.corner_2.y
            && self.corner_2.y > other.corner_1.y
            && self.corner_1.z < other.corner_2.z
            && self.corner_2.z > other.corner_1.z
    }

    pub fn extended_in_direction(&self, dir: Vector3<f32>) -> Self {
        let mut out = *self;
        if dir.x < 0.0 {
            out.corner_1.x += dir.x;
        } else {
            out.corner_2.x += dir.x;
        }
        if dir.y < 0.0 {
            out.corner_1.y += dir.y;
        } else {
            out.corner_2.y += dir.y;
        }
        if dir.z < 0.0 {
            out.corner_1.z += dir.z;
        } else {
            out.corner_2.z += dir.z;
        }
        out
    }

    pub fn expanded_by(&self, dims: Vector3<f32>) -> Self {
        Self {
            corner_1: self.corner_1 - dims,
            corner_2: self.corner_2 + dims,
        }
    }

    pub fn contracted_by(&self, dims: Vector3<f32>) -> Self {
        Self {
            corner_1: self.corner_1 + dims,
            corner_2: self.corner_2 - dims,
        }
    }

    /// Returns the penetration normal and entry time for `self` moving at a given velocity
    /// interacting with `other`, a static collider.
    /// Returns `None` if no collision occurs in the next time frame.
    pub fn get_collision_info(
        &self,
        other: &Self,
        velocity: Vector3<f32>,
    ) -> Option<(Vector3<f32>, f32)> {
        fn entry_exit_times(s1: f32, s2: f32, o1: f32, o2: f32, velocity: f32) -> [f32; 2] {
            if velocity > 0.0 {
                [(o1 - s2) / velocity, (o2 - s1) / velocity]
            } else if velocity == 0.0 {
                [
                    f32::copysign(f32::INFINITY, s1 - o2),
                    f32::copysign(f32::INFINITY, s2 - o1),
                ]
            } else {
                [(o2 - s1) / velocity, (o1 - s2) / velocity]
            }
        }
        let [x_entry, x_exit] = entry_exit_times(
            self.corner_1.x,
            self.corner_2.x,
            other.corner_1.x,
            other.corner_2.x,
            velocity.x,
        );
        let [y_entry, y_exit] = entry_exit_times(
            self.corner_1.y,
            self.corner_2.y,
            other.corner_1.y,
            other.corner_2.y,
            velocity.y,
        );
        let [z_entry, z_exit] = entry_exit_times(
            self.corner_1.z,
            self.corner_2.z,
            other.corner_1.z,
            other.corner_2.z,
            velocity.z,
        );
        // Check if collision occured
        if x_entry < 0.0 && y_entry < 0.0 && z_entry < 0.0 {
            return None;
        }
        if x_entry > 1.0 || y_entry > 1.0 || z_entry > 1.0 {
            return None;
        }
        // Get first collision axis
        let entry = [x_entry, y_entry, z_entry]
            .into_iter()
            .max_by(|a, b| a.total_cmp(b))
            .unwrap();
        let exit = [x_exit, y_exit, z_exit]
            .into_iter()
            .min_by(|a, b| a.total_cmp(b))
            .unwrap();
        if entry > exit {
            return None;
        }
        // Get normal of collision surface
        let normal = if entry == x_entry {
            Vector3::new((1.0_f32).copysign(-velocity.x), 0.0, 0.0)
        } else if entry == y_entry {
            Vector3::new(0.0, (1.0_f32).copysign(-velocity.y), 0.0)
        } else {
            Vector3::new(0.0, 0.0, (1.0_f32).copysign(-velocity.z))
        };
        Some((normal, entry))
    }

    pub fn compute_x_offset(&self, other: &Self, initial_value: f32) -> f32 {
        // Ensure that `self` and `other` intersect in Y and Z axes.
        if other.corner_2.y < self.corner_1.y
            || other.corner_1.y > self.corner_2.y
            || other.corner_2.z < self.corner_1.z
            || other.corner_1.z > self.corner_2.z
        {
            return initial_value;
        }
        if initial_value > 0.0 && other.corner_2.x <= self.corner_1.x {
            initial_value.min(self.corner_1.x - other.corner_2.x)
        } else if initial_value < 0.0 && other.corner_1.x >= self.corner_2.x {
            initial_value.max(self.corner_2.x - other.corner_1.x)
        } else {
            initial_value
        }
    }

    pub fn compute_y_offset(&self, other: &Self, initial_value: f32) -> f32 {
        // Ensure that `self` and `other` intersect in X and Z axes.
        if other.corner_2.x < self.corner_1.x
            || other.corner_1.x > self.corner_2.x
            || other.corner_2.z < self.corner_1.z
            || other.corner_1.z > self.corner_2.z
        {
            return initial_value;
        }
        if initial_value > 0.0 && other.corner_2.y <= self.corner_1.y {
            initial_value.min(self.corner_1.y - other.corner_2.y)
        } else if initial_value < 0.0 && other.corner_1.y >= self.corner_2.y {
            initial_value.max(self.corner_2.y - other.corner_1.y)
        } else {
            initial_value
        }
    }

    pub fn compute_z_offset(&self, other: &Self, initial_value: f32) -> f32 {
        // Ensure that `self` and `other` intersect in X and Y axes.
        if other.corner_2.x < self.corner_1.x
            || other.corner_1.x > self.corner_2.x
            || other.corner_2.y < self.corner_1.y
            || other.corner_1.y > self.corner_2.y
        {
            return initial_value;
        }
        if initial_value > 0.0 && other.corner_2.z <= self.corner_1.z {
            initial_value.min(self.corner_1.z - other.corner_2.z)
        } else if initial_value < 0.0 && other.corner_1.z >= self.corner_2.z {
            initial_value.max(self.corner_2.z - other.corner_1.z)
        } else {
            initial_value
        }
    }
}

impl std::ops::Add<Vector3<f32>> for AABB {
    type Output = Self;

    fn add(self, offset: Vector3<f32>) -> Self {
        Self {
            corner_1: self.corner_1 + offset,
            corner_2: self.corner_2 + offset,
        }
    }
}

impl std::ops::AddAssign<Vector3<f32>> for AABB {
    fn add_assign(&mut self, offset: Vector3<f32>) {
        self.corner_1 += offset;
        self.corner_2 += offset;
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerPhysicsState {
    pub local_aabb: AABB,
    pub velocity: Vector3<f32>,
    pub on_ground: bool,
    pub jump_queued: bool,
    pub jump_ticks: u32,
    pub sprinting: bool,
}

impl Default for PlayerPhysicsState {
    fn default() -> Self {
        Self {
            local_aabb: AABB {
                corner_1: Point3::new(-0.3, 0.0, -0.3),
                corner_2: Point3::new(0.3, 1.8, 0.3),
            },
            velocity: Vector3::zeros(),
            on_ground: true,
            jump_queued: false,
            jump_ticks: 0,
            sprinting: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerInput {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub jump: bool,
    pub sneak: bool,
    pub sprint: bool,
}

pub fn simulate_player(
    global_palette: &BlockRegistry,
    raw_chunks: &AHashMap<[i32; 2], Arc<RawChunk>>,
    player: &mut Player,
    input: &mut PlayerInput,
) {
    let physics = &mut player.physics_state;
    let velocity = &mut physics.velocity;
    let pos = &mut player.pos;
    // Collapse low velocity components to zero
    if velocity.x.abs() < NEGLIGIBLE_VELOCITY {
        velocity.x = 0.0;
    }
    if velocity.y.abs() < NEGLIGIBLE_VELOCITY {
        velocity.y = 0.0;
    }
    if velocity.z.abs() < NEGLIGIBLE_VELOCITY {
        velocity.z = 0.0;
    }
    // Jumping
    if input.jump || physics.jump_queued {
        if physics.jump_ticks > 0 {
            physics.jump_ticks -= 1;
        }
        if physics.on_ground && physics.jump_ticks == 0 {
            physics.jump_ticks = AUTO_JUMP_COOLDOWN_TICKS;
            velocity.y = 0.42;
            if input.sprint {
                velocity.x += player.yaw.to_radians().sin() * 0.2;
                velocity.z -= player.yaw.to_radians().cos() * 0.2;
            }
        }
    } else {
        physics.jump_ticks = 0;
    }
    physics.jump_queued = false;

    // TODO: Sneaking:
    // - If sneak is held, then change pose to sneaking.
    // - If sneak isn't held, then we need to check if sneaking is forced:
    // - Test the player's collider against the world:
    // - If it's colliding, then test a sneaking collider:
    // - If that doesn't collide, then force sneaking.

    // Main movement
    let mut forwards_target = (input.forward as u8 as f32 - input.backward as u8 as f32) * 0.98;
    let mut sideways_target = (input.right as u8 as f32 - input.left as u8 as f32) * 0.98;
    if input.sneak {
        forwards_target *= SNEAK_SPEED;
        sideways_target *= SNEAK_SPEED;
    }
    if !input.sprint {
        physics.sprinting = false;
    }
    move_player_with_heading(
        global_palette,
        raw_chunks,
        pos,
        player.yaw,
        physics,
        forwards_target,
        sideways_target,
        &mut input.sprint,
    );
}

#[allow(clippy::too_many_arguments)]
pub fn move_player_with_heading(
    global_palette: &BlockRegistry,
    raw_chunks: &AHashMap<[i32; 2], Arc<RawChunk>>,
    pos: &mut Point3<f32>,
    yaw: f32,
    physics: &mut PlayerPhysicsState,
    forwards_target: f32,
    sideways_target: f32,
    input_sprinting: &mut bool,
) {
    let gravity_multiplier = 1.0;
    let (acceleration, inertia) = if physics.on_ground {
        let inertia: f32 = 0.6 * 0.91;
        let sprint_modifier = if physics.sprinting { 2.0 } else { 1.0 };
        let player_speed = BASE_PLAYER_SPEED * sprint_modifier;
        let acceleration = (player_speed * (0.1627714 / (inertia.powi(3)))).max(0.0);
        (acceleration, inertia)
    } else {
        let sprint_modifier = if physics.sprinting { 1.3 } else { 1.0 };
        (AIR_ACCELERATION * sprint_modifier, AIR_INERTIA)
    };
    apply_heading_to_player(yaw, physics, forwards_target, sideways_target, acceleration);
    move_player(global_palette, raw_chunks, pos, physics, input_sprinting);
    let velocity = &mut physics.velocity;
    velocity.y -= GRAVITY * gravity_multiplier;
    velocity.y *= AIR_DRAG_COEF;
    velocity.x *= inertia;
    velocity.z *= inertia;
}

pub fn apply_heading_to_player(
    yaw: f32,
    physics: &mut PlayerPhysicsState,
    forwards_target: f32,
    sideways_target: f32,
    acceleration: f32,
) {
    let speed = f32::sqrt(sideways_target.powi(2) + forwards_target.powi(2));
    if speed < 0.01 {
        return;
    }
    let accelerated_speed = acceleration / f32::max(speed, 1.0);
    let accelerated_forwards = forwards_target * accelerated_speed;
    let accelerated_sideways = sideways_target * accelerated_speed;
    let (yaw_sin, yaw_cos) = (yaw.to_radians().sin(), yaw.to_radians().cos());
    let velocity = &mut physics.velocity;
    velocity.x += accelerated_sideways * yaw_cos + accelerated_forwards * yaw_sin;
    velocity.z -= accelerated_forwards * yaw_cos - accelerated_sideways * yaw_sin;
}

pub fn move_player(
    global_palette: &BlockRegistry,
    raw_chunks: &AHashMap<[i32; 2], Arc<RawChunk>>,
    pos: &mut Point3<f32>,
    physics: &mut PlayerPhysicsState,
    input_sprinting: &mut bool,
) {
    let velocity = &mut physics.velocity;

    let old_velocity = *velocity;

    let mut player_global_aabb = physics.local_aabb + pos.coords;
    let mut query_aabb = player_global_aabb.extended_in_direction(*velocity);
    let mut surrounding_aabbs = get_world_aabbs_in_area(global_palette, raw_chunks, query_aabb);

    let mut new_on_ground = false;
    while *velocity != Vector3::zeros() {
        let Some((normal, entry_time, world_aabb)) = surrounding_aabbs
            .iter()
            .copied()
            .filter_map(|world_aabb| {
                player_global_aabb
                    .get_collision_info(&world_aabb, *velocity)
                    .map(|(normal, entry_time)| (normal, entry_time, world_aabb))
            })
            .min_by(|(_, entry_a, _), (_, entry_b, _)| entry_a.total_cmp(entry_b))
        else {
            break;
        };
        let safe_entry_time = entry_time - 0.001;
        fn height_difference(player_aabb: &AABB, world_aabb: &AABB) -> f32 {
            world_aabb.corner_2.y - player_aabb.corner_1.y
        }
        if normal.x != 0.0 {
            // See if we can step up the colliding block
            let height_diff = height_difference(&player_global_aabb, &world_aabb);
            if (physics.on_ground || new_on_ground) && height_diff < STEP_HEIGHT {
                let x_change = velocity.x * (entry_time + 0.001);
                let step_test_aabb = player_global_aabb + Vector3::new(x_change, height_diff, 0.0);
                if !aabb_collides_with_world(global_palette, raw_chunks, &step_test_aabb) {
                    velocity.x -= x_change;
                    player_global_aabb = step_test_aabb;
                    query_aabb = player_global_aabb.extended_in_direction(*velocity);
                    surrounding_aabbs =
                        get_world_aabbs_in_area(global_palette, raw_chunks, query_aabb);
                    continue;
                }
            }
            // If we can't step up, collide with block
            player_global_aabb += Vector3::new(velocity.x * safe_entry_time, 0.0, 0.0);
            velocity.x = 0.0;
            // Check if collision stops us sprinting
            let old_vel_xz = old_velocity.xz();
            let old_vel_yaw =
                std::f32::consts::PI - f32::atan2(old_vel_xz[0].abs(), -old_vel_xz[1].abs());
            let minor_collision_angle_threshold_radians =
                MINOR_COLLISION_ANGLE_THRESHOLD.to_radians();
            if old_vel_yaw >= minor_collision_angle_threshold_radians {
                *input_sprinting = false;
            }
        } else if normal.y != 0.0 {
            player_global_aabb += Vector3::new(0.0, velocity.y * safe_entry_time, 0.0);
            velocity.y = 0.0;
            if old_velocity.y < 0.0 {
                new_on_ground = true;
            }
        } else {
            // See if we can step up the colliding block
            let height_diff = height_difference(&player_global_aabb, &world_aabb);
            if (physics.on_ground || new_on_ground) && height_diff < STEP_HEIGHT {
                let z_change = velocity.z * (entry_time + 0.001);
                let step_test_aabb = player_global_aabb + Vector3::new(0.0, height_diff, z_change);
                if !aabb_collides_with_world(global_palette, raw_chunks, &step_test_aabb) {
                    velocity.z -= z_change;
                    player_global_aabb = step_test_aabb;
                    query_aabb = player_global_aabb.extended_in_direction(*velocity);
                    surrounding_aabbs =
                        get_world_aabbs_in_area(global_palette, raw_chunks, query_aabb);
                    continue;
                }
            }
            // If we can't step up, collide with block
            player_global_aabb += Vector3::new(0.0, 0.0, velocity.z * safe_entry_time);
            velocity.z = 0.0;
            // Check if collision stops us sprinting
            let old_vel_xz = old_velocity.xz();
            let old_vel_yaw = f32::atan2(old_vel_xz[1].abs(), old_vel_xz[0].abs());
            let minor_collision_angle_threshold_radians =
                MINOR_COLLISION_ANGLE_THRESHOLD.to_radians();
            if old_vel_yaw >= minor_collision_angle_threshold_radians {
                *input_sprinting = false;
            }
        }
    }
    physics.on_ground = new_on_ground;
    physics.sprinting = *input_sprinting;
    // if *input_sprinting {
    //     physics.sprinting = true;
    // }
    // TODO: Shift safety:
    // - If we're on ground and shift is held:
    // - Create a test collider that's offsetted by leftover velocity and -STEP_HEIGHT.
    // - If the test collider doesn't collide with the world:
    // - Create new collider at original position offsetted by -STEP_HEIGHT.
    // - Go through colliding world aabbs:
    // - Clamp velocity by max allowed movement in X and Z axes.
    player_global_aabb += *velocity;
    // Update player info
    set_player_pos_to_aabb(pos, player_global_aabb, physics.local_aabb);

    // // Clamp velocity components using world collisions
    // let mut new_entity_aabb = entity_world_aabb;
    // {
    //     delta.y = surrounding_aabbs.iter().fold(delta.y, |acc, block_aabb| {
    //         block_aabb.compute_y_offset(&new_entity_aabb, acc)
    //     });
    //     new_entity_aabb += Vector3::new(0.0, delta.y, 0.0);
    //
    //     delta.x = surrounding_aabbs.iter().fold(delta.x, |acc, block_aabb| {
    //         block_aabb.compute_x_offset(&new_entity_aabb, acc)
    //     });
    //     new_entity_aabb += Vector3::new(delta.x, 0.0, 0.0);
    //
    //     delta.z = surrounding_aabbs.iter().fold(delta.z, |acc, block_aabb| {
    //         block_aabb.compute_z_offset(&new_entity_aabb, acc)
    //     });
    //     new_entity_aabb += Vector3::new(0.0, 0.0, delta.z);
    // }
    //
    // // Step on block if height < STEP_HEIGHT
    // if entity_physics.on_ground || (delta.y != old_velocity.y && old_velocity.y < 0.0) {
    //     let old_vel_col = delta;
    //     let old_bb_col = new_entity_aabb;
    //
    //     delta.y = STEP_HEIGHT;
    //     let query_aabb = entity_world_aabb.extended_in_direction(Vector3::new(
    //         old_velocity.x,
    //         delta.y,
    //         old_velocity.z,
    //     ));
    //     let surrounding_aabbs = get_world_aabbs_in_area(global_palette, raw_chunks, query_aabb);
    //
    //     let [mut bb1, mut bb2] = [entity_world_aabb; 2];
    //     let bb_xz = bb1.extended_in_direction(Vector3::new(delta.x, 0.0, delta.z));
    //
    //     // Adjust Y
    //     let [mut dy1, mut dy2] = [delta.y; 2];
    //     for block_aabb in &surrounding_aabbs {
    //         dy1 = block_aabb.compute_y_offset(&bb_xz, dy1);
    //         dy2 = block_aabb.compute_y_offset(&bb2, dy2);
    //     }
    //     bb1 += Vector3::new(0.0, dy1, 0.0);
    //     bb2 += Vector3::new(0.0, dy2, 0.0);
    //
    //     // Adjust X
    //     let [mut dx1, mut dx2] = [old_velocity.x; 2];
    //     for block_aabb in &surrounding_aabbs {
    //         dx1 = block_aabb.compute_x_offset(&bb1, dx1);
    //         dx2 = block_aabb.compute_x_offset(&bb2, dx2);
    //     }
    //     bb1 += Vector3::new(dx1, 0.0, 0.0);
    //     bb2 += Vector3::new(dx2, 0.0, 0.0);
    //
    //     // Adjust Z
    //     let [mut dz1, mut dz2] = [old_velocity.z; 2];
    //     for block_aabb in &surrounding_aabbs {
    //         dz1 = block_aabb.compute_z_offset(&bb1, dz1);
    //         dz2 = block_aabb.compute_z_offset(&bb2, dz2);
    //     }
    //     bb1 += Vector3::new(0.0, 0.0, dz1);
    //     bb2 += Vector3::new(0.0, 0.0, dz2);
    //
    //     // Move entity
    //     let norm_1 = dx1.powi(2) + dz1.powi(2);
    //     let norm_2 = dx2.powi(2) + dz2.powi(2);
    //
    //     if norm_1 > norm_2 {
    //         delta = Vector3::new(dx1, -dy1, dz1);
    //         new_entity_aabb = bb1;
    //     } else {
    //         delta = Vector3::new(dx2, -dy2, dz2);
    //         new_entity_aabb = bb2;
    //     }
    //
    //     for block_aabb in &surrounding_aabbs {
    //         delta.y = block_aabb.compute_y_offset(&new_entity_aabb, delta.y);
    //     }
    //     new_entity_aabb += Vector3::new(0.0, delta.y, 0.0);
    //
    //     // Move entity back if failed
    //     if old_vel_col.x.powi(2) + old_vel_col.z.powi(2) >= delta.x.powi(2) + delta.z.powi(2) {
    //         delta = old_vel_col;
    //         new_entity_aabb = old_bb_col;
    //     }
    // }
    // // Update entity info
    // set_entity_pos_to_aabb(pos, new_entity_aabb, entity_physics.local_aabb);
    // entity_physics.on_ground = delta.y != old_velocity.y && old_velocity.y < 0.0;
    // if delta.x != old_velocity.x {
    //     entity_physics.velocity.x = 0.0;
    // }
    // if delta.y != old_velocity.y {
    //     entity_physics.velocity.y = 0.0;
    // }
    // if delta.z != old_velocity.z {
    //     entity_physics.velocity.z = 0.0;
    // }
}

fn set_player_pos_to_aabb(pos: &mut Point3<f32>, aabb: AABB, local_aabb: AABB) {
    pos.x = aabb.corner_1.x + ((local_aabb.corner_2.x - local_aabb.corner_1.x) / 2.0);
    pos.y = aabb.corner_1.y;
    pos.z = aabb.corner_1.z + ((local_aabb.corner_2.z - local_aabb.corner_1.z) / 2.0);
}

fn aabb_collides_with_world(
    global_palette: &BlockRegistry,
    raw_chunks: &AHashMap<[i32; 2], Arc<RawChunk>>,
    aabb: &AABB,
) -> bool {
    for global_y in (aabb.corner_1.y as i32 - 1)..=(aabb.corner_2.y as i32) {
        for global_z in (aabb.corner_1.z as i32 - 1)..=(aabb.corner_2.z as i32) {
            for global_x in (aabb.corner_1.x as i32 - 1)..=(aabb.corner_2.x as i32) {
                let global_pos = [global_x, global_y, global_z];
                let maybe_info = get_section_info_and_inner_pos(raw_chunks, global_pos);
                let Some((chunk_section, [x, y, z])) = maybe_info else {
                    continue;
                };
                let global_palette_index = chunk_section.block_states.get(x, y, z);
                let blockstate_info = &global_palette[global_palette_index];
                let global_pos_vec_f32 = Vector3::from(global_pos.map(|n| n as f32));
                match &blockstate_info.extra_info.collision_info {
                    CollisionInfo::Empty => {}
                    CollisionInfo::FullBlock => {
                        let corner_1 = Point3::new(0.0, 0.0, 0.0);
                        let corner_2 = Point3::new(1.0, 1.0, 1.0);
                        let block_aabb = AABB { corner_1, corner_2 } + global_pos_vec_f32;
                        if aabb.intersects(&block_aabb) {
                            return true;
                        }
                    }
                    CollisionInfo::Complex(aabbs) => {
                        for base_aabb in aabbs {
                            let global_aabb = *base_aabb + global_pos_vec_f32;
                            if aabb.intersects(&global_aabb) {
                                return true;
                            }
                        }
                    }
                }
            }
        }
    }
    false
}

fn get_world_aabbs_in_area(
    global_palette: &BlockRegistry,
    raw_chunks: &AHashMap<[i32; 2], Arc<RawChunk>>,
    area: AABB,
) -> Vec<AABB> {
    let mut aabbs = Vec::new();
    for y in (area.corner_1.y as i32 - 1)..=(area.corner_2.y as i32) {
        for z in (area.corner_1.z as i32 - 1)..=(area.corner_2.z as i32) {
            for x in (area.corner_1.x as i32 - 1)..=(area.corner_2.x as i32) {
                get_block_aabbs(global_palette, raw_chunks, [x, y, z], &mut aabbs);
            }
        }
    }
    aabbs
}

#[inline]
fn get_block_aabbs(
    global_palette: &BlockRegistry,
    raw_chunks: &AHashMap<[i32; 2], Arc<RawChunk>>,
    global_pos: [i32; 3],
    output: &mut Vec<AABB>,
) {
    let maybe_info = get_section_info_and_inner_pos(raw_chunks, global_pos);
    let Some((chunk_section, [x, y, z])) = maybe_info else {
        return;
    };
    let global_palette_index = chunk_section.block_states.get(x, y, z);
    let blockstate_info = &global_palette[global_palette_index];
    let global_pos_vec_f32 = Vector3::from(global_pos.map(|n| n as f32));
    match &blockstate_info.extra_info.collision_info {
        CollisionInfo::Empty => {}
        CollisionInfo::FullBlock => {
            let corner_1 = Point3::new(0.0, 0.0, 0.0);
            let corner_2 = Point3::new(1.0, 1.0, 1.0);
            let base_aabb = AABB { corner_1, corner_2 };
            output.push(base_aabb + global_pos_vec_f32)
        }
        CollisionInfo::Complex(aabbs) => output.extend(
            aabbs
                .iter()
                .copied()
                .map(|base_aabb| base_aabb + global_pos_vec_f32),
        ),
    }
}

#[inline]
fn get_section_info_and_inner_pos(
    raw_chunks: &AHashMap<[i32; 2], Arc<RawChunk>>,
    global_pos: [i32; 3],
) -> Option<(&ChunkSection, [usize; 3])> {
    let chunk_x = global_pos[0].div_euclid(SUBCHUNK_AXIS_LEN_I32);
    let chunk_z = global_pos[2].div_euclid(SUBCHUNK_AXIS_LEN_I32);
    let section_i: usize = (global_pos[1] - MIN_HEIGHT_I32)
        .div_euclid(SUBCHUNK_AXIS_LEN_I32)
        .try_into()
        .ok()?;
    let chunk = raw_chunks.get(&[chunk_x, chunk_z])?;
    let chunk_section = &chunk.sections[section_i];
    let x = global_pos[0].rem_euclid(SUBCHUNK_AXIS_LEN_I32);
    let x_usize: usize = x.try_into().unwrap();
    let y = global_pos[1].rem_euclid(SUBCHUNK_AXIS_LEN_I32);
    let y_usize: usize = y.try_into().unwrap();
    let z = global_pos[2].rem_euclid(SUBCHUNK_AXIS_LEN_I32);
    let z_usize: usize = z.try_into().unwrap();
    Some((chunk_section, [x_usize, y_usize, z_usize]))
}
