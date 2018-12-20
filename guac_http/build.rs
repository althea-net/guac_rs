extern crate config_struct;

use config_struct::Options;

fn main() {
    config_struct::create_config("config.toml", "src/config.rs", &Options { derived_traits: vec!["Debug".to_string(), "Clone".to_string()] , generate_load_fns: false, ..Default::default() });
}