//! bmapctl — Minimal CLI for controlling BMAP devices.
//!
//! Single-binary alternative to bosectl, built on the bmap Rust library.

use std::env;
use std::process;

use bmap::{connect, BmapError, BmapResult, PairedDevice};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        usage();
        process::exit(1);
    }

    let cmd = args[1].to_lowercase();
    if cmd == "help" || cmd == "-h" || cmd == "--help" {
        usage();
        process::exit(0);
    }

    let mac = env::var("BMAP_MAC").ok();
    let device_type = env::var("BMAP_DEVICE").ok();

    let dev = match connect(mac.as_deref(), device_type.as_deref()) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Connection failed: {}", e);
            if let Some(hint) = connection_hint(&e) {
                eprintln!("{}", hint);
            }
            process::exit(1);
        }
    };

    let result = match cmd.as_str() {
        "status" if args[2..].iter().any(|a| a == "--json") => cmd_status_json(&dev),
        "status" => cmd_status(&dev),
        "battery" => dev.battery().map(|b| println!("{}", b)),
        "current" => dev.mode().map(|m| println!("{}", m)),
        "quiet" | "aware" | "immersion" | "cinema" => {
            dev.set_mode(&cmd, false).map(|_| println!("OK: {}", cmd))
        }
        "cnc" => {
            if args.len() < 3 {
                dev.cnc().map(|(cur, max)| println!("{}/{}", cur, max))
            } else {
                let level: u8 = args[2].parse().unwrap_or_else(|_| {
                    eprintln!("CNC level must be 0-10");
                    process::exit(1);
                });
                dev.set_cnc(level).map(|_| println!("CNC: {}/10", level))
            }
        }
        "spatial" => {
            if args.len() < 3 {
                eprintln!("Usage: bmapctl spatial <off|room|head>");
                process::exit(1);
            }
            dev.set_spatial(&args[2]).map(|_| println!("Spatial: {}", args[2]))
        }
        "autoanswer" => {
            if args.len() > 2 {
                let on = matches!(args[2].as_str(), "on" | "1" | "true" | "yes");
                if let Err(e) = dev.set_auto_answer(on) {
                    return err_exit(&e);
                }
            }
            dev.auto_answer().map(|a| println!("{}", if a { "on" } else { "off" }))
        }
        "switch" => {
            if args.len() < 3 {
                eprintln!("Usage: bmapctl switch <name>");
                process::exit(1);
            }
            dev.set_mode(&args[2], false).map(|_| println!("OK: {}", args[2]))
        }
        "eq" => {
            if args.len() < 5 {
                dev.eq().map(|bands| {
                    for b in &bands {
                        println!("{:6}: {:+}", b.name, b.current);
                    }
                })
            } else {
                let bass: i8 = args[2].parse().unwrap_or(0);
                let mid: i8 = args[3].parse().unwrap_or(0);
                let treble: i8 = args[4].parse().unwrap_or(0);
                dev.set_eq(bass, mid, treble).map(|_| println!("EQ: {}/{}/{}", bass, mid, treble))
            }
        }
        "name" => {
            if args.len() > 2 {
                let new_name = args[2..].join(" ");
                if let Err(e) = dev.set_name(&new_name) {
                    return err_exit(&e);
                }
            }
            dev.name().map(|n| println!("{}", n))
        }
        "sidetone" => {
            if args.len() > 2 {
                if let Err(e) = dev.set_sidetone(&args[2]) {
                    return err_exit(&e);
                }
            }
            dev.sidetone().map(|s| println!("{}", s))
        }
        "multipoint" => {
            if args.len() > 2 {
                let on = matches!(args[2].as_str(), "on" | "1" | "true" | "yes");
                if let Err(e) = dev.set_multipoint(on) {
                    return err_exit(&e);
                }
            }
            dev.multipoint().map(|m| println!("{}", if m { "on" } else { "off" }))
        }
        "autopause" => {
            if args.len() > 2 {
                let on = matches!(args[2].as_str(), "on" | "1" | "true" | "yes");
                if let Err(e) = dev.set_auto_pause(on) {
                    return err_exit(&e);
                }
            }
            dev.auto_pause().map(|a| println!("{}", if a { "on" } else { "off" }))
        }
        "anr" => {
            if args.len() > 2 {
                if let Err(e) = dev.set_anr(&args[2]) {
                    return err_exit(&e);
                }
            }
            dev.anr().map(|a| println!("{}", a))
        }
        "anc" => {
            if args.len() > 2 {
                let on = matches!(args[2].as_str(), "on" | "1" | "true" | "yes");
                if let Err(e) = dev.set_anc(on) {
                    return err_exit(&e);
                }
            }
            if dev.config().audio_settings.is_none() && !dev.config().supports_anc_toggle {
                Ok(println!("unsupported"))
            } else {
                dev.audio_settings().map(|(_, _, _, _, anc)| {
                    println!("{}", if anc != 0 { "on" } else { "off" });
                })
            }
        }
        "wind" => {
            if args.len() > 2 {
                let on = matches!(args[2].as_str(), "on" | "1" | "true" | "yes");
                if let Err(e) = dev.set_wind(on) {
                    return err_exit(&e);
                }
            }
            dev.audio_settings().map(|(_, _, _, wind, _)| {
                println!("{}", if wind != 0 { "on" } else { "off" });
            })
        }
        "prompts" => {
            if args.len() > 2 {
                let on = matches!(args[2].as_str(), "on" | "1" | "true" | "yes");
                if let Err(e) = dev.set_prompts(on) {
                    return err_exit(&e);
                }
            }
            dev.prompts().map(|(on, lang)| {
                println!("{} ({})", if on { "on" } else { "off" }, lang);
            })
        }
        "buttons" => {
            if args.len() >= 4 && args[2] == "set" {
                let btn = match dev.buttons() {
                    Ok(b) => b,
                    Err(e) => return err_exit(&e),
                };
                let action = bmap::device::action_id_from_name(&args[3])
                    .unwrap_or_else(|| {
                        eprintln!("Unknown action: {}", args[3]);
                        process::exit(1);
                    });
                dev.set_buttons(btn.button_id, btn.event, action)
                    .map(|result| {
                        println!("Remapped: {} {} -> {}", result.button_name, result.event_name, result.action_name);
                    })
            } else {
                dev.buttons().map(|btn| {
                    println!("Button:  {} (0x{:02x})", btn.button_name, btn.button_id);
                    println!("Event:   {}", btn.event_name);
                    println!("Action:  {}", btn.action_name);
                })
            }
        }
        "source" => dev.source().map(|s| {
            if let Some(mac) = &s.source_mac {
                println!("{} ({})", s.source_type, mac);
            } else {
                println!("{}", s.source_type);
            }
        }),
        "route" => {
            if args.len() < 3 {
                eprintln!("Usage: bmapctl route <XX:XX:XX:XX:XX:XX>");
                process::exit(1);
            }
            dev.route(&args[2]).map(|_| println!("Routed to {}", args[2]))
        }
        "devices" => cmd_devices(&dev),
        "connect" | "disconnect" => {
            if args.len() < 3 {
                eprintln!("Usage: bmapctl {} <XX:XX:XX:XX:XX:XX>", cmd);
                process::exit(1);
            }
            let mac = args[2].to_uppercase();
            if cmd == "connect" {
                dev.connect_device(&mac).map(|_| println!("Connecting {}", mac))
            } else {
                dev.disconnect_device(&mac).map(|_| println!("Disconnecting {}", mac))
            }
        }
        "profiles" => dev.modes().map(|modes| {
            for m in &modes {
                print!("  {:2}  {}", m.mode_idx, m.name);
                if !m.editable { print!(" [preset]"); }
                else if !m.configured { print!(" [empty]"); }
                println!();
            }
        }),
        "dump" => {
            let pkt = bmap::protocol::bmap_packet(31, 1, bmap::Operator::Start, &[]);
            dev.send_raw(&pkt).map(|responses| {
                for r in &responses {
                    println!("{}", r.fmt());
                }
            })
        }
        "pair" => dev.pair().map(|_| println!("Pairing mode enabled")),
        "off" => dev.power_off().map(|_| println!("Powering off")),
        "raw" => {
            if args.len() < 3 {
                eprintln!("Usage: bmapctl raw <hex>");
                process::exit(1);
            }
            let hex_str: String = args[2..].join("").replace(' ', "");
            let data = hex_to_bytes(&hex_str);
            println!("TX: {}", hex_str);
            dev.send_raw(&data).map(|responses| {
                for r in &responses {
                    println!("RX: {}", r.fmt());
                }
            })
        }
        _ => {
            // Try as custom profile name
            match dev.set_mode(&cmd, false) {
                Ok(_) => Ok(println!("OK: {}", cmd)),
                Err(e) if is_unknown_mode(&e) => {
                    eprintln!("Unknown command: {}", cmd);
                    process::exit(1);
                }
                // A dropped or desynced link is not an unknown command.
                Err(e) => Err(e),
            }
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
}

/// Whether a failed mode switch means "no such mode" rather than a link or
/// device failure that the user needs to see.
fn is_unknown_mode(e: &BmapError) -> bool {
    matches!(e, BmapError::InvalidArg(_) | BmapError::Unsupported(_))
}

/// Follow-up hint for a failed connect; setup mistakes and a busy headset
/// are not Bluetooth problems.
fn connection_hint(e: &BmapError) -> Option<&'static str> {
    match e {
        BmapError::InvalidArg(_) | BmapError::Busy(_) => None,
        _ => Some("Is Bluetooth on? Are the headphones paired and connected?"),
    }
}

