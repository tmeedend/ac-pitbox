//! The parser proper — layout described in spec §3.

use crate::error::{Kn5Error, Result};
use crate::limits::Limits;
use crate::model::*;
use crate::reader::Reader;

/// Magic at offset 0 of every unprotected KN5 (§3.1).
const MAGIC: [u8; 6] = *b"sc6969";

/// Highest version this parser was written against. Newer files are still
/// attempted — the layout has been stable across 5 and 6 — but they log a
/// warning, so a future format change shows up in the user's log file instead
/// of as a silently mis-parsed model.
const MAX_KNOWN_VERSION: u32 = 6;

// Smallest on-disk size of one entry of each repeated section. Fed to
// `Reader::count` so that no count can ever reserve more than the remaining
// bytes could hold, whatever the configured caps are.
const MIN_TEXTURE_BYTES: usize = 4; // a type-0 entry is just the type word
const MIN_MATERIAL_BYTES: usize = 22; // 2 string lengths + blend + reserved + 2 counts
const MIN_NODE_BYTES: usize = 13; // type + name length + children count + active flag
const MIN_BONE_BYTES: usize = 68; // name length + 4x4 matrix

/// Parses a whole KN5 with the default [`Limits`].
pub fn parse(data: &[u8]) -> Result<Kn5Model> {
    parse_with_limits(data, &Limits::default())
}

/// Same, with caps of the caller's choosing (tests use tiny ones to prove the
/// guards fire).
pub fn parse_with_limits(data: &[u8], limits: &Limits) -> Result<Kn5Model> {
    let mut r = Reader::new(data);

    if r.array::<6>()? != MAGIC {
        return Err(Kn5Error::NotAKn5File);
    }
    let version = r.u32()?;
    if version > MAX_KNOWN_VERSION {
        log::warn!("kn5: unknown version {version}, parsing anyway (layout assumed unchanged)");
    }
    let extra = if version > 5 { Some(r.u32()?) } else { None };

    let textures = read_textures(&mut r, limits)?;
    let materials = read_materials(&mut r, limits, version)?;
    let root = read_node(&mut r, limits, materials.len(), 0)?;

    // Trailing bytes are not fatal (some files carry padding), but they are a
    // strong hint that a section was mis-read — worth a line in the log.
    if r.remaining() > 0 {
        log::warn!(
            "kn5: {} trailing bytes after the node tree (offset {})",
            r.remaining(),
            r.position()
        );
    }

    Ok(Kn5Model {
        version,
        extra,
        textures,
        materials,
        root,
    })
}

fn read_textures(r: &mut Reader, limits: &Limits) -> Result<Vec<Kn5Texture>> {
    let count = r.count("texture_count", limits.max_textures, MIN_TEXTURE_BYTES)?;
    let mut textures = Vec::with_capacity(count);
    for _ in 0..count {
        let kind = r.i32()?;
        // A type-0 entry is a four-byte marker and nothing else: no name, no
        // size, no blob. The spec (§3.2) only said "0 rencontré = pas de
        // données", which reads as "the blob is empty"; taken that way the
        // whole section desynchronises. Found on `rss_gtm_lanzo_v8`, whose
        // first entry is type 0 — reading a name there swallowed the next
        // entry's type field and turned its size into 0x43000000. Skipping
        // the entry outright realigns every following texture exactly on its
        // PNG/DDS magic. See docs/kn5-format.md.
        if kind == 0 {
            textures.push(Kn5Texture {
                kind,
                name: String::new(),
                data: Vec::new(),
            });
            continue;
        }
        let name = r.string("texture_name", limits.max_string_bytes)?;
        let size = r.count("texture_size", limits.max_texture_bytes, 1)?;
        let data = r.bytes(size)?;
        textures.push(Kn5Texture { kind, name, data });
    }
    Ok(textures)
}

