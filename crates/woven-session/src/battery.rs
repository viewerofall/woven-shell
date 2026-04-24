use anyhow::Result;
use std::fs;
use std::path::Path;

pub struct BatteryReader;

impl BatteryReader {
    pub fn read() -> Result<(u8, bool)> {
        let power_supply = Path::new("/sys/class/power_supply");

        let mut total_capacity = 0;
        let mut total_energy = 0;
        let mut ac_online = false;

        // Read all power supplies
        if let Ok(entries) = fs::read_dir(power_supply) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Ok(typ) = fs::read_to_string(path.join("type")) {
                    let typ = typ.trim();

                    if typ == "Mains" {
                        if let Ok(online) = fs::read_to_string(path.join("online")) {
                            if online.trim() == "1" {
                                ac_online = true;
                            }
                        }
                    }

                    if typ == "Battery" {
                        // Try energy_full/energy_now first (Joules)
                        if let (Ok(full), Ok(now)) =
                            (
                                fs::read_to_string(path.join("energy_full")),
                                fs::read_to_string(path.join("energy_now")),
                            ) {
                            if let (Ok(f), Ok(n)) = (full.trim().parse::<u64>(), now.trim().parse::<u64>()) {
                                total_capacity += f;
                                total_energy += n;
                                continue;
                            }
                        }

                        // Fall back to charge_full/charge_now (Coulombs)
                        if let (Ok(full), Ok(now)) =
                            (
                                fs::read_to_string(path.join("charge_full")),
                                fs::read_to_string(path.join("charge_now")),
                            ) {
                            if let (Ok(f), Ok(n)) = (full.trim().parse::<u64>(), now.trim().parse::<u64>()) {
                                total_capacity += f;
                                total_energy += n;
                            }
                        }
                    }
                }
            }
        }

        let percent = if total_capacity > 0 {
            ((total_energy as f64 / total_capacity as f64) * 100.0).min(100.0) as u8
        } else {
            0
        };

        Ok((percent, ac_online))
    }
}