fn err_exit(e: &BmapError) {
    eprintln!("Error: {}", e);
    process::exit(1);
}

fn cmd_status(dev: &bmap::BmapConnection<impl bmap::Transport>) -> Result<(), BmapError> {
    let s = dev.status()?;
    let cnc_bar: String = "█".repeat(s.cnc_level as usize)
        + &"░".repeat((s.cnc_max - s.cnc_level) as usize);

    println!("  Model        {}", dev.config().info.name);
    println!("  Battery      {}%", s.battery);
    for (component_id, label) in dev.config().battery_components {
        if let Some(reading) = s
            .battery_readings
            .iter()
            .find(|reading| reading.component_id == *component_id)
        {
            println!("  {:12} {}%", label, reading.level);
        }
    }
    if !s.mode.is_empty() {
        println!("  Mode         {}", s.mode);
    }
    if dev.anr().is_ok() {
        if let Ok(anr) = dev.anr() {
            println!("  ANR          {}", anr);
        }
    } else {
        println!("  CNC          {} {}/{}", cnc_bar, s.cnc_level, s.cnc_max);
    }
    if !s.eq.is_empty() {
        let eq_str: Vec<String> = s.eq.iter().map(|b| format!("{:+}", b.current)).collect();
        println!("  EQ           {} (bass/mid/treble)", eq_str.join("/"));
    }
    println!("  Name         {}", s.name);
    println!("  FW           {}", s.firmware);
    println!("  Sidetone     {}", s.sidetone);
    println!("  Multipoint   {}", if s.multipoint { "on" } else { "off" });
    println!("  AutoPause    {}", if s.auto_pause { "on" } else { "off" });
    println!("  Prompts      {} ({})", if s.prompts_enabled { "on" } else { "off" }, s.prompts_language);
    Ok(())
}

