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

#[test]
fn shader_scene_ids_match_catalog_order() {
    let expected = "rain starfield fire pipes plasma aurora life boids lava tunnel dvd bump canopy finale ocean circuits clouds mandel meteors koi sand city abyss den traffic nexus ripple fireflies lanterns incense frost orbits ribbons sonar tide clockwork grid inkdrop mosaic harmonograph nebula pendulum reaction meadow airspace aquarium drive candy scroll";
    assert_eq!(
        termpaper::scene::names(),
        expected.split_whitespace().collect::<Vec<_>>()
    );
}
