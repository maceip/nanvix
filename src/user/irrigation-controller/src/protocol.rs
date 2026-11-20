// Unverified text protocol shared by the host binary and the nanvix guest.
// One command per line in, one JSON state line out:
//   open <z> | close <z> | tick | refill <liters> | state
// no_std + alloc so it builds for nanvix.

use alloc::{format, string::String, vec::Vec};
use crate::ctl::*;

pub fn state_json(c: &Ctl, ok: bool, msg: &str, runtime: &str) -> String {
    let b = |v: &[bool; 4]| v.iter().map(|x| format!("{}", x)).collect::<Vec<_>>().join(",");
    let n = |v: &[u64; 4]| v.iter().map(|x| format!("{}", x)).collect::<Vec<_>>().join(",");
    format!(
        "{{\"ok\":{},\"msg\":\"{}\",\"runtime\":\"{}\",\"open\":[{}],\"run\":[{}],\"delivered\":[{}],\"tank\":{},\"ticks\":{},\"limits\":{{\"zones\":{},\"max_open\":{},\"max_run\":{},\"flow\":{},\"tank_cap\":{}}}}}",
        ok, msg, runtime, b(&c.open), n(&c.run), n(&c.delivered), c.tank, c.ticks,
        NZONES, MAX_OPEN, MAX_RUN, FLOW, TANK_CAP
    )
}

/// Applies one command line. Returns None for `quit`.
pub fn handle(c: &mut Ctl, line: &str, runtime: &str) -> Option<String> {
    let mut parts = line.split_whitespace();
    let cmd = parts.next().unwrap_or("");
    let arg = parts.next().and_then(|s| s.parse::<u64>().ok());
    let (ok, msg) = match (cmd, arg) {
        ("open", Some(z)) => {
            let g = c.open_zone(z as usize);
            (g, if g { "opened" } else { "denied" })
        }
        ("close", Some(z)) => {
            let g = c.close_zone(z as usize);
            (g, if g { "closed" } else { "not open" })
        }
        ("tick", _) if c.ticks < u64::MAX / FLOW - 1 => {
            c.tick();
            (true, "tick")
        }
        ("refill", Some(l)) => {
            c.refill(l);
            (true, "refilled")
        }
        ("state", _) => (true, "state"),
        ("quit", _) => return None,
        _ => (false, "bad command"),
    };
    Some(state_json(c, ok, msg, runtime))
}