/// A read the device may not support: its default instead of an error.
/// A dropped or desynced link still fails; that is not a missing feature.
fn optional<V>(result: BmapResult<V>, default: V) -> BmapResult<V> {
    match result {
        Ok(v) => Ok(v),
        Err(e @ (BmapError::Connection(_) | BmapError::Busy(_) | BmapError::Desync(_))) => Err(e),
        Err(_) => Ok(default),
    }
}

/// Source [5.1] reports this MAC while nothing is routed.
const NO_SOURCE_MAC: &str = "00:00:00:00:00:00";

/// Paired devices plus the active source's MAC, or (empty, None).
fn devices_with_source(
    dev: &bmap::BmapConnection<impl bmap::Transport>,
) -> BmapResult<(Vec<PairedDevice>, Option<String>)> {
    let active = active_mac(optional(dev.source().map(Some), None)?);
    Ok((optional(dev.paired_devices(), Vec::new())?, active))
}

/// The active source's MAC; None when nothing is routed.
fn active_mac(source: Option<bmap::device::AudioSource>) -> Option<String> {
    source.and_then(|s| s.source_mac).filter(|mac| mac != NO_SOURCE_MAC)
}

/// One line per paired device: `*` marks the active source, "this" the
/// host asking.
fn format_devices(devices: &[PairedDevice], active: Option<&str>) -> Vec<String> {
    devices
        .iter()
        .map(|d| {
            let mark = if Some(d.mac.as_str()) == active { "*" } else { " " };
            let state = if d.connected { "connected" } else { "paired" };
            let local = if d.local { "this" } else { "" };
            format!("  {} {}  {:9}  {:4}  {}", mark, d.mac, state, local, d.name)
        })
        .collect()
}

