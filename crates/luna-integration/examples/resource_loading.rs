// SPDX-License-Identifier: MPL-2.0

//! Minimal downstream resource-discovery example.

use luna_integration::ResourceLocator;
use std::error::Error;
use std::path::Path;

fn main() -> Result<(), Box<dyn Error>> {
    let locator = ResourceLocator::discover("org.lunaui.EditorDemo")?;
    match locator.resolve(Path::new("EDITOR_DEMO_COMMANDS.md")) {
        Ok(path) => println!("resolved editor command reference: {}", path.display()),
        Err(error) => println!("resource not installed in this development layout: {error}"),
    }
    Ok(())
}
