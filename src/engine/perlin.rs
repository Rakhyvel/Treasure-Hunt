static HASH: [i32; 256] = [
    208, 34, 231, 213, 32, 248, 233, 56, 161, 78, 24, 140, 71, 48, 140, 254, 245, 255, 247, 247,
    40, 185, 248, 251, 245, 28, 124, 204, 204, 76, 36, 1, 107, 28, 234, 163, 202, 224, 245, 128,
    167, 204, 9, 92, 217, 54, 239, 174, 173, 102, 193, 189, 190, 121, 100, 108, 167, 44, 43, 77,
    180, 204, 8, 81, 70, 223, 11, 38, 24, 254, 210, 210, 177, 32, 81, 195, 243, 125, 8, 169, 112,
    32, 97, 53, 195, 13, 203, 9, 47, 104, 125, 117, 114, 124, 165, 203, 181, 235, 193, 206, 70,
    180, 174, 0, 167, 181, 41, 164, 30, 116, 127, 198, 245, 146, 87, 224, 149, 206, 57, 4, 192,
    210, 65, 210, 129, 240, 178, 105, 228, 108, 245, 148, 140, 40, 35, 195, 38, 58, 65, 207, 215,
    253, 65, 85, 208, 76, 62, 3, 237, 55, 89, 232, 50, 217, 64, 244, 157, 199, 121, 252, 90, 17,
    212, 203, 149, 152, 140, 187, 234, 177, 73, 174, 193, 100, 192, 143, 97, 53, 145, 135, 19, 103,
    13, 90, 135, 151, 199, 91, 239, 247, 33, 39, 145, 101, 120, 99, 3, 186, 86, 99, 41, 237, 203,
    111, 79, 220, 135, 158, 42, 30, 154, 120, 67, 87, 167, 135, 176, 183, 191, 253, 115, 184, 21,
    233, 58, 129, 233, 142, 39, 128, 211, 118, 137, 139, 255, 114, 20, 218, 113, 154, 27, 127, 246,
    250, 1, 8, 198, 250, 209, 92, 222, 173, 21, 88, 102, 219,
];

pub fn generate(map_size: usize, amplitude: f32, seed: i32) -> Vec<f32> {
    let mut retval: Vec<f32> = Vec::new();
    retval.resize(map_size * map_size, 0.0);

    for y in 0..map_size {
        for x in 0..map_size {
            retval[x + y * map_size] = perlin2d(x as f32, y as f32, amplitude, 10, seed);
        }
    }

    retval
}

fn perlin2d(x: f32, y: f32, freq: f32, depth: i32, seed: i32) -> f32 {
    let mut xa = x * freq;
    let mut ya = y * freq;
    let mut amp: f32 = 1.0;
    let mut fin: f32 = 0.0;
    let mut div: f32 = 0.0;

    for _ in 0..depth {
        div += 256.0 * amp;
        fin += noise2d(xa, ya, seed) * amp;
        amp /= 2.0;
        xa *= 2.0;
        ya *= 2.0;
    }

    fin / div
}

fn noise2d(x: f32, y: f32, seed: i32) -> f32 {
    let x_int = x as i32;
    let y_int = y as i32;
    let x_frac: f32 = x - (x_int as f32);
    let y_frac: f32 = y - (y_int as f32);
    let s = noise2(x_int, y_int, seed);
    let t = noise2(x_int + 1, y_int, seed);
    let u = noise2(x_int, y_int + 1, seed);
    let v = noise2(x_int + 1, y_int + 1, seed);
    let low = smooth_inter(s as f32, t as f32, x_frac);
    let high = smooth_inter(u as f32, v as f32, x_frac);
    smooth_inter(low, high, y_frac)
}

fn noise2(x: i32, y: i32, seed: i32) -> i32 {
    let tmp = HASH[((y + seed) % 256).abs() as usize];
    HASH[((tmp + x) % 256).abs() as usize]
}

fn smooth_inter(x: f32, y: f32, s: f32) -> f32 {
    lin_inter(x, y, s * s * (3.0 - 2.0 * s))
}

fn lin_inter(x: f32, y: f32, s: f32) -> f32 {
    x + s * (y - x)
}

