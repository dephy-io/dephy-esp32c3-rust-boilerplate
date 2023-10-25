use paste::paste;
use std::env;
use std::fs;
use std::path::Path;
use std::str::FromStr;

macro_rules! p {
    ($($tokens: tt)*) => {{
        if env::var("BUILD_PRINT_EXPANDED_ENV").unwrap_or_default() == "true" {
            println!("cargo:warning={}", format!($($tokens)*));
        }
    }}
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    prost_build::compile_protos(
        &["src/proto/message.proto", "src/proto/stpw.proto"],
        &["src/proto/"],
    )?;
    embuild::build::CfgArgs::output_propagated("ESP_IDF")?;
    embuild::build::LinkArgs::output_propagated("ESP_IDF")?;
    build_env()?;
    Ok(())
}

fn build_env() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("build_env.rs");
    dotenvy::from_filename("build.env")?;
    let mut lines = vec![];

    macro_rules! env_string {
        ($name: expr, $default: expr) => {
            paste! {
                lines.push(format!(
                    "pub static {}: &'static str = \"{}\";",
                    $name,
                    env::var($name).unwrap_or($default.into())
                ));
            }
        };
    }
    macro_rules! env_number {
        ($name: expr, $type: ty, $default: expr) => {
            paste! {
                lines.push(format!(
                    "pub static {}: {} = {};",
                    $name,
                    stringify!($type),
                    env::var($name).map(|s| $type::from_str(s.as_str()).unwrap()).unwrap_or($default)
                ));
            }
        };
    }

    env_string!(
        "DEPHY_ENDPOINT_HTTP",
        "https://send.testnet.dephy.io/dephy/signed_message"
    );
    env_number!("APP_SEND_LOOP_DURATION", u64, 10);

    for l in lines.iter() {
        p!("cargo:warning={}", l)
    }

    fs::write(&dest_path, lines.join("\n")).unwrap();
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build.env");

    Ok(())
}
