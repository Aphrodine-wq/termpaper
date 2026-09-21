#[test]
fn world_shader_is_valid_without_a_gpu() {
    let source = include_str!("../src/gpu/shaders/world.wgsl");
    let module = naga::front::wgsl::parse_str(source)
        .unwrap_or_else(|error| panic!("{}", error.emit_to_string(source)));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .unwrap_or_else(|error| panic!("{}", error.emit_to_string(source)));
}

// WGSL `case N:` arms are indexed by catalog position, so new scenes are
// appended at the end of `SCENES` (and of this list). A scene without an arm
// falls into the shader's `default:` and is CPU-rendered unless allowlisted.
#[test]
fn shader_scene_ids_match_catalog_order() {
    let expected = "rain starfield fire pipes plasma aurora life boids lava tunnel dvd bump canopy finale ocean circuits clouds mandel meteors koi sand city abyss den traffic nexus ripple fireflies lanterns incense frost orbits ribbons sonar tide clockwork grid inkdrop mosaic harmonograph nebula pendulum reaction meadow airspace aquarium drive candy scroll";
    assert_eq!(
        termpaper::scene::names(),
        expected.split_whitespace().collect::<Vec<_>>()
    );
}

#[test]
fn allowlisted_scenes_have_shader_arms() {
    // every GPU-world scene must sit inside the range the shader switches on
    let with_arms = "rain starfield fire pipes plasma aurora life boids lava tunnel dvd bump canopy finale ocean circuits clouds mandel meteors koi sand city abyss den traffic nexus ripple fireflies lanterns incense frost orbits ribbons sonar tide clockwork grid inkdrop mosaic harmonograph nebula pendulum reaction meadow airspace aquarium drive candy scroll";
    for name in termpaper::scene::GPU_WORLD_SCENES {
        assert!(
            with_arms.split_whitespace().any(|n| n == *name),
            "{name} is allowlisted for the GPU world but has no WGSL arm"
        );
    }
}
