fn main() {
    #[cfg(feature = "slint-ui")]
    {
        slint_build::compile("ui/hud.slint").expect("slint compile hud.slint");
    }
}