pub fn erosion(map: &mut Vec<f32>, map_size: usize, intensity: f32) {
    let sediment_capacity_factor: f32 = 1000.0;
    let max_sediment_capacity: f32 = 1.0; // small values = more deposit
    let deposit_speed = 0.01;
    let erode_speed: f32 = 0.01;
    let evaporate_speed = 0.0000000001;
    let gravity = 100.0;

    let mut total_eroded: f32 = 0.0;

    for i in 0..(map_size as f32 * intensity) as usize {
        let scale = 1.0 / (2.0 * intensity);
        let mut pos = nalgebra_glm::vec2(
            scale * i as f32 * (i as f32).cos() + map_size as f32 * 0.5,
            scale * i as f32 * (i as f32).sin() + map_size as f32 * 0.5,
        );
        let mut vel = nalgebra_glm::vec2(0.0, 0.0);
        let mut water = 1.0;
        let mut sediment = 0.0;
        for _ in 0..(map_size / 2) {
            let node = nalgebra_glm::vec2(pos.x.floor(), pos.y.floor());
            let droplet_index = (node.x as i32 + node.y as i32 * map_size as i32) as usize;
            let cell_offset = pos - node;
            if droplet_index >= map.len() - map_size - 1 {
                break;
            }

            let grad = get_normal(map, map_size, pos.x, pos.y);
            let erosion_ease = 1.0 / (nalgebra_glm::length(&grad.xy()) + 1.0).powf(2.0); // How easy is it to erode this substance, given it's angle
            vel *= erosion_ease; // Provide more friction for rock than for dirt and sand
            vel += gravity * grad.xy();
            let mut dir = vel;
            let len = vel.x.max(vel.y);
            if len != 0.0 {
                dir /= len; // This is (somehow) not the same as nalgebra_glm::normalize()
            }
            pos += dir;

            // Stop simulating droplet if it's not moving or has flowed over edge of map
            if (vel.x <= 0.1 && vel.y <= 0.1)
                || pos.x <= 0.0
                || pos.x >= map_size as f32 - 1.0
                || pos.y <= 0.0
                || pos.y >= map_size as f32 - 1.0
            {
                break;
            }

            // Find the droplet's new height and calculate the deltaHeight
            let new_height = get_z_scaled_interpolated(map, map_size, pos.x, pos.y);
            let delta_height = new_height - grad.z;

            // Calculate the droplet's sediment capacity (higher when moving fast down a slope and contains lots of water)
            let speed = nalgebra_glm::length(&vel);
            let sediment_capacity: f32 =
                (speed * water * sediment_capacity_factor).min(max_sediment_capacity);

            let delta_z: f32 =
            // If carrying more sediment than capacity, or if flowing uphill:
            if sediment > sediment_capacity || delta_height > 0.0 {
                let deposit_amount = (sediment_capacity - sediment) * deposit_speed;
                if delta_height > 0.0 {
                    delta_height.min(deposit_amount)
                } else {
                    deposit_amount
                }
            } else {
                -(erosion_ease * erode_speed).min(delta_height.abs() + 0.1)
            };
            sediment -= delta_z;
            total_eroded += if delta_z < 0.0 { delta_z } else { 0.0 };
            map[droplet_index] += delta_z * (1.0 - cell_offset.x) * (1.0 - cell_offset.y);
            map[droplet_index + 1] += delta_z * cell_offset.x * (1.0 - cell_offset.y);
            map[droplet_index + map_size as usize] +=
                delta_z * (1.0 - cell_offset.x) * cell_offset.y;
            map[droplet_index + 1 + map_size as usize] += delta_z * cell_offset.x * cell_offset.y;

            // Update droplets speed and water content
            water -= evaporate_speed;
            if water < 0.0 {
                break;
            }
        }
    }
    println!("Total eroded: {}", total_eroded);
}

fn get_z(tiles: &Vec<f32>, map_size: usize, x: usize, y: usize) -> f32 {
    tiles[x + y * map_size]
}