fn read_materials(r: &mut Reader, limits: &Limits, version: u32) -> Result<Vec<Kn5Material>> {
    let count = r.count("material_count", limits.max_materials, MIN_MATERIAL_BYTES)?;
    let mut materials = Vec::with_capacity(count);
    for _ in 0..count {
        let name = r.string("material_name", limits.max_string_bytes)?;
        let shader = r.string("material_shader", limits.max_string_bytes)?;
        // Spec §3.3 calls this a single `i16` of unknown meaning. It is in
        // fact two independent bytes — see docs/kn5-format.md: across the
        // reference library only 0, 1 and 256 occur, and every material at
        // 256 uses an `*_AT` (alpha-tested) shader while every material at 1
        // is glass or a decal.
        let blend_mode = r.u8()?;
        let alpha_tested = r.bool()?;
        let reserved = if version > 4 { r.i32()? } else { 0 };

        let prop_count = r.count("material_property_count", limits.max_material_properties, 44)?;
        let mut properties = Vec::with_capacity(prop_count);
        for _ in 0..prop_count {
            let name = r.string("material_property_name", limits.max_string_bytes)?;
            let value = r.f32()?;
            // 36 bytes of what is most likely a vector value (§12, q5). Read
            // as floats rather than skipped: costs nothing and lets the tool
            // answer the question from real files.
            let extra = r.f32s::<9>()?;
            properties.push(Kn5MaterialProperty { name, value, extra });
        }

        let sampler_count = r.count("material_sampler_count", limits.max_material_samplers, 12)?;
        let mut samplers = Vec::with_capacity(sampler_count);
        for _ in 0..sampler_count {
            let name = r.string("sampler_name", limits.max_string_bytes)?;
            let slot = r.i32()?;
            let texture = r.string("sampler_texture", limits.max_string_bytes)?;
            samplers.push(Kn5Sampler { name, slot, texture });
        }

        materials.push(Kn5Material {
            name,
            shader,
            blend_mode,
            alpha_tested,
            reserved,
            properties,
            samplers,
        });
    }
    Ok(materials)
}

/// Reads one node and, recursively, its children.
///
/// `depth` is checked first: a crafted file with a million nested dummies
/// would otherwise blow the stack, and a stack overflow aborts the process —
/// no `Result` can save us there (§5.2).
fn read_node(r: &mut Reader, limits: &Limits, material_count: usize, depth: usize) -> Result<Kn5Node> {
    if depth > limits.max_depth {
        return Err(Kn5Error::DepthLimitExceeded {
            limit: limits.max_depth,
        });
    }

    let node_type = r.i32()?;
    let name = r.string("node_name", limits.max_string_bytes)?;
    let children_count = r.count("children_count", limits.max_children, MIN_NODE_BYTES)?;
    let active = r.bool()?;

    let kind = match node_type {
        1 => Kn5NodeKind::Dummy {
            transform: r.f32s::<16>()?,
        },
        2 => Kn5NodeKind::Mesh(read_mesh(r, limits, material_count, &name)?),
        3 => Kn5NodeKind::SkinnedMesh(read_skinned_mesh(r, limits, material_count, &name)?),
        other => return Err(Kn5Error::UnknownNodeType(other, name)),
    };

    // In practice only dummies have children, but the format does not say so
    // and the parser must not assume it (§3.4).
    let mut children = Vec::with_capacity(children_count);
    for _ in 0..children_count {
        children.push(read_node(r, limits, material_count, depth + 1)?);
    }

    Ok(Kn5Node {
        name,
        active,
        kind,
        children,
    })
}

fn read_mesh(r: &mut Reader, limits: &Limits, material_count: usize, node_name: &str) -> Result<Kn5Mesh> {
    let cast_shadows = r.bool()?;
    let is_visible = r.bool()?;
    let is_transparent = r.bool()?;

    let vertices = read_vertices(r, limits)?;
    let indices = read_indices(r, limits)?;
    let material_id = read_material_id(r, material_count, node_name)?;

    Ok(Kn5Mesh {
        cast_shadows,
        is_visible,
        is_transparent,
        vertices,
        indices,
        material_id,
        layer: r.u32()?,
        lod_in: r.f32()?,
        lod_out: r.f32()?,
        bounding_sphere_center: r.f32s::<3>()?,
        bounding_sphere_radius: r.f32()?,
        is_renderable: r.bool()?,
    })
}