fn cmd_devices(dev: &bmap::BmapConnection<impl bmap::Transport>) -> Result<(), BmapError> {
    let (devices, active) = devices_with_source(dev)?;
    if devices.is_empty() {
        println!("No paired devices reported");
    }
    for line in format_devices(&devices, active.as_deref()) {
        println!("{}", line);
    }
    Ok(())
}

/// A JSON string literal. Non-ASCII stays UTF-8, as Python's
/// `json.dumps(..., ensure_ascii=False)` writes it.
fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn json_list(items: impl IntoIterator<Item = String>) -> String {
    format!("[{}]", items.into_iter().collect::<Vec<_>>().join(", "))
}

fn json_object(fields: &[(&str, String)]) -> String {
    let body: Vec<String> = fields
        .iter()
        .map(|(key, value)| format!("{}: {}", json_str(key), value))
        .collect();
    format!("{{{}}}", body.join(", "))
}

/// Everything `status --json` reports, gathered before rendering.
struct StatusSnapshot<'a> {
    model: &'a str,
    status: bmap::DeviceStatus,
    battery_components: &'a [(u8, &'a str)],
    audio_modes: bool,
    profiles: Vec<String>,
    source: Option<String>,
    devices: Vec<PairedDevice>,
}

/// Same keys, order and separators as the Python CLI's `status --json`.
fn status_json(snap: &StatusSnapshot) -> String {
    let s = &snap.status;
    let battery_parts = s.battery_readings.iter().filter_map(|r| {
        snap.battery_components
            .iter()
            .find(|(id, _)| *id == r.component_id)
            .map(|(_, label)| json_object(&[
                ("label", json_str(label)),
                ("level", r.level.to_string()),
            ]))
    });
    let active = snap.source.as_deref();
    let devices = snap.devices.iter().map(|d| json_object(&[
        ("mac", json_str(&d.mac)),
        ("name", json_str(&d.name)),
        ("connected", d.connected.to_string()),
        ("local", d.local.to_string()),
        ("active", (Some(d.mac.as_str()) == active).to_string()),
    ]));
    json_object(&[
        ("model", json_str(snap.model)),
        ("battery", s.battery.to_string()),
        ("battery_parts", json_list(battery_parts)),
        ("mode", json_str(&s.mode)),
        ("audio_modes", snap.audio_modes.to_string()),
        ("profiles", json_list(snap.profiles.iter().map(|p| json_str(p)))),
        ("cnc", s.cnc_level.to_string()),
        ("cnc_max", s.cnc_max.to_string()),
        ("eq", json_list(s.eq.iter().map(|b| b.current.to_string()))),
        ("name", json_str(&s.name)),
        ("firmware", json_str(&s.firmware)),
        ("sidetone", json_str(&s.sidetone)),
        ("multipoint", s.multipoint.to_string()),
        ("auto_pause", s.auto_pause.to_string()),
        ("prompts", s.prompts_enabled.to_string()),
        ("source", active.map_or_else(|| "null".to_string(), json_str)),
        ("devices", json_list(devices)),
    ])
}

/// One connection's worth of state as JSON, for scripts and panels.
fn cmd_status_json(dev: &bmap::BmapConnection<impl bmap::Transport>) -> Result<(), BmapError> {
    let status = dev.status()?;
    let (devices, source) = devices_with_source(dev)?;
    let audio_modes = dev.audio_modes_available()?;
    let mut profiles = Vec::new();
    if audio_modes {
        let mut modes = optional(dev.modes(), Vec::new())?;
        modes.sort_by_key(|m| m.mode_idx);
        profiles = modes
            .into_iter()
            .filter(|m| !m.editable || m.configured)
            .map(|m| m.name)
            .collect();
    }
    println!("{}", status_json(&StatusSnapshot {
        model: dev.config().info.name,
        status,
        battery_components: dev.config().battery_components,
        audio_modes,
        profiles,
        source,
        devices,
    }));
    Ok(())
}

