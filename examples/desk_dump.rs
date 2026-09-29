//! Print the physical desk termpaper derives from Hyprland (and desk.toml).
fn main() {
    let Some(mons) = termpaper::hypr::monitors() else {
        eprintln!("not running under Hyprland");
        std::process::exit(1);
    };
    let cfg = termpaper::desk::load_desk();
    let desk = termpaper::desk::Desk::from_hypr(&mons, &cfg);
    print!("{}", termpaper::desk::describe(&desk));
}
