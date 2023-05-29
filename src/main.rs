use gander::{inventory::load_inventory, playbook::load_playbook};

fn main() -> anyhow::Result<()> {
    let inventory = load_inventory("/home/goose/dev/flock/inventory")?;
    println!("{:#?}", inventory);

    let playbook = load_playbook("/home/goose/dev/flock/playbook.toml")?;
    println!("{:#?}", playbook);

    Ok(())
}