fn hex_to_bytes(hex: &str) -> Vec<u8> {
    let hex = if hex.len() % 2 != 0 { &hex[..hex.len() - 1] } else { hex };
    (0..hex.len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect()
}

fn usage() {
    println!("bmapctl {} ({}) — Control BMAP Bluetooth audio devices",
             env!("CARGO_PKG_VERSION"), env!("GIT_HASH"));
    println!();
    println!("Usage: bmapctl <command> [args...]");
    println!();
    println!("  status [--json]     Show all settings (--json: plus devices, one line)");
    println!("  battery             Battery percentage");
    println!("  current             Current mode name");
    println!("  quiet/aware/...     Switch audio mode");
    println!("  cnc [0-10]          CNC level (0=max ANC, 10=most ambient)");
    println!("  anc [on|off]        Toggle Active Noise Cancellation");
    println!("  wind [on|off]       Toggle Wind Block (masks CNC effect)");
    println!("  eq [B M T]          Show/set EQ (-10 to +10)");
    println!("  name [TEXT]         Show/set device name");
    println!("  sidetone [MODE]     Show/set sidetone (off/low/medium/high)");
    println!("  multipoint [on|off] Toggle multipoint");
    println!("  autopause [on|off]  Toggle auto play/pause");
    println!("  prompts             Show voice prompt status");
    println!("  buttons             Show button mapping");
    println!("  buttons set <action> Remap button (e.g. ANC, VPA, Disabled)");
    println!("  source              Show active audio source");
    println!("  route <MAC>         Switch active BT device (XX:XX:XX:XX:XX:XX)");
    println!("  devices             Paired devices; * = active source");
    println!("  connect <MAC>       Connect a paired device to the headphones");
    println!("  disconnect <MAC>    Disconnect a device from the headphones");
    println!("  pair                Enter pairing mode");
    println!("  off                 Power off");
    println!("  raw <hex>           Send raw BMAP packet");
    println!();
    println!("Environment:");
    println!("  BMAP_MAC=XX:XX:XX:XX:XX:XX   Device MAC (auto-detected if unset)");
    println!("  BMAP_DEVICE=qc_ultra2|qc_ultra2_earbuds|qc_prince|qc35|qc_earbuds|qc45|ultra_open");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_mode_falls_back_only_for_lookup_errors() {
        assert!(is_unknown_mode(&BmapError::InvalidArg("Unknown mode: x".into())));
        assert!(is_unknown_mode(&BmapError::Unsupported("no modes".into())));
        assert!(!is_unknown_mode(&BmapError::Desync("[0.5] vs [31.3]".into())));
        assert!(!is_unknown_mode(&BmapError::Device { message: "x".into(), code: 0 }));
        assert!(!is_unknown_mode(&BmapError::Connection("gone".into())));
    }

    #[test]
    fn missing_device_type_skips_bluetooth_hint() {
        let err = connect(Some("00:11:22:33:44:55"), None).err().unwrap();
        assert!(matches!(err, BmapError::InvalidArg(_)));
        assert_eq!(connection_hint(&err), None);
        assert!(connection_hint(&BmapError::NotFound("none".into())).is_some());
    }

    #[test]
    fn busy_connect_skips_bluetooth_hint() {
        assert_eq!(connection_hint(&BmapError::Busy("Headphones busy".into())), None);
        assert!(connection_hint(&BmapError::Connection("No BMAP channel found".into())).is_some());
    }

    fn paired(mac: &str, name: &str, connected: bool, local: bool) -> PairedDevice {
        PairedDevice { mac: mac.into(), name: name.into(), connected, local, raw: Vec::new() }
    }

    fn sample_devices() -> Vec<PairedDevice> {
        vec![
            paired("00:11:22:33:44:01", "laptop", true, true),
            paired("00:11:22:33:44:02", "Owner\u{2019}s Phone", true, false),
            paired("00:11:22:33:44:03", "desk-mac", false, false),
        ]
    }

    fn sample_status() -> bmap::DeviceStatus {
        bmap::DeviceStatus {
            battery: 80,
            battery_readings: vec![
                bmap::BatteryReading { component_id: 1, level: 60 },
                bmap::BatteryReading { component_id: 4, level: 55 },
            ],
            mode: "quiet".into(),
            mode_idx: 0,
            cnc_level: 7,
            cnc_max: 10,
            eq: vec![bmap::EqBand { band_id: 0, name: "Bass", current: -2, min_val: -10, max_val: 10 }],
            name: "Say \"hi\"\\\n".into(),
            firmware: "2.0.4".into(),
            sidetone: "off".into(),
            multipoint: true,
            auto_pause: false,
            prompts_enabled: true,
            prompts_language: "US English".into(),
        }
    }

    #[test]
    fn devices_marks_active_source_and_local_host() {
        let lines = format_devices(&sample_devices(), Some("00:11:22:33:44:02"));
        let words: Vec<Vec<&str>> = lines.iter().map(|l| l.split_whitespace().collect()).collect();
        assert_eq!(words[0], vec!["00:11:22:33:44:01", "connected", "this", "laptop"]);
        assert_eq!(words[1][..3], ["*", "00:11:22:33:44:02", "connected"]);
        assert_eq!(words[2], vec!["00:11:22:33:44:03", "paired", "desk-mac"]);
    }

    #[test]
    fn status_json_carries_devices_and_capability() {
        let json = status_json(&StatusSnapshot {
            model: "Bose QC Ultra Headphones 2",
            status: sample_status(),
            battery_components: &[(1, "Right")],
            audio_modes: false,
            profiles: Vec::new(),
            source: Some("00:11:22:33:44:02".into()),
            devices: sample_devices(),
        });
        assert_eq!(json, concat!(
            r#"{"model": "Bose QC Ultra Headphones 2", "battery": 80, "#,
            r#""battery_parts": [{"label": "Right", "level": 60}], "mode": "quiet", "#,
            r#""audio_modes": false, "profiles": [], "cnc": 7, "cnc_max": 10, "eq": [-2], "#,
            r#""name": "Say \"hi\"\\\n", "firmware": "2.0.4", "sidetone": "off", "#,
            r#""multipoint": true, "auto_pause": false, "prompts": true, "#,
            r#""source": "00:11:22:33:44:02", "devices": ["#,
            r#"{"mac": "00:11:22:33:44:01", "name": "laptop", "connected": true, "local": true, "active": false}, "#,
            "{\"mac\": \"00:11:22:33:44:02\", \"name\": \"Owner\u{2019}s Phone\", ",
            r#""connected": true, "local": false, "active": true}, "#,
            r#"{"mac": "00:11:22:33:44:03", "name": "desk-mac", "connected": false, "local": false, "active": false}]}"#,
        ));
    }

    #[test]
    fn status_json_zero_source_means_none() {
        let json = status_json(&StatusSnapshot {
            model: "m",
            status: sample_status(),
            battery_components: &[],
            audio_modes: true,
            profiles: vec!["quiet".into()],
            source: None,
            devices: sample_devices(),
        });
        assert!(json.contains(r#""source": null"#));
        assert!(json.contains(r#""profiles": ["quiet"]"#));
        assert!(!json.contains(r#""active": true"#));
    }

    #[test]
    fn zero_source_mac_is_no_active_source() {
        let src = |mac: &str| Some(bmap::device::AudioSource {
            source_type: "bluetooth", source_mac: Some(mac.into()),
        });
        assert_eq!(active_mac(src("00:00:00:00:00:00")), None);
        assert_eq!(active_mac(src("00:11:22:33:44:02")).as_deref(), Some("00:11:22:33:44:02"));
        assert_eq!(active_mac(None), None);
    }

    #[test]
    fn optional_reads_keep_link_failures() {
        assert_eq!(optional::<u8>(Err(BmapError::Unsupported("x".into())), 7).unwrap(), 7);
        assert_eq!(optional::<u8>(Err(BmapError::Timeout("x".into())), 7).unwrap(), 7);
        assert!(optional::<u8>(Err(BmapError::Desync("x".into())), 7).is_err());
        assert!(optional::<u8>(Err(BmapError::Connection("x".into())), 7).is_err());
    }

    #[test]
    fn json_str_escapes_controls() {
        assert_eq!(json_str("a\u{1}b\tc"), r#""a\u0001b\tc""#);
    }
}
