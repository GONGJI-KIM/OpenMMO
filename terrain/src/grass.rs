use std::io;

use crate::{coords, defaults::TILE_DIM, io::TerrainIO};
use onlinerpg_shared::worldgen::vegetation::{
    GRASS_V3_BYTES_PER_INSTANCE, GRASS_V3_HEADER_BYTES, GRASS_V3_MAGIC,
};

pub type GrassExclusionRect = [f32; 4];

#[derive(Debug)]
pub struct GrassRemovalStats {
    pub tiles_changed: usize,
    pub grass_removed: usize,
    pub changed_tiles: Vec<(i32, i32)>,
}

fn invalid_grass_data(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn read_u32_le(data: &[u8], offset: usize) -> io::Result<u32> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| invalid_grass_data("grass data header is truncated"))?;
    Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
}

fn should_remove_grass(
    tile_x: i32,
    tile_z: i32,
    instance: &[u8],
    exclusion_rects: &[GrassExclusionRect],
) -> io::Result<bool> {
    let local_x = u16::from_le_bytes(
        instance[0..2]
            .try_into()
            .map_err(|_| invalid_grass_data("grass instance is truncated"))?,
    ) as f32
        * TILE_DIM as f32
        / 65535.0;
    let local_z = u16::from_le_bytes(
        instance[2..4]
            .try_into()
            .map_err(|_| invalid_grass_data("grass instance is truncated"))?,
    ) as f32
        * TILE_DIM as f32
        / 65535.0;
    let tile_offset = TILE_DIM as f32 * 0.5;
    let world_x = tile_x as f32 * TILE_DIM as f32 - tile_offset + local_x;
    let world_z = tile_z as f32 * TILE_DIM as f32 - tile_offset + local_z;

    Ok(exclusion_rects.iter().any(|[min_x, min_z, max_x, max_z]| {
        world_x >= *min_x && world_x <= *max_x && world_z >= *min_z && world_z <= *max_z
    }))
}

pub fn filter_grass_v3_bytes_in_rects(
    tile_x: i32,
    tile_z: i32,
    data: &[u8],
    exclusion_rects: &[GrassExclusionRect],
) -> io::Result<Option<(Vec<u8>, usize)>> {
    if exclusion_rects.is_empty() {
        return Ok(None);
    }
    if data.len() < GRASS_V3_HEADER_BYTES {
        return Err(invalid_grass_data("grass data header is truncated"));
    }
    let magic = read_u32_le(data, 0)?;
    if magic != GRASS_V3_MAGIC {
        return Err(invalid_grass_data(format!(
            "unsupported grass data magic 0x{magic:08x}"
        )));
    }

    let counts = [
        read_u32_le(data, 4)? as usize,
        read_u32_le(data, 8)? as usize,
        read_u32_le(data, 12)? as usize,
    ];
    let total: usize = counts.iter().sum();
    let expected_len = GRASS_V3_HEADER_BYTES + total * GRASS_V3_BYTES_PER_INSTANCE;
    if data.len() != expected_len {
        return Err(invalid_grass_data(format!(
            "grass data length mismatch: expected {expected_len}, got {}",
            data.len()
        )));
    }

    let mut out = Vec::with_capacity(data.len());
    out.extend_from_slice(&GRASS_V3_MAGIC.to_le_bytes());
    out.extend_from_slice(&[0; 12]);
    let mut kept_counts = [0usize; 3];
    let mut removed = 0usize;
    let mut offset = GRASS_V3_HEADER_BYTES;
    for grass_type in 0..3 {
        for _ in 0..counts[grass_type] {
            let instance = &data[offset..offset + GRASS_V3_BYTES_PER_INSTANCE];
            offset += GRASS_V3_BYTES_PER_INSTANCE;
            if should_remove_grass(tile_x, tile_z, instance, exclusion_rects)? {
                removed += 1;
                continue;
            }
            kept_counts[grass_type] += 1;
            out.extend_from_slice(instance);
        }
    }

    if removed == 0 {
        return Ok(None);
    }
    for (grass_type, kept) in kept_counts.into_iter().enumerate() {
        let offset = 4 + grass_type * 4;
        out[offset..offset + 4].copy_from_slice(&(kept as u32).to_le_bytes());
    }
    Ok(Some((out, removed)))
}

pub async fn remove_grass_in_rects(
    terrain: &TerrainIO,
    exclusion_rects: &[GrassExclusionRect],
) -> io::Result<GrassRemovalStats> {
    let mut stats = GrassRemovalStats {
        tiles_changed: 0,
        grass_removed: 0,
        changed_tiles: Vec::new(),
    };
    let mut tiles = Vec::new();
    for &[min_x, min_z, max_x, max_z] in exclusion_rects {
        for tile_z in coords::world_to_tile(min_z)..=coords::world_to_tile(max_z) {
            for tile_x in coords::world_to_tile(min_x)..=coords::world_to_tile(max_x) {
                if !tiles.contains(&(tile_x, tile_z)) {
                    tiles.push((tile_x, tile_z));
                }
            }
        }
    }

    for (tile_x, tile_z) in tiles {
        let Some(data) = terrain.read_grass(tile_x, tile_z).await? else {
            continue;
        };
        let Some((filtered, removed)) =
            filter_grass_v3_bytes_in_rects(tile_x, tile_z, &data, exclusion_rects)?
        else {
            continue;
        };
        terrain.ensure_original_grass(tile_x, tile_z).await?;
        terrain.write_grass(tile_x, tile_z, &filtered).await?;
        stats.tiles_changed += 1;
        stats.grass_removed += removed;
        stats.changed_tiles.push((tile_x, tile_z));
    }

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grass_data(types: &[&[(u16, u16)]]) -> Vec<u8> {
        let mut out = GRASS_V3_MAGIC.to_le_bytes().to_vec();
        for instances in types {
            out.extend_from_slice(&(instances.len() as u32).to_le_bytes());
        }
        for instances in types {
            for &(x, z) in *instances {
                out.extend_from_slice(&x.to_le_bytes());
                out.extend_from_slice(&z.to_le_bytes());
                out.extend_from_slice(&[0, 0]);
            }
        }
        out
    }

    #[test]
    fn filters_all_grass_types_inside_world_rect() {
        let data = grass_data(&[
            &[(32768, 32768), (65535, 65535)],
            &[(32768, 32768)],
            &[(65535, 65535)],
        ]);
        let (filtered, removed) =
            filter_grass_v3_bytes_in_rects(0, 0, &data, &[[-1.0, -1.0, 1.0, 1.0]])
                .unwrap()
                .unwrap();

        assert_eq!(removed, 2);
        assert_eq!(read_u32_le(&filtered, 4).unwrap(), 1);
        assert_eq!(read_u32_le(&filtered, 8).unwrap(), 0);
        assert_eq!(read_u32_le(&filtered, 12).unwrap(), 1);
    }

    #[test]
    fn returns_none_when_no_instances_match() {
        let data = grass_data(&[&[(65535, 65535)], &[], &[]]);
        assert!(
            filter_grass_v3_bytes_in_rects(0, 0, &data, &[[-1.0, -1.0, 1.0, 1.0]])
                .unwrap()
                .is_none()
        );
    }
}
