use std::fs;

use super::constants::data_dir;
use super::fetch_pdb;
use super::parse_pdb::parse_pdb;

pub fn get_rva(guid: String) -> u32 {
    let dir = data_dir();
    let pdbpath = dir.join(guid.clone() + ".rva");

    if pdbpath.exists() {
        let file = fs::read(pdbpath).unwrap();
        u32::from_be_bytes(file.try_into().unwrap())
    } else {
        let url = fetch_pdb::build_url(guid);
        let pdbfile = fetch_pdb::fetch(url);
        let rva = parse_pdb(pdbfile);

        if dir.exists() {
            fs::remove_dir_all(&dir).unwrap();
        }
        fs::create_dir_all(&dir).unwrap();
        fs::write(pdbpath, rva.to_be_bytes()).unwrap();
        rva
    }
}