fn read_skinned_mesh(
    r: &mut Reader,
    limits: &Limits,
    material_count: usize,
    node_name: &str,
) -> Result<Kn5SkinnedMesh> {
    let cast_shadows = r.bool()?;
    let is_visible = r.bool()?;
    let is_transparent = r.bool()?;

    let bone_count = r.count("bone_count", limits.max_bones, MIN_BONE_BYTES)?;
    let mut bones = Vec::with_capacity(bone_count);
    for _ in 0..bone_count {
        bones.push(Kn5Bone {
            name: r.string("bone_name", limits.max_string_bytes)?,
            inverse_bind_matrix: r.f32s::<16>()?,
        });
    }

    // Skinned vertices interleave the rigid attributes with weights and bone
    // indices in a single 76-byte stride, hence the split into two parallel
    // vectors rather than a second pass.
    let count = r.count("vertex_count", limits.max_vertices, 76)?;
    let mut vertices = Vec::with_capacity(count);
    let mut skin = Vec::with_capacity(count);
    for _ in 0..count {
        vertices.push(Kn5Vertex {
            position: r.f32s::<3>()?,
            normal: r.f32s::<3>()?,
            uv: r.f32s::<2>()?,
            tangent: r.f32s::<3>()?,
        });
        skin.push(Kn5SkinBinding {
            weights: r.f32s::<4>()?,
            bone_indices: r.f32s::<4>()?,
        });
    }

    let indices = read_indices(r, limits)?;
    let material_id = read_material_id(r, material_count, node_name)?;

    let mesh = Kn5Mesh {
        cast_shadows,
        is_visible,
        is_transparent,
        vertices,
        indices,
        material_id,
        layer: r.u32()?,
        lod_in: r.f32()?,
        lod_out: r.f32()?,
        // Skinned meshes carry no bounding sphere and no renderable flag: the
        // trailing block is 12 bytes here against 29 on a rigid mesh (§3.4).
        bounding_sphere_center: [0.0; 3],
        bounding_sphere_radius: 0.0,
        is_renderable: true,
    };

    Ok(Kn5SkinnedMesh { mesh, bones, skin })
}

fn read_vertices(r: &mut Reader, limits: &Limits) -> Result<Vec<Kn5Vertex>> {
    let count = r.count("vertex_count", limits.max_vertices, 44)?;
    let mut vertices = Vec::with_capacity(count);
    for _ in 0..count {
        vertices.push(Kn5Vertex {
            position: r.f32s::<3>()?,
            normal: r.f32s::<3>()?,
            uv: r.f32s::<2>()?,
            tangent: r.f32s::<3>()?,
        });
    }
    Ok(vertices)
}

fn read_indices(r: &mut Reader, limits: &Limits) -> Result<Vec<u16>> {
    let count = r.count("index_count", limits.max_indices, 2)?;
    let mut indices = Vec::with_capacity(count);
    for _ in 0..count {
        indices.push(u16::from_le_bytes(r.array::<2>()?));
    }
    Ok(indices)
}

