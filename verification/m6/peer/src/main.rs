//! Independent Wasmtime/bindgen execution peer. Noble compiler crates are not dependencies.
mod adapter;
mod compatibility;
mod controls;
mod driver;
mod progress;
mod transmit;

use anyhow::{Result, bail};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    let result = match args.as_slice() {
        [_, mode, case] if mode == "--lifecycle" => controls::run(case)?,
        [_, mode, case] if mode == "--progress" => progress::run(case).await?,
        [_, mode, component, case] if mode == "--compatibility" => {
            compatibility::run(component, case).await?
        }
        [_, component, export, case] if !component.starts_with("--") => {
            driver::execute(component, export, case).await?
        }
        _ => bail!(
            "usage: noble-m6-peer COMPONENT EXPORT CASE | --lifecycle CASE | --progress CASE | --compatibility COMPONENT MODE"
        ),
    };
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}
