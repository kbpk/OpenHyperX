fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&hyperx_gui::Session::demo().unwrap().snapshot()).unwrap()
    );
}