/// Validates the material index before it is ever used to index a slice —
/// a mesh pointing outside the material table is corruption, not something to
/// paper over (§5.2).
fn read_material_id(r: &mut Reader, material_count: usize, node_name: &str) -> Result<u32> {
    let id = r.i32()?;
    if id < 0 || id as usize >= material_count {
        return Err(Kn5Error::MaterialIdOutOfRange {
            node: node_name.to_string(),
            id: id as i64,
            count: material_count,
        });
    }
    Ok(id as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal but structurally valid file: one texture, one material, one
    /// dummy root holding one mesh. Built by hand so the whole test suite can
    /// run in CI, where no Assetto Corsa install exists.
    struct Kn5Builder {
        bytes: Vec<u8>,
    }

    impl Kn5Builder {
        fn new(version: u32) -> Self {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&MAGIC);
            bytes.extend_from_slice(&version.to_le_bytes());
            if version > 5 {
                bytes.extend_from_slice(&0u32.to_le_bytes());
            }
            Self { bytes }
        }

        fn i32(&mut self, v: i32) -> &mut Self {
            self.bytes.extend_from_slice(&v.to_le_bytes());
            self
        }

        fn u8(&mut self, v: u8) -> &mut Self {
            self.bytes.push(v);
            self
        }

        fn f32(&mut self, v: f32) -> &mut Self {
            self.bytes.extend_from_slice(&v.to_le_bytes());
            self
        }

        fn str(&mut self, s: &str) -> &mut Self {
            self.i32(s.len() as i32);
            self.bytes.extend_from_slice(s.as_bytes());
            self
        }

        fn raw(&mut self, data: &[u8]) -> &mut Self {
            self.bytes.extend_from_slice(data);
            self
        }
    }

    fn sample_file() -> Vec<u8> {
        let mut b = Kn5Builder::new(6);
        // One embedded texture.
        b.i32(1).i32(1).str("body.dds").i32(4).raw(&[0xDE, 0xAD, 0xBE, 0xEF]);
        // One material with one property and one sampler.
        b.i32(1).str("carpaint").str("ksPerPixelMultiMap");
        b.raw(&1i16.to_le_bytes()).i32(0);
        b.i32(1).str("ksDiffuse").f32(0.5);
        for _ in 0..9 {
            b.f32(0.0);
        }
        b.i32(1).str("txDiffuse").i32(0).str("body.dds");
        // Root dummy with one child mesh.
        b.i32(1).str("root").i32(1).u8(1);
        for i in 0..16 {
            b.f32(if i % 5 == 0 { 1.0 } else { 0.0 });
        }
        b.i32(2).str("BODY").i32(0).u8(1);
        b.u8(1).u8(1).u8(0);
        b.i32(3); // three vertices
        for i in 0..3 {
            b.f32(i as f32).f32(0.0).f32(0.0); // position
            b.f32(0.0).f32(1.0).f32(0.0); // normal
            b.f32(0.0).f32(0.0); // uv
            b.f32(1.0).f32(0.0).f32(0.0); // tangent
        }
        b.i32(3).raw(&[0, 0, 1, 0, 2, 0]); // three indices
        b.i32(0); // material id
        b.i32(0).f32(0.0).f32(0.0); // layer, lod in/out
        b.f32(0.0).f32(0.0).f32(0.0).f32(1.0); // bounding sphere
        b.u8(1); // renderable
        b.bytes
    }

    // The happy path: every section lands where the layout says it does.
    #[test]
    fn parses_a_minimal_model() {
        let model = parse(&sample_file()).expect("synthetic file parses");
        assert_eq!(model.version, 6, "version read from header");
        assert_eq!(model.textures.len(), 1, "one embedded texture");
        assert_eq!(model.textures[0].name, "body.dds", "texture name");
        assert_eq!(model.materials.len(), 1, "one material");
        assert_eq!(
            model.materials[0].texture_for("txDiffuse"),
            Some("body.dds"),
            "sampler resolves to the embedded texture"
        );
        assert_eq!(model.materials[0].property("ksDiffuse"), Some(0.5), "scalar property");
        assert_eq!(model.node_count(), 2, "root plus one mesh");
        assert_eq!(model.triangle_count(), 1, "one triangle");
    }

    // Rule: a type-0 texture entry occupies four bytes and nothing more, so
    // the entries that follow it stay aligned. Regression test for the only
    // car of the reference library that failed to parse (`rss_gtm_lanzo_v8`),
    // whose very first texture is one of those markers.
    #[test]
    fn type_zero_texture_entry_holds_no_name_and_no_blob() {
        let mut b = Kn5Builder::new(6);
        b.i32(2); // two entries…
        b.i32(0); // …the first one being an empty marker
        b.i32(1).str("body.dds").i32(4).raw(&[0xDE, 0xAD, 0xBE, 0xEF]);
        b.i32(0); // no materials
        b.i32(1).str("root").i32(0).u8(1);
        for i in 0..16 {
            b.f32(if i % 5 == 0 { 1.0 } else { 0.0 });
        }
        let model = parse(&b.bytes).expect("marker entry does not desynchronise the section");
        assert_eq!(model.textures.len(), 2, "both entries kept, faithful to the file");
        assert!(!model.textures[0].has_data(), "marker carries no blob");
        assert_eq!(
            model.texture("body.dds").map(|t| t.data.len()),
            Some(4),
            "the entry after the marker is read at the right offset"
        );
    }

    // Rule: a file that is not a KN5 is refused on its magic, not parsed as
    // garbage. Encrypted CSP models land here too (§4.5).
    #[test]
    fn wrong_magic_is_rejected() {
        let mut data = sample_file();
        data[0] = b'X';
        assert!(
            matches!(parse(&data), Err(Kn5Error::NotAKn5File)),
            "bad magic rejected before anything is read"
        );
    }

    // Rule: truncation at *any* offset yields an error, never a panic. This is
    // the single most important property of the parser — mods ship broken
    // files and one of them must not take the app down (§5.2).
    #[test]
    fn truncation_at_any_offset_errors_without_panic() {
        let full = sample_file();
        for cut in 0..full.len() {
            let result = parse(&full[..cut]);
            assert!(result.is_err(), "truncated at {cut} bytes must fail, not succeed");
        }
    }

    // Rule: random bytes are refused. Cheap stand-in for fuzzing until
    // `cargo-fuzz` is wired up.
    #[test]
    fn random_noise_is_rejected_without_panic() {
        // Deterministic pseudo-random: no dependency, reproducible failures.
        let mut state = 0x2545_F491_4F6C_DD1Du64;
        for _ in 0..256 {
            let noise: Vec<u8> = (0..100)
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    (state >> 24) as u8
                })
                .collect();
            assert!(parse(&noise).is_err(), "random noise must not parse as a model");
        }
    }

    // Rule: a mesh pointing outside the material table is corruption, and must
    // not reach the converter where it would index a slice.
    #[test]
    fn out_of_range_material_id_is_rejected() {
        let mut b = Kn5Builder::new(5);
        b.i32(0); // no textures
        b.i32(0); // no materials
        b.i32(2).str("BODY").i32(0).u8(1);
        b.u8(1).u8(1).u8(0);
        b.i32(0); // no vertices
        b.i32(0); // no indices
        b.i32(7); // material id, with zero materials declared
        assert!(
            matches!(parse(&b.bytes), Err(Kn5Error::MaterialIdOutOfRange { .. })),
            "material id validated against the table size"
        );
    }

    // Rule: the recursion is bounded. A deep tree must come back as an error,
    // because a stack overflow would abort the whole process.
    #[test]
    fn deep_node_tree_hits_the_depth_limit() {
        let mut b = Kn5Builder::new(5);
        b.i32(0).i32(0);
        // 40 nested dummies, each declaring one child.
        for i in 0..40 {
            b.i32(1).str("d").i32(1).u8(1);
            for _ in 0..16 {
                b.f32(if i == 0 { 1.0 } else { 0.0 });
            }
        }
        let limits = Limits {
            max_depth: 8,
            ..Limits::default()
        };
        assert!(
            matches!(
                parse_with_limits(&b.bytes, &limits),
                Err(Kn5Error::DepthLimitExceeded { .. })
            ),
            "recursion stops at the configured depth"
        );
    }

    // Rule: no count read from the file may drive an allocation larger than
    // the file could hold — this is what keeps a 100-byte file from reserving
    // gigabytes.
    #[test]
    fn absurd_vertex_count_is_rejected() {
        let mut b = Kn5Builder::new(5);
        b.i32(0).i32(0);
        b.i32(2).str("BODY").i32(0).u8(1);
        b.u8(1).u8(1).u8(0);
        b.i32(i32::MAX); // vertex count larger than any file
        assert!(
            matches!(parse(&b.bytes), Err(Kn5Error::LimitExceeded { .. })),
            "implausible vertex count rejected before allocating"
        );
    }
}