// TODO: These NEED to be all put into a single file
fn get_z_scaled_interpolated(tiles: &Vec<f32>, map_size: usize, x: f32, y: f32) -> f32 {
    assert!(!x.is_nan());
    // The coordinates of the tile's origin (bottom left corner)
    let x_origin = x.floor();
    let y_origin = y.floor();

    // Coordinates inside the tile. [0,1]
    let x_offset = x - x_origin;
    let y_offset = y - y_origin;

    let ray_origin = nalgebra_glm::vec3(x, y, 10000.0);
    let ray_direction = nalgebra_glm::vec3(0.0, 0.0, -1.0);

    if y_offset <= 1.0 - x_offset {
        // In bottom triangle
        let (retval, _t) = intersect(
            nalgebra_glm::vec3(
                x_origin,
                y_origin,
                get_z(tiles, map_size, x_origin as usize, y_origin as usize),
            ),
            nalgebra_glm::vec3(
                x_origin + 1.0,
                y_origin,
                get_z(tiles, map_size, x_origin as usize + 1, y_origin as usize),
            ),
            nalgebra_glm::vec3(
                x_origin,
                y_origin + 1.0,
                get_z(tiles, map_size, x_origin as usize, y_origin as usize + 1),
            ),
            ray_origin,
            ray_direction,
        )
        .unwrap();
        retval.z
    } else {
        // In top triangle
        let (retval, _t) = intersect(
            nalgebra_glm::vec3(
                x_origin + 1.0,
                y_origin,
                get_z(tiles, map_size, x_origin as usize + 1, y_origin as usize),
            ),
            nalgebra_glm::vec3(
                x_origin + 1.0,
                y_origin + 1.0,
                get_z(
                    tiles,
                    map_size,
                    x_origin as usize + 1,
                    y_origin as usize + 1,
                ),
            ),
            nalgebra_glm::vec3(
                x_origin,
                y_origin + 1.0,
                get_z(tiles, map_size, x_origin as usize, y_origin as usize + 1),
            ),
            ray_origin,
            ray_direction,
        )
        .unwrap();
        retval.z
    }
}

fn intersect(
    v0: nalgebra_glm::Vec3,
    v1: nalgebra_glm::Vec3,
    v2: nalgebra_glm::Vec3,
    ray_origin: nalgebra_glm::Vec3,
    ray_direction: nalgebra_glm::Vec3,
) -> Option<(nalgebra_glm::Vec3, f32)> {
    const EPSILON: f32 = 0.0000001;
    let edge1 = v1 - v0;
    let edge2 = v2 - v0;
    let h = nalgebra_glm::cross(&ray_direction, &edge2);
    let a = nalgebra_glm::dot(&edge1, &h);

    if a.abs() < EPSILON {
        return None; // Ray is parallel to the triangle
    }

    let f = 1.0 / a;
    let s = ray_origin - v0;
    let u = f * nalgebra_glm::dot(&s, &h);

    if u < 0.0 || u > 1.0 {
        return None;
    }

    let q = nalgebra_glm::cross(&s, &edge1);
    let v = f * nalgebra_glm::dot(&ray_direction, &q);

    if v < 0.0 || u + v > 1.0 {
        return None;
    }

    let t = f * nalgebra_glm::dot(&edge2, &q);

    let intersection_point = ray_origin + t * ray_direction;
    Some((intersection_point, t))
}

fn get_normal(tiles: &Vec<f32>, map_size: usize, x: f32, y: f32) -> nalgebra_glm::Vec3 {
    assert!(!x.is_nan());
    // The coordinates of the tile's origin (bottom left corner)
    let x_origin = x.floor();
    let y_origin = y.floor();

    // Coordinates inside the tile. [0,1]
    let x_offset = x - x_origin;
    let y_offset = y - y_origin;

    if y_offset <= 1.0 - x_offset {
        // In bottom triangle
        tri_normal(
            nalgebra_glm::vec3(
                x_origin,
                y_origin,
                get_z(tiles, map_size, x_origin as usize, y_origin as usize),
            ),
            nalgebra_glm::vec3(
                x_origin + 1.0,
                y_origin,
                get_z(tiles, map_size, x_origin as usize + 1, y_origin as usize),
            ),
            nalgebra_glm::vec3(
                x_origin,
                y_origin + 1.0,
                get_z(tiles, map_size, x_origin as usize, y_origin as usize + 1),
            ),
        )
    } else {
        // In top triangle
        tri_normal(
            nalgebra_glm::vec3(
                x_origin + 1.0,
                y_origin,
                get_z(tiles, map_size, x_origin as usize + 1, y_origin as usize),
            ),
            nalgebra_glm::vec3(
                x_origin + 1.0,
                y_origin + 1.0,
                get_z(
                    tiles,
                    map_size,
                    x_origin as usize + 1,
                    y_origin as usize + 1,
                ),
            ),
            nalgebra_glm::vec3(
                x_origin,
                y_origin + 1.0,
                get_z(tiles, map_size, x_origin as usize, y_origin as usize + 1),
            ),
        )
    }
}

fn tri_normal(
    v0: nalgebra_glm::Vec3,
    v1: nalgebra_glm::Vec3,
    v2: nalgebra_glm::Vec3,
) -> nalgebra_glm::Vec3 {
    let edge1 = v1 - v0;
    let edge2 = v2 - v0;
    let normal = nalgebra_glm::cross(&edge1, &edge2).normalize();
    normal
}
